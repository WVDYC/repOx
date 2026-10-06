//! # repox-tui
//!
//! Interactive, lazygit-style file picker for `repox`.
//!
//! ```text
//! ┌ Files ───────────────────┐┌ src/main.rs · 1.2k tok · 4.5 KB ─────┐
//! │ ▾ [-] src/          18k  ││ 1 │ fn main() {                      │
//! │ │ ▸ [x] lib.rs      18k  ││ 2 │     println!("hello");           │
//! │ │   [ ] main.rs    320   ││ 3 │ }                                │
//! └──────────────────────────┘└──────────────────────────────────────┘
//!  3/5 files │ ████░░░░░░░░ 18,420 / 200,000 tokens (9.2%) [Claude 3.5 Sonnet] │ 124.5 KB
//! ```
//!
//! Design notes:
//!
//! * The tree is a flat, pre-order arena ([`tree::FlatTree`]); there are no
//!   recursive widgets. Selection aggregates are cached per node, so the status
//!   bar is `O(1)` and never re-tokenizes.
//! * Frames are drawn straight into the ratatui buffer and allocate nothing.
//! * The UI is drawn on **stderr**, so `repox -i | pbcopy` keeps stdout clean.
//!
//! Callers must pre-compute `RepoFile::token_count` (the scanner does not); files
//! without a count fall back to a `bytes / 4` estimate.

mod app;
mod error;
pub mod highlight;
mod preview;
mod terminal;
mod tree;
mod ui;
mod util;

use std::time::Duration;

use ratatui::crossterm::event;
use repox_core::RepoFile;

use crate::app::{App, Exit};
use crate::terminal::Tui;

pub use crate::error::TuiError;

/// Presentation settings for the UI.
#[derive(Debug, Clone)]
pub struct TuiOptions {
    /// Model name shown next to the token gauge, e.g. `Claude 3.5 Sonnet`.
    pub model_name: String,
    /// Context window (in tokens) the gauge is measured against.
    pub context_limit: usize,
    /// Directories shallower than this start expanded (default `2`).
    pub expand_depth: u16,
}

impl TuiOptions {
    pub fn new(model_name: impl Into<String>, context_limit: usize) -> Self {
        Self {
            model_name: model_name.into(),
            context_limit,
            expand_depth: 2,
        }
    }
}

/// What the user asked to do with the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiAction {
    /// `c`: copy the formatted context to the clipboard.
    Copy,
    /// `C`: copy the reproducible CLI command invocation to the clipboard.
    CopyCommand,
    /// `Enter` on a file: output through the normal channels (stdout / `-o`).
    Output,
}

/// Result of an interactive session.
#[derive(Debug)]
pub enum TuiOutcome {
    /// `q` / `Esc` / `Ctrl-C`: nothing should happen.
    Cancelled,
    /// The user confirmed. `files` keeps the scanner's original order.
    Selected {
        action: TuiAction,
        files: Vec<RepoFile>,
    },
}

/// Runs the interactive picker and returns the user's selection.
///
/// The terminal is always restored before this returns, including when an error
/// occurs or the process panics.
pub fn run(files: Vec<RepoFile>, options: TuiOptions) -> Result<TuiOutcome, TuiError> {
    let mut app = App::new(files, options);

    let (mut terminal, guard) = terminal::enter()?;
    let result = event_loop(&mut terminal, &mut app);
    // Restore the screen first so anything the caller prints lands in the
    // normal buffer.
    drop(terminal);
    drop(guard);

    Ok(match result? {
        Exit::Abort => TuiOutcome::Cancelled,
        Exit::Copy => TuiOutcome::Selected {
            action: TuiAction::Copy,
            files: app.into_selected(),
        },
        Exit::CopyCommand => TuiOutcome::Selected {
            action: TuiAction::CopyCommand,
            files: app.into_selected(),
        },
        Exit::Output => TuiOutcome::Selected {
            action: TuiAction::Output,
            files: app.into_selected(),
        },
    })
}

/// Draw -> block for an event -> drain whatever else is already queued -> draw.
///
/// Redrawing only after input means zero CPU while idle, and draining the queue
/// coalesces key-repeat bursts (holding `j`) into a single frame.
fn event_loop(terminal: &mut Tui, app: &mut App) -> Result<Exit, TuiError> {
    loop {
        terminal.draw(|frame| ui::render(frame, app))?;

        let mut next = event::read()?;
        loop {
            if let Some(exit) = app.handle_event(next) {
                return Ok(exit);
            }
            if !event::poll(Duration::ZERO)? {
                break;
            }
            next = event::read()?;
        }
    }
}
