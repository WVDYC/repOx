use ratatui::style::{Color, Modifier, Style};

/// Lightweight, high-speed lexical syntax highlighter for the TUI preview pane.
///
/// Highlights keywords, types, strings, comments, numbers, and boolean literals
/// across Rust, Python, TOML, JSON, YAML, Go, TypeScript, JavaScript, C, and Shell.
pub fn highlight_line<'a>(line: &'a str, lang: &str) -> Vec<(&'a str, Style)> {
    if line.is_empty() {
        return Vec::new();
    }

    let trimmed = line.trim_start();
    let indent_len = line.len() - trimmed.len();

    let mut spans = Vec::new();
    if indent_len > 0 {
        spans.push((&line[..indent_len], Style::new()));
    }

    if is_comment_start(trimmed, lang) {
        spans.push((trimmed, Style::new().fg(Color::DarkGray)));
        return spans;
    }

    let mut rest = trimmed;
    while !rest.is_empty() {
        if is_comment_start(rest, lang) {
            spans.push((rest, Style::new().fg(Color::DarkGray)));
            break;
        }

        // Quoted strings
        if rest.starts_with('"') || rest.starts_with('\'') || rest.starts_with('`') {
            let quote = rest.chars().next().unwrap();
            let mut end_idx = rest.len();
            let mut escaped = false;
            for (idx, ch) in rest[1..].char_indices() {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == quote {
                    end_idx = idx + 2;
                    break;
                }
            }
            let (str_tok, remainder) = rest.split_at(end_idx);
            spans.push((str_tok, Style::new().fg(Color::Green)));
            rest = remainder;
            continue;
        }

        let first_char = rest.chars().next().unwrap();

        // Identifiers (keywords, types, variables)
        if first_char.is_alphabetic() || first_char == '_' {
            let ident_len = rest
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(rest.len());
            let (word, remainder) = rest.split_at(ident_len);

            let style = if is_keyword(word, lang) {
                Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD)
            } else if is_type_name(word, lang) {
                Style::new().fg(Color::Yellow)
            } else if is_boolean_or_nil(word) {
                Style::new().fg(Color::Cyan)
            } else {
                Style::new().fg(Color::White)
            };

            spans.push((word, style));
            rest = remainder;
            continue;
        }

        // Numbers
        if first_char.is_ascii_digit() {
            let num_len = rest
                .find(|c: char| !c.is_alphanumeric() && c != '.' && c != '_')
                .unwrap_or(rest.len());
            let (num, remainder) = rest.split_at(num_len);
            spans.push((num, Style::new().fg(Color::Blue)));
            rest = remainder;
            continue;
        }

        // Punctuation, symbols, operators
        let symbol_len = rest
            .find(|c: char| {
                c.is_alphanumeric()
                    || c == '_'
                    || c == '"'
                    || c == '\''
                    || c == '`'
                    || c == '/'
                    || c == '#'
            })
            .unwrap_or(rest.len());
        let chunk_len = if symbol_len == 0 { 1 } else { symbol_len };
        let (punct, remainder) = rest.split_at(chunk_len);
        spans.push((punct, Style::new().fg(Color::Reset)));
        rest = remainder;
    }

    spans
}

fn is_comment_start(s: &str, lang: &str) -> bool {
    match lang {
        "python" | "toml" | "yaml" | "sh" | "bash" | "zsh" => s.starts_with('#'),
        _ => s.starts_with("//") || s.starts_with("/*"),
    }
}

fn is_boolean_or_nil(s: &str) -> bool {
    matches!(
        s,
        "true" | "false" | "null" | "None" | "nil" | "undefined" | "True" | "False"
    )
}

fn is_type_name(s: &str, lang: &str) -> bool {
    // PascalCase standard convention for types across Rust, Go, Python, TS
    if let Some(first) = s.chars().next()
        && first.is_uppercase()
    {
        return true;
    }

    match lang {
        "rust" => matches!(
            s,
            "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "f32" | "f64" | "bool" | "char" | "str" | "Self"
        ),
        "go" => matches!(
            s,
            "int" | "int32" | "int64" | "uint" | "uint32" | "uint64" | "string" | "byte" | "error" | "float32" | "float64" | "bool"
        ),
        "c" | "cpp" | "java" => matches!(
            s,
            "int" | "long" | "float" | "double" | "void" | "char" | "bool" | "boolean" | "auto" | "size_t"
        ),
        "typescript" => matches!(s, "string" | "number" | "boolean" | "any" | "unknown" | "never" | "void"),
        _ => false,
    }
}

fn is_keyword(s: &str, lang: &str) -> bool {
    match lang {
        "rust" => matches!(
            s,
            "as" | "break" | "const" | "continue" | "crate" | "else" | "enum" | "extern" | "fn"
                | "for" | "if" | "impl" | "in" | "let" | "loop" | "match" | "mod" | "move"
                | "mut" | "pub" | "ref" | "return" | "self" | "static" | "struct" | "super"
                | "trait" | "type" | "unsafe" | "use" | "where" | "while" | "async" | "await"
                | "dyn"
        ),
        "python" => matches!(
            s,
            "def" | "class" | "return" | "import" | "from" | "as" | "if" | "elif" | "else"
                | "for" | "while" | "try" | "except" | "finally" | "with" | "async" | "await"
                | "yield" | "lambda" | "pass" | "raise" | "is" | "in" | "not" | "and" | "or"
        ),
        "toml" => matches!(s, "name" | "version" | "edition" | "dependencies" | "workspace"),
        "go" => matches!(
            s,
            "func" | "package" | "import" | "type" | "struct" | "interface" | "return" | "if"
                | "else" | "for" | "range" | "switch" | "case" | "default" | "var" | "const"
                | "go" | "chan" | "select" | "defer"
        ),
        "typescript" | "javascript" | "tsx" | "jsx" => matches!(
            s,
            "function" | "const" | "let" | "var" | "return" | "if" | "else" | "for" | "while"
                | "import" | "export" | "from" | "default" | "class" | "extends" | "interface"
                | "type" | "async" | "await" | "try" | "catch" | "finally" | "throw" | "new"
                | "this" | "typeof" | "instanceof"
        ),
        _ => matches!(
            s,
            "fn" | "func" | "def" | "class" | "struct" | "import" | "return" | "if" | "else"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_rust_line() {
        let line = "pub fn add(a: i32, b: i32) -> i32 {";
        let spans = highlight_line(line, "rust");
        assert!(spans.iter().any(|(word, s)| *word == "pub" && s.fg == Some(Color::Magenta)));
        assert!(spans.iter().any(|(word, s)| *word == "fn" && s.fg == Some(Color::Magenta)));
        assert!(spans.iter().any(|(word, s)| *word == "i32" && s.fg == Some(Color::Yellow)));
    }

    #[test]
    fn test_highlight_comment() {
        let line = "    // this is a comment";
        let spans = highlight_line(line, "rust");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].0, "    ");
        assert_eq!(spans[1].0, "// this is a comment");
        assert_eq!(spans[1].1.fg, Some(Color::DarkGray));
    }

    #[test]
    fn test_highlight_string() {
        let line = "let msg = \"hello world\";";
        let spans = highlight_line(line, "rust");
        assert!(spans.iter().any(|(tok, s)| *tok == "\"hello world\"" && s.fg == Some(Color::Green)));
    }
}
