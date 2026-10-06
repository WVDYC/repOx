use std::collections::BTreeSet;
use std::path::Path;

/// Summarizes a supported lockfile into a compact Markdown/text dependency manifest.
///
/// Returns `Some(manifest)` where each dependency is formatted as:
/// ```text
/// package @ version
/// ```
/// sorted alphabetically and deduplicated.
///
/// Supported lockfiles:
/// - `Cargo.lock` (Rust)
/// - `package-lock.json` (Node.js npm v1, v2, v3)
/// - `poetry.lock` (Python Poetry)
/// - `pnpm-lock.yaml` (Node.js pnpm v5, v6, v9)
/// - `go.sum` (Go)
/// - `yarn.lock` (Node.js Yarn v1, v2+)
///
/// Returns `None` if the file name is not a supported lockfile.
pub fn summarize_lockfile<P: AsRef<Path>>(path: P, content: &str) -> Option<String> {
    let file_name = path.as_ref().file_name().and_then(|s| s.to_str())?;

    let deps = match file_name {
        "Cargo.lock" => parse_cargo_lock(content),
        "package-lock.json" => parse_package_lock_json(content),
        "poetry.lock" => parse_poetry_lock(content),
        "pnpm-lock.yaml" => parse_pnpm_lock_yaml(content),
        "go.sum" => parse_go_sum(content),
        "yarn.lock" => parse_yarn_lock(content),
        _ => return None,
    };

    Some(format_dependencies(&deps))
}

/// Formats a list of (package, version) pairs into a compact dependency manifest.
pub fn format_dependencies(deps: &[(String, String)]) -> String {
    let mut out = String::new();
    for (name, version) in deps {
        out.push_str(name);
        out.push_str(" @ ");
        out.push_str(version);
        out.push('\n');
    }
    out
}

/// Parses a `Cargo.lock` file into sorted, unique `(package, version)` pairs.
pub fn parse_cargo_lock(content: &str) -> Vec<(String, String)> {
    parse_toml_package_blocks(content)
}

/// Parses a Python `poetry.lock` file into sorted, unique `(package, version)` pairs.
pub fn parse_poetry_lock(content: &str) -> Vec<(String, String)> {
    parse_toml_package_blocks(content)
}

/// Shared parser for TOML-based lockfiles (`Cargo.lock` and `poetry.lock`) using `[[package]]` blocks.
fn parse_toml_package_blocks(content: &str) -> Vec<(String, String)> {
    let mut packages = BTreeSet::new();
    let mut current_name: Option<String> = None;
    let mut current_version: Option<String> = None;
    let mut in_package = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') {
            if trimmed == "[[package]]" {
                if let (Some(name), Some(version)) = (current_name.take(), current_version.take()) {
                    packages.insert((name, version));
                }
                in_package = true;
            } else {
                if let (Some(name), Some(version)) = (current_name.take(), current_version.take()) {
                    packages.insert((name, version));
                }
                in_package = false;
            }
            continue;
        }

        if !in_package {
            continue;
        }

        if let Some((k, v)) = trimmed.split_once('=') {
            let key = k.trim();
            if key == "name" {
                if let Some(val) = extract_quoted_string(v.trim()) {
                    current_name = Some(val);
                }
            } else if key == "version"
                && let Some(val) = extract_quoted_string(v.trim())
            {
                current_version = Some(val);
            }
        }
    }

    if let (Some(name), Some(version)) = (current_name, current_version) {
        packages.insert((name, version));
    }

    packages.into_iter().collect()
}

/// Parses an npm `package-lock.json` file (supporting lockfileVersion 1, 2, and 3).
pub fn parse_package_lock_json(content: &str) -> Vec<(String, String)> {
    // 1. Try parsing modern npm lockfiles (v2/v3) with "packages" map
    let v2_v3_packages = parse_npm_packages_section(content);
    if !v2_v3_packages.is_empty() {
        return v2_v3_packages.into_iter().collect();
    }

    // 2. Fall back to npm v1 format with "dependencies" map
    parse_npm_dependencies_section(content)
        .into_iter()
        .collect()
}

/// Extracts dependencies from the `"packages"` object of npm lockfile v2/v3.
fn parse_npm_packages_section(content: &str) -> BTreeSet<(String, String)> {
    let mut packages = BTreeSet::new();
    let marker = "\"packages\"";
    let Some(packages_start) = content.find(marker) else {
        return packages;
    };
    let rest = &content[packages_start + marker.len()..];

    let node_modules_needle = "\"node_modules/";
    let mut cursor = 0;

    while let Some(idx) = rest[cursor..].find(node_modules_needle) {
        let entry_start = cursor + idx + node_modules_needle.len();
        let Some(quote_idx) = rest[entry_start..].find('"') else {
            break;
        };

        let full_module_path = &rest[entry_start..entry_start + quote_idx];
        // Handle nested node_modules (e.g., "foo/node_modules/bar")
        let pkg_name = match full_module_path.rfind("node_modules/") {
            Some(nm_pos) => &full_module_path[nm_pos + "node_modules/".len()..],
            None => full_module_path,
        };

        let after_key = entry_start + quote_idx;
        let next_entry = rest[after_key..]
            .find(node_modules_needle)
            .unwrap_or(rest.len() - after_key);
        let block_slice = &rest[after_key..after_key + next_entry];

        if let Some(version) = extract_json_field(block_slice, "version")
            && !pkg_name.is_empty()
        {
            packages.insert((pkg_name.to_string(), version));
        }

        cursor = after_key;
    }

    packages
}

/// Extracts dependencies from npm lockfile v1 `"dependencies"` sections.
fn parse_npm_dependencies_section(content: &str) -> BTreeSet<(String, String)> {
    let mut packages = BTreeSet::new();
    let marker = "\"dependencies\"";
    let Some(dep_start) = content.find(marker) else {
        return packages;
    };
    let slice = &content[dep_start + marker.len()..];

    // Scan lines for package keys and versions
    let mut current_name: Option<String> = None;

    for line in slice.lines() {
        let trimmed = line.trim();

        // Check if line declares a dependency entry: `"package-name": {`
        if trimmed.ends_with('{')
            && let Some(colon_idx) = trimmed.find(':')
        {
            let key_part = trimmed[..colon_idx].trim();
            if let Some(name) = extract_quoted_string(key_part)
                && name != "dependencies"
                && name != "requires"
                && name != "packages"
            {
                current_name = Some(name);
            }
        }

        // Check for version inside package entry
        if let Some(ref name) = current_name
            && trimmed.starts_with("\"version\"")
            && let Some(ver) = extract_json_field(trimmed, "version")
        {
            packages.insert((name.clone(), ver));
            current_name = None;
        }
    }

    packages
}

/// Parses a `pnpm-lock.yaml` file into sorted, unique `(package, version)` pairs.
pub fn parse_pnpm_lock_yaml(content: &str) -> Vec<(String, String)> {
    let mut packages = BTreeSet::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Look for package key patterns:
        // /@babel/core@7.23.0:
        // '@babel/core@7.23.0':
        // 'lodash@4.17.21':
        // /lodash@4.17.21:
        // react-dom@18.2.0(react@18.2.0):
        // lodash@4.17.21:
        if (trimmed.ends_with(':') || trimmed.ends_with("}:")) && trimmed.contains('@') {
            let mut key = trimmed.trim_end_matches(':').trim();
            // Strip quotes
            key = key.trim_matches('\'').trim_matches('"');
            // Strip optional leading slash
            key = key.strip_prefix('/').unwrap_or(key);

            // Strip peer dependency qualifier e.g. (react@18.2.0)
            if let Some(paren_idx) = key.find('(') {
                key = key[..paren_idx].trim();
            }

            // Find the last '@' separating package name from version
            // (handles scoped packages like @scope/pkg@1.0.0)
            if let Some(last_at) = key.rfind('@')
                && last_at > 0
            {
                let name = &key[..last_at];
                let mut version = &key[last_at + 1..];

                // Clean version from internal pnpm hashes (e.g. 1.0.0_peer-dep)
                if let Some((clean_ver, _)) = version.split_once('_') {
                    version = clean_ver;
                }

                if !name.is_empty() && !version.is_empty() {
                    packages.insert((name.to_string(), version.to_string()));
                }
            }
        }
    }

    packages.into_iter().collect()
}

/// Parses a Go `go.sum` file into sorted, unique `(package, version)` pairs.
pub fn parse_go_sum(content: &str) -> Vec<(String, String)> {
    let mut packages = BTreeSet::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let mut parts = trimmed.split_whitespace();
        let Some(pkg) = parts.next() else { continue };
        let Some(version_raw) = parts.next() else {
            continue;
        };

        // Strip the "/go.mod" suffix used for checksum verification entries
        let version = version_raw.strip_suffix("/go.mod").unwrap_or(version_raw);

        packages.insert((pkg.to_string(), version.to_string()));
    }

    packages.into_iter().collect()
}

/// Parses a `yarn.lock` file into sorted, unique `(package, version)` pairs.
pub fn parse_yarn_lock(content: &str) -> Vec<(String, String)> {
    let mut packages = BTreeSet::new();
    let mut current_name: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Header lines defining packages (not indented)
        if !line.starts_with(' ') && !line.starts_with('\t') && trimmed.ends_with(':') {
            let header = trimmed.trim_end_matches(':');
            // May contain multiple specs separated by comma: "foo@^1.0.0", "foo@^1.2.0":
            let first_spec = header.split(',').next().unwrap_or(header).trim();
            let clean_spec = first_spec.trim_matches('"').trim_matches('\'');

            // Find last '@' to extract package name
            if let Some(last_at) = clean_spec.rfind('@') {
                if last_at > 0 {
                    current_name = Some(clean_spec[..last_at].to_string());
                } else {
                    current_name = None;
                }
            } else {
                current_name = None;
            }
        } else if let Some(ref name) = current_name
            && trimmed.starts_with("version")
        {
            if let Some((_, ver_part)) = trimmed.split_once(' ') {
                let ver = ver_part.trim().trim_matches('"').trim_matches('\'');
                packages.insert((name.clone(), ver.to_string()));
                current_name = None;
            } else if let Some((_, ver_part)) = trimmed.split_once(':') {
                let ver = ver_part.trim().trim_matches('"').trim_matches('\'');
                packages.insert((name.clone(), ver.to_string()));
                current_name = None;
            }
        }
    }

    packages.into_iter().collect()
}

/// Helper to extract string between double quotes `"..."`.
fn extract_quoted_string(s: &str) -> Option<String> {
    let start = s.find('"')?;
    let rest = &s[start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Helper to extract a JSON string field e.g. `"version": "1.2.3"`.
fn extract_json_field(block: &str, field_name: &str) -> Option<String> {
    let target = format!("\"{field_name}\"");
    let f_idx = block.find(&target)?;
    let after_f = &block[f_idx + target.len()..];
    let colon_idx = after_f.find(':')?;
    let after_colon = after_f[colon_idx + 1..].trim_start();
    let quote_start = after_colon.strip_prefix('"')?;
    let quote_end = quote_start.find('"')?;
    Some(quote_start[..quote_end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cargo_lock_basic() {
        let content = r#"
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

[[package]]
name = "addr2line"
version = "0.25.1"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "1b5d307320b3181d6d7954e663bd7c774a838b8220fe0593c86d9fb09f498b4b"
dependencies = [
 "gimli",
]

[[package]]
name = "tokio"
version = "1.43.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "3d0161faee2702972cca10e7aac86c528569fb450e50f5a4d1a421b339ce868c"

[[package]]
name = "serde"
version = "1.0.219"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;

        let deps = parse_cargo_lock(content);
        assert_eq!(
            deps,
            vec![
                ("addr2line".to_string(), "0.25.1".to_string()),
                ("serde".to_string(), "1.0.219".to_string()),
                ("tokio".to_string(), "1.43.0".to_string()),
            ]
        );

        let manifest = summarize_lockfile("Cargo.lock", content).unwrap();
        assert_eq!(
            manifest,
            "addr2line @ 0.25.1\nserde @ 1.0.219\ntokio @ 1.43.0\n"
        );
    }

    #[test]
    fn test_parse_cargo_lock_multiple_versions() {
        let content = r#"
version = 4

[[package]]
name = "syn"
version = "1.0.109"

[[package]]
name = "syn"
version = "2.0.98"
"#;

        let deps = parse_cargo_lock(content);
        assert_eq!(
            deps,
            vec![
                ("syn".to_string(), "1.0.109".to_string()),
                ("syn".to_string(), "2.0.98".to_string()),
            ]
        );

        let manifest = summarize_lockfile("Cargo.lock", content).unwrap();
        assert_eq!(manifest, "syn @ 1.0.109\nsyn @ 2.0.98\n");
    }

    #[test]
    fn test_parse_package_lock_json_v3() {
        let content = r#"{
  "name": "my-web-app",
  "version": "1.0.0",
  "lockfileVersion": 3,
  "requires": true,
  "packages": {
    "": {
      "name": "my-web-app",
      "version": "1.0.0",
      "dependencies": {
        "react": "^18.2.0"
      }
    },
    "node_modules/react": {
      "version": "18.2.0",
      "resolved": "https://registry.npmjs.org/react/-/react-18.2.0.tgz",
      "integrity": "sha512-..."
    },
    "node_modules/@types/node": {
      "version": "20.1.0",
      "resolved": "https://registry.npmjs.org/@types/node/-/node-20.1.0.tgz"
    },
    "node_modules/some-parent/node_modules/lodash": {
      "version": "4.17.21"
    }
  }
}"#;

        let deps = parse_package_lock_json(content);
        assert_eq!(
            deps,
            vec![
                ("@types/node".to_string(), "20.1.0".to_string()),
                ("lodash".to_string(), "4.17.21".to_string()),
                ("react".to_string(), "18.2.0".to_string()),
            ]
        );

        let manifest = summarize_lockfile("package-lock.json", content).unwrap();
        assert_eq!(
            manifest,
            "@types/node @ 20.1.0\nlodash @ 4.17.21\nreact @ 18.2.0\n"
        );
    }

    #[test]
    fn test_parse_package_lock_json_v1() {
        let content = r#"{
  "name": "legacy-app",
  "version": "0.1.0",
  "lockfileVersion": 1,
  "dependencies": {
    "express": {
      "version": "4.18.2",
      "resolved": "https://registry.npmjs.org/express/-/express-4.18.2.tgz"
    },
    "@babel/core": {
      "version": "7.23.0"
    }
  }
}"#;

        let deps = parse_package_lock_json(content);
        assert_eq!(
            deps,
            vec![
                ("@babel/core".to_string(), "7.23.0".to_string()),
                ("express".to_string(), "4.18.2".to_string()),
            ]
        );

        let manifest = summarize_lockfile("package-lock.json", content).unwrap();
        assert_eq!(manifest, "@babel/core @ 7.23.0\nexpress @ 4.18.2\n");
    }

    #[test]
    fn test_parse_poetry_lock() {
        let content = r#"
[[package]]
name = "certifi"
version = "2023.7.22"
description = "Python package for providing Mozilla's CA Bundle."
optional = false
python-versions = ">=3.6"

[[package]]
name = "requests"
version = "2.31.0"
description = "Python HTTP for Humans."
"#;

        let deps = parse_poetry_lock(content);
        assert_eq!(
            deps,
            vec![
                ("certifi".to_string(), "2023.7.22".to_string()),
                ("requests".to_string(), "2.31.0".to_string()),
            ]
        );

        let manifest = summarize_lockfile("poetry.lock", content).unwrap();
        assert_eq!(manifest, "certifi @ 2023.7.22\nrequests @ 2.31.0\n");
    }

    #[test]
    fn test_parse_pnpm_lock_yaml() {
        let content = r#"
lockfileVersion: '6.0'

packages:
  /@babel/core@7.23.0:
    resolution: {integrity: sha512-...}
  '/lodash@4.17.21':
    resolution: {integrity: sha512-...}
  'react-dom@18.2.0(react@18.2.0)':
    resolution: {integrity: sha512-...}
"#;

        let deps = parse_pnpm_lock_yaml(content);
        assert_eq!(
            deps,
            vec![
                ("@babel/core".to_string(), "7.23.0".to_string()),
                ("lodash".to_string(), "4.17.21".to_string()),
                ("react-dom".to_string(), "18.2.0".to_string()),
            ]
        );

        let manifest = summarize_lockfile("pnpm-lock.yaml", content).unwrap();
        assert_eq!(
            manifest,
            "@babel/core @ 7.23.0\nlodash @ 4.17.21\nreact-dom @ 18.2.0\n"
        );
    }

    #[test]
    fn test_parse_go_sum() {
        let content = r#"
github.com/gin-gonic/gin v1.9.1 h1:4+fr/ElT2qcKFvmPLPnv4f4dRe7bm5hOchwv30M5h84=
github.com/gin-gonic/gin v1.9.1/go.mod h1:OasbgU5b+gD3fD9yR4f141I3...
golang.org/x/crypto v0.14.0 h1:wBqRN2Lt3hn7n7...
golang.org/x/crypto v0.14.0/go.mod h1:...
"#;

        let deps = parse_go_sum(content);
        assert_eq!(
            deps,
            vec![
                ("github.com/gin-gonic/gin".to_string(), "v1.9.1".to_string()),
                ("golang.org/x/crypto".to_string(), "v0.14.0".to_string()),
            ]
        );

        let manifest = summarize_lockfile("go.sum", content).unwrap();
        assert_eq!(
            manifest,
            "github.com/gin-gonic/gin @ v1.9.1\ngolang.org/x/crypto @ v0.14.0\n"
        );
    }

    #[test]
    fn test_summarize_lockfile_unsupported_file() {
        assert!(summarize_lockfile("main.rs", "fn main() {}").is_none());
        assert!(summarize_lockfile("Cargo.toml", "[package]").is_none());
    }
}
