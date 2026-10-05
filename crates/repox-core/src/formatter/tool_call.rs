use crate::domain::RepoFile;

/// Fast, allocation-conscious JSON string escaping.
pub fn escape_json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + (s.len() / 10));
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x08' => out.push_str("\\b"),
            '\x0C' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

/// Formats repository files into a synthetic JSON tool-call response array.
///
/// Designed for agent harnesses, Ollama tool loops, and LLM priming where
/// the model expects source files to originate from authoritative `read_file` tool results.
pub fn format_tool_calls(files: &[RepoFile]) -> String {
    if files.is_empty() {
        return "[]\n".to_string();
    }

    let est_cap = files.iter().map(|f| f.content.len() + 180).sum::<usize>() + 16;
    let mut out = String::with_capacity(est_cap);
    out.push_str("[\n");

    for (idx, file) in files.iter().enumerate() {
        let display_path = file.display_path();
        let escaped_path = escape_json_str(&display_path);
        let escaped_content = escape_json_str(&file.content);
        let call_id = format!("call_read_file_{}", idx + 1);

        out.push_str("  {\n");
        out.push_str("    \"role\": \"tool\",\n");
        out.push_str(&format!("    \"tool_call_id\": \"{call_id}\",\n"));
        out.push_str("    \"name\": \"read_file\",\n");
        out.push_str(&format!("    \"path\": \"{escaped_path}\",\n"));
        out.push_str(&format!("    \"content\": \"{escaped_content}\"\n"));
        out.push_str("  }");

        if idx + 1 < files.len() {
            out.push(',');
        }
        out.push('\n');
    }

    out.push_str("]\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_format_tool_calls_empty() {
        let files: Vec<RepoFile> = Vec::new();
        assert_eq!(format_tool_calls(&files), "[]\n");
    }

    #[test]
    fn test_escape_json_special_chars() {
        let input = "hello \"world\"\nline 2\t\r\\test";
        let escaped = escape_json_str(input);
        assert_eq!(escaped, "hello \\\"world\\\"\\nline 2\\t\\r\\\\test");
    }

    #[test]
    fn test_format_tool_calls_single_file() {
        let file = RepoFile::new(
            PathBuf::from("src/main.rs"),
            PathBuf::from("/repo/src/main.rs"),
            20,
            "fn main() {\n  println!(\"hi\");\n}".to_string(),
        );

        let output = format_tool_calls(&[file]);
        assert!(output.starts_with("[\n  {\n"));
        assert!(output.contains("\"role\": \"tool\""));
        assert!(output.contains("\"tool_call_id\": \"call_read_file_1\""));
        assert!(output.contains("\"name\": \"read_file\""));
        assert!(output.contains("\"path\": \"src/main.rs\""));
        assert!(output.contains("fn main() {\\n  println!(\\\"hi\\\");\\n}"));
        assert!(output.ends_with("  }\n]\n"));
    }
}
