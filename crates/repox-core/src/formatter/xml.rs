use crate::domain::RepoFile;
use crate::tree::generate_file_tree;

/// Formats the repository files into Claude-optimized XML prompt structure.
///
/// Structure:
/// ```xml
/// <repository_structure>
/// .
/// ├── src
/// │   └── main.rs
/// └── Cargo.toml
/// </repository_structure>
///
/// <file path="Cargo.toml">
/// ... content ...
/// </file>
///
/// <file path="src/main.rs">
/// ... content ...
/// </file>
/// ```
pub fn format_xml(files: &[RepoFile]) -> String {
    let mut output =
        String::with_capacity(files.iter().map(|f| f.content.len() + 128).sum::<usize>() + 2048);

    // 1. Generate repository structure tree
    let paths: Vec<_> = files.iter().map(|f| &f.relative_path).collect();
    let tree = generate_file_tree(&paths);

    output.push_str("<repository_structure>\n");
    output.push_str(&tree);
    output.push_str("</repository_structure>\n\n");

    // 2. Append each file block
    for file in files {
        let display_path = file.display_path();
        output.push_str(&format!("<file path=\"{display_path}\">\n"));
        output.push_str(&file.content);
        if !file.content.ends_with('\n') {
            output.push('\n');
        }
        output.push_str("</file>\n\n");
    }

    // Strip trailing newline
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
    fn test_format_xml_output() {
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

        let xml = format_xml(&files);

        assert!(xml.contains("<repository_structure>"));
        assert!(xml.contains("├── Cargo.toml"));
        assert!(xml.contains("└── src"));
        assert!(xml.contains("</repository_structure>"));
        assert!(xml.contains("<file path=\"Cargo.toml\">\n[package]\nname = \"repox\"\n</file>"));
        assert!(xml.contains("<file path=\"src/main.rs\">\nfn main() {}\n</file>"));
    }
}
