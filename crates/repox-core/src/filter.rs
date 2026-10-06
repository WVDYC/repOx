use std::path::Path;

/// Check if a given relative file path matches known lockfile names.
pub fn is_lockfile(path: &Path) -> bool {
    let file_name = match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name,
        None => return false,
    };

    matches!(
        file_name,
        "Cargo.lock"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "poetry.lock"
            | "composer.lock"
            | "Gemfile.lock"
            | "Pipfile.lock"
            | "flake.lock"
            | "mix.lock"
            | "pubspec.lock"
            | "bun.lockb"
            | "bun.lock"
            | "go.sum"
    )
}

/// Check if a given file has an SVG extension.
pub fn is_svg(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
}

/// Check if a given path corresponds to a minified file or source map.
pub fn is_minified_or_bundle(path: &Path) -> bool {
    let file_name = match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name.to_ascii_lowercase(),
        None => return false,
    };

    file_name.ends_with(".min.js")
        || file_name.ends_with(".min.css")
        || file_name.ends_with(".min.mjs")
        || file_name.ends_with(".min.cjs")
        || file_name == "bundle.js"
        || file_name.ends_with(".bundle.js")
        || file_name == "bundle.css"
        || file_name.ends_with(".bundle.css")
        || file_name.ends_with(".map")
}

/// Check if a given path matches sensitive credentials, secrets, or keys.
pub fn is_sensitive_path(path: &Path) -> bool {
    let file_name = match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name.to_ascii_lowercase(),
        None => return false,
    };

    if file_name.starts_with(".env") {
        return true;
    }

    if file_name.ends_with(".pem")
        || file_name.ends_with(".key")
        || file_name.ends_with(".keystore")
        || file_name.ends_with(".p12")
        || file_name.ends_with(".pfx")
        || file_name.ends_with(".cert")
        || file_name.ends_with(".crt")
    {
        return true;
    }

    if file_name.contains("id_rsa")
        || file_name.contains("id_ecdsa")
        || file_name.contains("id_ed25519")
        || file_name.contains("id_dsa")
    {
        return true;
    }

    matches!(
        file_name.as_str(),
        "credentials.json"
            | "secrets.json"
            | "secrets.yaml"
            | "secrets.yml"
            | "service-account.json"
    )
}

/// Known binary extensions to quickly bypass reading non-text files into memory.
pub fn has_binary_extension(path: &Path) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e.to_ascii_lowercase(),
        None => return false,
    };

    matches!(
        ext.as_str(),
        // Media
        "png" | "jpg" | "jpeg" | "gif" | "ico" | "webp" | "bmp" | "tiff" | "heic" |
        "mp3" | "wav" | "ogg" | "flac" | "mp4" | "mkv" | "avi" | "mov" | "webm" |
        // Binaries and Libraries
        "exe" | "dll" | "so" | "dylib" | "bin" | "wasm" | "o" | "a" | "lib" | "obj" |
        // Archives
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" | "zst" |
        // Bytecode / Runtime
        "pyc" | "pyo" | "class" | "jar" | "war" | "ear" |
        // Databases
        "db" | "sqlite" | "sqlite3" |
        // Fonts
        "ttf" | "otf" | "woff" | "woff2" | "eot" |
        // Documents
        "pdf" | "docx" | "xlsx" | "pptx"
    )
}

/// Fast heuristic inspection to detect binary data.
/// Uses the Git standard heuristic: inspecting the first 8,192 bytes for NUL (`\0`) bytes.
pub fn is_binary_content(buffer: &[u8]) -> bool {
    const INSPECT_LIMIT: usize = 8192;
    let inspect_slice = if buffer.len() > INSPECT_LIMIT {
        &buffer[..INSPECT_LIMIT]
    } else {
        buffer
    };

    // Git heuristic: presence of NUL byte strongly indicates binary content.
    inspect_slice.contains(&0)
}

/// Evaluates all default path-based skip rules.
pub fn should_skip_path(path: &Path) -> bool {
    is_lockfile(path)
        || is_svg(path)
        || is_minified_or_bundle(path)
        || is_sensitive_path(path)
        || has_binary_extension(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_lockfiles_are_skipped() {
        assert!(is_lockfile(Path::new("Cargo.lock")));
        assert!(is_lockfile(Path::new("deep/nested/package-lock.json")));
        assert!(is_lockfile(Path::new("pnpm-lock.yaml")));
        assert!(is_lockfile(Path::new("yarn.lock")));
        assert!(is_lockfile(Path::new("go.sum")));
        assert!(!is_lockfile(Path::new("Cargo.toml")));
        assert!(!is_lockfile(Path::new("package.json")));
    }

    #[test]
    fn test_svg_is_skipped() {
        assert!(is_svg(Path::new("assets/logo.svg")));
        assert!(is_svg(Path::new("ICON.SVG")));
        assert!(!is_svg(Path::new("logo.png")));
        assert!(!is_svg(Path::new("src/main.rs")));
    }

    #[test]
    fn test_minified_and_bundles_are_skipped() {
        assert!(is_minified_or_bundle(Path::new("dist/app.min.js")));
        assert!(is_minified_or_bundle(Path::new("styles/main.min.css")));
        assert!(is_minified_or_bundle(Path::new("dist/bundle.js")));
        assert!(is_minified_or_bundle(Path::new("dist/app.js.map")));
        assert!(!is_minified_or_bundle(Path::new("src/app.js")));
    }

    #[test]
    fn test_sensitive_patterns_are_skipped() {
        assert!(is_sensitive_path(Path::new(".env")));
        assert!(is_sensitive_path(Path::new(".env.local")));
        assert!(is_sensitive_path(Path::new(".env.production")));
        assert!(is_sensitive_path(Path::new("certs/server.pem")));
        assert!(is_sensitive_path(Path::new("ssh/id_rsa")));
        assert!(is_sensitive_path(Path::new("ssh/id_rsa.pub")));
        assert!(is_sensitive_path(Path::new("ssh/id_ed25519")));
        assert!(is_sensitive_path(Path::new("config/credentials.json")));
        assert!(!is_sensitive_path(Path::new("src/config.rs")));
    }

    #[test]
    fn test_binary_extensions_are_skipped() {
        assert!(has_binary_extension(Path::new("image.png")));
        assert!(has_binary_extension(Path::new("archive.tar.gz")));
        assert!(has_binary_extension(Path::new("target/debug/repox.dylib")));
        assert!(has_binary_extension(Path::new("build/app.wasm")));
        assert!(!has_binary_extension(Path::new("src/lib.rs")));
        assert!(!has_binary_extension(Path::new("README.md")));
    }

    #[test]
    fn test_binary_content_detection() {
        let text_bytes = b"fn main() { println!(\"hello world\"); }\n";
        assert!(!is_binary_content(text_bytes));

        let mut binary_bytes = vec![b'a'; 100];
        binary_bytes[50] = 0; // Insert NUL byte
        assert!(is_binary_content(&binary_bytes));
    }

    #[test]
    fn test_should_skip_path_aggregate() {
        assert!(should_skip_path(&PathBuf::from("nested/.env")));
        assert!(should_skip_path(&PathBuf::from("Cargo.lock")));
        assert!(should_skip_path(&PathBuf::from("icons/app.svg")));
        assert!(!should_skip_path(&PathBuf::from(
            "crates/repox-core/src/lib.rs"
        )));
    }
}
