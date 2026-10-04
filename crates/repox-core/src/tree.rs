use std::collections::BTreeMap;
use std::path::Path;

/// A node in the repository directory hierarchy.
#[derive(Debug, Default)]
struct TreeNode {
    /// Subdirectories or files mapped by their name.
    children: BTreeMap<String, TreeNode>,
    /// Indicates whether this node corresponds to a file.
    is_file: bool,
}

impl TreeNode {
    fn insert(&mut self, path: &Path) {
        let mut current = self;
        for component in path.components() {
            let name = component.as_os_str().to_string_lossy().to_string();
            current = current.children.entry(name).or_default();
        }
        current.is_file = true;
    }

    fn render(&self, buffer: &mut String, prefix: &str) {
        let total = self.children.len();
        for (i, (name, child)) in self.children.iter().enumerate() {
            let is_last = i + 1 == total;
            let branch = if is_last { "└── " } else { "├── " };
            buffer.push_str(prefix);
            buffer.push_str(branch);
            buffer.push_str(name);
            buffer.push('\n');

            let next_prefix = if is_last {
                format!("{prefix}    ")
            } else {
                format!("{prefix}│   ")
            };

            child.render(buffer, &next_prefix);
        }
    }
}

/// Generates an ASCII tree visualization for a list of relative file paths.
pub fn generate_file_tree<P: AsRef<Path>>(paths: &[P]) -> String {
    let mut root = TreeNode::default();
    for path in paths {
        let p = path.as_ref();
        if p.as_os_str().is_empty() || p == Path::new(".") {
            continue;
        }
        root.insert(p);
    }

    let mut buffer = String::new();
    buffer.push_str(".\n");
    root.render(&mut buffer, "");
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_tree_generation() {
        let paths = vec![
            PathBuf::from("Cargo.toml"),
            PathBuf::from("crates/repox-core/Cargo.toml"),
            PathBuf::from("crates/repox-core/src/lib.rs"),
            PathBuf::from("src/main.rs"),
        ];

        let tree = generate_file_tree(&paths);
        let expected = ".\n├── Cargo.toml\n├── crates\n│   └── repox-core\n│       ├── Cargo.toml\n│       └── src\n│           └── lib.rs\n└── src\n    └── main.rs\n";
        assert_eq!(tree, expected);
    }

    #[test]
    fn test_empty_tree() {
        let paths: Vec<PathBuf> = vec![];
        let tree = generate_file_tree(&paths);
        assert_eq!(tree, ".\n");
    }
}
