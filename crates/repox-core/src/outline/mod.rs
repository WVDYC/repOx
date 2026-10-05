pub mod generic_c;
pub mod python;
pub mod rust;

/// Extracts an architectural outline / signature skeleton for a source code file.
///
/// Strips internal function and method implementation bodies while preserving:
/// - Type definitions (structs, classes, interfaces, enums, unions, type aliases)
/// - Function and method signatures with parameter and return types
/// - Module imports, exports, traits, and package declarations
/// - Docstrings and attributes
///
/// Drastically compresses multi-thousand-line repositories into compact token budgets
/// ideal for high-level architectural queries, refactor planning, and system mapping.
pub fn extract_outline(source: &str, language_hint: &str) -> String {
    match language_hint {
        "rust" => rust::extract_rust_outline(source),
        "python" => python::extract_python_outline(source),
        "go" | "typescript" | "javascript" | "tsx" | "jsx" | "c" | "cpp" | "java" | "kotlin"
        | "swift" | "csharp" => generic_c::extract_generic_c_outline(source, language_hint),
        _ => source.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_outline_unsupported_language_passthrough() {
        let code = "key: value\nanother: 123\n";
        assert_eq!(extract_outline(code, "yaml"), code);
    }
}
