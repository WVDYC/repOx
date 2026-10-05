/// Extracts architectural outline and signatures for C-style brace-delimited languages:
/// TypeScript, JavaScript, Go, C, C++, Java, Kotlin, Swift, C#.
pub fn extract_generic_c_outline(source: &str, lang: &str) -> String {
    let mut out = String::with_capacity(source.len() / 3);
    let mut in_body = false;
    let mut target_brace_depth: usize = 0;
    let mut current_brace_depth: usize = 0;

    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if in_body {
            let open_braces = line.chars().filter(|&c| c == '{').count();
            let close_braces = line.chars().filter(|&c| c == '}').count();
            current_brace_depth += open_braces;

            if current_brace_depth >= close_braces {
                current_brace_depth -= close_braces;
            } else {
                current_brace_depth = 0;
            }

            if current_brace_depth < target_brace_depth {
                in_body = false;
            }
            i += 1;
            continue;
        }

        let is_fn = is_generic_function_declaration(trimmed, lang);

        if is_fn {
            let mut sig = String::from(line);
            while !sig.contains('{') && !sig.contains(';') && i + 1 < lines.len() {
                i += 1;
                sig.push('\n');
                sig.push_str(lines[i]);
            }

            if sig.contains('{') {
                if let Some(brace_idx) = sig.find('{') {
                    let prefix = sig[..brace_idx].trim_end();
                    out.push_str(prefix);
                    out.push_str(" { /* ... */ }\n");

                    let rest = &sig[brace_idx..];
                    let open_count = rest.chars().filter(|&c| c == '{').count();
                    let close_count = rest.chars().filter(|&c| c == '}').count();

                    if open_count > close_count {
                        in_body = true;
                        target_brace_depth = current_brace_depth + 1;
                        current_brace_depth = current_brace_depth + open_count - close_count;
                    }
                } else {
                    out.push_str(&sig);
                    out.push('\n');
                }
            } else {
                out.push_str(&sig);
                out.push('\n');
            }
            i += 1;
            continue;
        }

        let open_braces = line.chars().filter(|&c| c == '{').count();
        let close_braces = line.chars().filter(|&c| c == '}').count();
        current_brace_depth = current_brace_depth.saturating_add(open_braces).saturating_sub(close_braces);

        out.push_str(line);
        out.push('\n');
        i += 1;
    }

    out
}

fn is_generic_function_declaration(trimmed: &str, lang: &str) -> bool {
    match lang {
        "go" => trimmed.starts_with("func "),
        "typescript" | "javascript" | "tsx" | "jsx" => {
            trimmed.starts_with("function ")
                || trimmed.starts_with("export function ")
                || trimmed.starts_with("async function ")
                || trimmed.starts_with("export async function ")
                || (trimmed.contains(" => ") && (trimmed.starts_with("const ") || trimmed.starts_with("export const ")))
                || (trimmed.ends_with('{') && (trimmed.starts_with("constructor(") || (trimmed.contains('(') && !trimmed.starts_with("class ") && !trimmed.starts_with("interface ") && !trimmed.starts_with("type "))))
        }
        "c" | "cpp" | "java" | "kotlin" | "swift" | "csharp" => {
            (trimmed.ends_with('{') || trimmed.contains('('))
                && !trimmed.starts_with("struct ")
                && !trimmed.starts_with("class ")
                && !trimmed.starts_with("interface ")
                && !trimmed.starts_with("enum ")
                && !trimmed.starts_with("#")
                && !trimmed.starts_with("//")
                && (trimmed.contains("public ")
                    || trimmed.contains("private ")
                    || trimmed.contains("protected ")
                    || trimmed.contains("void ")
                    || trimmed.contains("int ")
                    || trimmed.contains("bool ")
                    || trimmed.contains("auto ")
                    || trimmed.starts_with("fun ")
                    || trimmed.starts_with("func "))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_go_outline() {
        let code = r#"package main

import "fmt"

type Service struct {
    port int
}

func (s *Service) Start() error {
    fmt.Println("starting")
    return nil
}
"#;

        let outline = extract_generic_c_outline(code, "go");
        assert!(outline.contains("package main"));
        assert!(outline.contains("type Service struct"));
        assert!(outline.contains("func (s *Service) Start() error { /* ... */ }"));
        assert!(!outline.contains("fmt.Println"));
    }

    #[test]
    fn test_extract_ts_outline() {
        let code = r#"import { Request, Response } from 'express';

export interface User {
  id: string;
  name: string;
}

export function handleUser(req: Request): User {
  const token = req.headers['auth'];
  return { id: '1', name: 'Alice' };
}
"#;

        let outline = extract_generic_c_outline(code, "typescript");
        assert!(outline.contains("export interface User"));
        assert!(outline.contains("export function handleUser(req: Request): User { /* ... */ }"));
        assert!(!outline.contains("const token"));
    }
}
