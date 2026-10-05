pub mod markdown;
pub mod tool_call;
pub mod xml;

use crate::domain::{OutputFormat, RepoFile};

/// Formats the repository files using the specified `OutputFormat`.
pub fn format_repository(files: &[RepoFile], format: OutputFormat) -> String {
    match format {
        OutputFormat::Xml => xml::format_xml(files),
        OutputFormat::Markdown => markdown::format_markdown(files),
        OutputFormat::ToolCall => tool_call::format_tool_calls(files),
    }
}
