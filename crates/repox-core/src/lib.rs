//! # repox-core
//!
//! High-performance engine for scanning, filtering, token counting, and formatting
//! source code repositories into LLM context prompts.

pub mod budget;
pub mod domain;
pub mod error;
pub mod filter;
pub mod formatter;
pub mod git;
pub mod lockfile;
pub mod outline;
pub mod redact;
pub mod scanner;
pub mod tokenizer;
pub mod tree;
pub mod video;

// Public re-exports for ergonomic library consumption
pub use budget::apply_token_budget;
pub use domain::{OutputFormat, RepoFile, ScanOptions, ScanSummary, TokenProfile};
pub use error::{RepoxError, Result};
pub use formatter::format_repository;
pub use git::get_git_changed_files;
pub use lockfile::summarize_lockfile;
pub use outline::extract_outline;
pub use redact::redact_secrets;
pub use scanner::scan_repository;
pub use tokenizer::{TokenCounter, count_text_tokens};
pub use tree::generate_file_tree;
pub use video::{
    compute_frame_sharpness, optimize_video_prompt, select_sharpest_frame,
    summarize_comfyui_workflow,
};
