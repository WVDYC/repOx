//! Terminal lifecycle: setup, teardown and crash safety.
//!
//! The UI is drawn on **stderr** so that `repox -i | pbcopy` keeps stdout clean
//! for the generated prompt. crossterm reads keys from the controlling TTY
//! even when stdin is redirected.

use std::io::{self, IsTerminal, Stderr};
use std::panic;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::cursor::{Hide, Show};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

use crate::error::TuiError;

pub type Tui = Terminal<CrosstermBackend<Stderr>>;

/// `true` while the terminal is in TUI mode. The panic hook only restores the
/// terminal when this is set, so a panic outside the TUI prints no stray escapes.
static TUI_ACTIVE: AtomicBool = AtomicBool::new(false);
static HOOK: Once = Once::new();

/// Restores the terminal when dropped (normal exit, early `?` return, unwinding).
#[must_use = "dropping the guard restores the terminal"]
pub struct TerminalGuard(());

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore();
    }
}

/// Idempotent teardown shared by the guard and the panic hook.
fn restore() {
    if TUI_ACTIVE.swap(false, Ordering::SeqCst) {
        // Best effort: there is nowhere left to report failures to.
        let _ = execute!(io::stderr(), Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

/// Chains a hook in front of the existing one (e.g. `color-eyre`'s): restore the
/// terminal first, *then* let the previous hook print the report on a sane screen.
fn install_panic_hook() {
    HOOK.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            restore();
            previous(info);
        }));
    });
}

/// Enters raw mode + alternate screen and hides the cursor.
pub fn enter() -> Result<(Tui, TerminalGuard), TuiError> {
    if !io::stderr().is_terminal() {
        return Err(TuiError::NotATerminal);
    }
    install_panic_hook();

    enable_raw_mode()?;
    // From here on the guard owns cleanup, including if the next steps fail.
    TUI_ACTIVE.store(true, Ordering::SeqCst);
    let guard = TerminalGuard(());

    execute!(io::stderr(), EnterAlternateScreen, Hide)?;
    let terminal = Terminal::new(CrosstermBackend::new(io::stderr()))?;
    Ok((terminal, guard))
}
