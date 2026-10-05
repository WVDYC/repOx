//! # repox-core
//!
//! High-performance engine for scanning, filtering, token counting, and formatting
//! source code repositories into LLM context prompts.

pub mod domain;
pub mod error;
pub mod filter;
pub mod formatter;
pub mod outline;
pub mod scanner;
pub mod tokenizer;
pub mod tree;

// Public re-exports for ergonomic library consumption
pub use domain::{OutputFormat, RepoFile, ScanOptions, ScanSummary, TokenProfile};
pub use error::{RepoxError, Result};
pub use formatter::format_repository;
pub use outline::extract_outline;
pub use scanner::scan_repository;
pub use tokenizer::{TokenCounter, count_text_tokens};
pub use tree::generate_file_tree;
