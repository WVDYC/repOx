/// Extracts architectural outline and signatures for Python source code.
///
/// Preserves:
/// - Imports (`import ...`, `from ... import ...`)
/// - Class definitions and class-level attributes
/// - Function/method signatures and decorators
/// - Module-level type annotations and constants
/// - Replaces function implementation bodies with `...`
pub fn extract_python_outline(source: &str) -> String {
    let mut out = String::with_capacity(source.len() / 3);
    let lines: Vec<&str> = source.lines().collect();
    let mut in_fn_body = false;
    let mut fn_indent: usize = 0;

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed.is_empty() {
            if !in_fn_body {
                out.push('\n');
            }
            i += 1;
            continue;
        }

        let indent = line.len() - line.trim_start().len();

        // If inside function body, check if we've exited the indentation level
        if in_fn_body {
            if indent > fn_indent {
                // Still inside function body, skip implementation
                i += 1;
                continue;
            } else {
                in_fn_body = false;
            }
        }

        // Decorators (@property, @dataclass, etc.)
        if trimmed.starts_with('@') {
            out.push_str(line);
            out.push('\n');
            i += 1;
            continue;
        }

        // Function or method definition
        if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
            let mut sig = String::from(line);
            // Handle multi-line signatures until ':'
            while !sig.trim_end().ends_with(':') && i + 1 < lines.len() {
                i += 1;
                sig.push('\n');
                sig.push_str(lines[i]);
            }

            out.push_str(&sig);
            out.push('\n');

            // Insert standard Python ellipsis stub
            let body_indent = indent + 4;
            let spaces = " ".repeat(body_indent);
            out.push_str(&spaces);
            out.push_str("...\n");

            in_fn_body = true;
            fn_indent = indent;
            i += 1;
            continue;
        }

        // Class definition, imports, or class/module variables
        out.push_str(line);
        out.push('\n');
        i += 1;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_python_outline() {
        let code = r#"import os
from typing import Optional, List

TIMEOUT: int = 30

class ModelHandler:
    """Handles LLM calls."""
    model_name: str

    def __init__(self, name: str):
        self.name = name
        self.cache = {}

    async def generate(self, prompt: str) -> str:
        temp = 123
        return f"result_{temp}"
"#;

        let outline = extract_python_outline(code);
        assert!(outline.contains("class ModelHandler:"));
        assert!(outline.contains("def __init__(self, name: str):"));
        assert!(outline.contains("async def generate(self, prompt: str) -> str:"));
        assert!(outline.contains("..."));
        assert!(!outline.contains("self.cache = {}"));
        assert!(!outline.contains("temp = 123"));
    }
}
