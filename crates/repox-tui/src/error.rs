use thiserror::Error;

/// Errors surfaced by the interactive UI.
#[derive(Debug, Error)]
pub enum TuiError {
    /// The TUI is drawn on stderr (so stdout stays pipe-friendly), which must
    /// therefore be attached to a terminal.
    #[error("interactive mode needs a terminal (stderr is not a TTY)")]
    NotATerminal,

    /// Any terminal I/O failure (raw mode, drawing, reading events).
    #[error("terminal I/O error: {0}")]
    Io(#[from] std::io::Error),
}
