use crate::domain::RepoFile;
use crate::tree::generate_file_tree;

/// Calculates the required fence backtick count to avoid collision with content containing backticks.
fn calculate_fence(content: &str) -> String {
    let mut max_backticks = 2;
    let mut current_streak = 0;

    for byte in content.bytes() {
        if byte == b'`' {
            current_streak += 1;
            if current_streak > max_backticks {
                max_backticks = current_streak;
            }
        } else {
            current_streak = 0;
        }
    }

    "`".repeat(max_backticks + 1)
}

/// Formats the repository files into standard Markdown with language-tagged code blocks.
pub fn format_markdown(files: &[RepoFile]) -> String {
    let mut output = String::with_capacity(files.iter().map(|f| f.content.len() + 128).sum::<usize>() + 2048);

    // 1. Repository structure section
    let paths: Vec<_> = files.iter().map(|f| &f.relative_path).collect();
    let tree = generate_file_tree(&paths);

    output.push_str("# Repository Structure\n\n```\n");
    output.push_str(&tree);
    output.push_str("```\n\n");

    // 2. Repository files section
    output.push_str("# Repository Files\n\n");

    for file in files {
        let display_path = file.display_path();
        let lang = file.language_hint();
        let fence = calculate_fence(&file.content);

        output.push_str(&format!("## File: {display_path}\n\n"));
        output.push_str(&format!("{fence}{lang}\n"));
        output.push_str(&file.content);
        if !file.content.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&fence);
        output.push_str("\n\n");
    }

    if output.ends_with("\n\n") {
        output.truncate(output.len() - 1);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_format_markdown_output() {
        let files = vec![
            RepoFile::new(
                PathBuf::from("Cargo.toml"),
                PathBuf::from("/repo/Cargo.toml"),
                20,
                "[package]\nname = \"repox\"\n".to_string(),
            ),
            RepoFile::new(
                PathBuf::from("src/main.rs"),
                PathBuf::from("/repo/src/main.rs"),
                30,
                "fn main() {}\n".to_string(),
            ),
        ];

        let md = format_markdown(&files);

        assert!(md.contains("# Repository Structure"));
        assert!(md.contains("├── Cargo.toml"));
        assert!(md.contains("## File: Cargo.toml\n\n```toml\n[package]"));
        assert!(md.contains("## File: src/main.rs\n\n```rust\nfn main() {}\n```"));
    }

    #[test]
    fn test_markdown_fence_collision_handling() {
        let files = vec![RepoFile::new(
            PathBuf::from("docs/example.md"),
            PathBuf::from("/repo/docs/example.md"),
            50,
            "Here is a code block:\n```rust\nlet x = 1;\n```\n".to_string(),
        )];

        let md = format_markdown(&files);
        // The outer fence should use 4 backticks (````markdown) to wrap the internal 3 backticks
        assert!(md.contains("````markdown\nHere is a code block:\n```rust\nlet x = 1;\n```\n````"));
    }
}
