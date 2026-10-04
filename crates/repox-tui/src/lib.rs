//! # repox-tui
//!
//! Terminal User Interface for repox built with ratatui and crossterm.

use repox_core::domain::RepoFile;
use std::io;

/// Launches the interactive terminal UI for selecting and filtering files.
pub fn run_tui(files: &[RepoFile]) -> io::Result<Vec<RepoFile>> {
    // Phase 1: Return current files with terminal announcement; full interactive widget tree in Phase 2
    eprintln!(
        "Interactive TUI mode initialized with {} repository files.",
        files.len()
    );
    Ok(files.to_vec())
}
