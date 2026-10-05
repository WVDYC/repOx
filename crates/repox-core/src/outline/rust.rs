/// Extracts architectural outline and signatures for Rust source code.
///
/// Preserves:
/// - Doc comments (`///`, `//!`)
/// - Attributes (`#[derive(...)]`, etc.)
/// - Module declarations (`mod foo;`, `pub mod foo;`)
/// - Use statements (`use ...;`)
/// - Struct, Enum, Union definitions with field types
/// - Trait definitions with method signatures
/// - Type aliases and Const/Static declarations
/// - Impl block headers and method signatures (with bodies replaced by `{ /* ... */ }`)
pub fn extract_rust_outline(source: &str) -> String {
    let mut out = String::with_capacity(source.len() / 3);
    let mut in_fn_body = false;
    let mut fn_brace_depth: usize = 0;
    let mut current_brace_depth: usize = 0;

    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // If we are currently inside a function body that we are skipping
        if in_fn_body {
            let open_braces = line.chars().filter(|&c| c == '{').count();
            let close_braces = line.chars().filter(|&c| c == '}').count();
            current_brace_depth += open_braces;

            if current_brace_depth >= close_braces {
                current_brace_depth -= close_braces;
            } else {
                current_brace_depth = 0;
            }

            if current_brace_depth < fn_brace_depth {
                in_fn_body = false;
            }
            i += 1;
            continue;
        }

        // Detect function definitions (standalone fn or method in impl/trait)
        let is_fn_decl = is_rust_fn_declaration(trimmed);

        if is_fn_decl {
            let mut sig = String::from(line);
            // Handle multi-line signatures until '{' or ';'
            while !sig.contains('{') && !sig.contains(';') && i + 1 < lines.len() {
                i += 1;
                sig.push('\n');
                sig.push_str(lines[i]);
            }

            if sig.contains('{') {
                // Replace everything from the opening '{' with '{ /* ... */ }'
                if let Some(brace_idx) = sig.find('{') {
                    let prefix = &sig[..brace_idx].trim_end();
                    out.push_str(prefix);
                    out.push_str(" { /* ... */ }\n");

                    // Check if the closing brace was on the same line
                    let rest = &sig[brace_idx..];
                    let open_count = rest.chars().filter(|&c| c == '{').count();
                    let close_count = rest.chars().filter(|&c| c == '}').count();

                    if open_count > close_count {
                        in_fn_body = true;
                        fn_brace_depth = current_brace_depth + 1;
                        current_brace_depth = current_brace_depth + open_count - close_count;
                    }
                } else {
                    out.push_str(&sig);
                    out.push('\n');
                }
            } else {
                // Trait method signature without body (ends with ;)
                out.push_str(&sig);
                out.push('\n');
            }
            i += 1;
            continue;
        }

        // Track general brace depth for non-skipped lines
        let open_braces = line.chars().filter(|&c| c == '{').count();
        let close_braces = line.chars().filter(|&c| c == '}').count();
        current_brace_depth = current_brace_depth
            .saturating_add(open_braces)
            .saturating_sub(close_braces);

        out.push_str(line);
        out.push('\n');
        i += 1;
    }

    out
}

fn is_rust_fn_declaration(trimmed: &str) -> bool {
    let prefixes = [
        "fn ",
        "pub fn ",
        "pub(crate) fn ",
        "pub(super) fn ",
        "async fn ",
        "pub async fn ",
        "pub(crate) async fn ",
        "const fn ",
        "pub const fn ",
        "unsafe fn ",
        "pub unsafe fn ",
        "extern fn ",
        "pub extern fn ",
    ];

    prefixes.iter().any(|prefix| trimmed.starts_with(prefix))
        || (trimmed.contains(" fn ")
            && (trimmed.starts_with('#')
                || trimmed.starts_with("/*")
                || !trimmed.starts_with("//")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_rust_outline_simple() {
        let code = r#"use std::path::Path;

/// A test struct.
#[derive(Debug)]
pub struct Config {
    pub name: String,
    pub count: u32,
}

impl Config {
    pub fn new(name: String) -> Self {
        let count = 42;
        Self { name, count }
    }

    pub fn is_valid(&self) -> bool {
        self.count > 0
    }
}
"#;

        let outline = extract_rust_outline(code);
        assert!(outline.contains("pub struct Config"));
        assert!(outline.contains("pub name: String"));
        assert!(outline.contains("pub fn new(name: String) -> Self { /* ... */ }"));
        assert!(outline.contains("pub fn is_valid(&self) -> bool { /* ... */ }"));
        assert!(!outline.contains("let count = 42;"));
    }
}
