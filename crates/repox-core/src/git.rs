use crate::error::{RepoxError, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Parses `git status --porcelain -uall` output into a set of repo-relative paths.
///
/// - When `staged_only` is `true`, only entries whose index status `X` is not `' '`, `'?'`, `'!'`, or `'D'`
///   (and not deleted in the worktree) are included.
/// - When `staged_only` is `false`, any modified, added, renamed, copied, or untracked (`'?'`) file
///   is included, excluding deleted (`'D'`) and ignored (`'!'`) files.
/// - Renamed entries (`XY ORIG -> PATH`) resolve to `PATH`, and surrounding double quotes are stripped.
pub fn parse_git_status(porcelain_output: &str, staged_only: bool) -> HashSet<PathBuf> {
    let mut files = HashSet::new();

    for line in porcelain_output.lines() {
        if line.len() < 4 {
            continue;
        }

        let bytes = line.as_bytes();
        let x = bytes[0] as char;
        let y = bytes[1] as char;
        let raw_path = &line[3..];

        let should_include = if staged_only {
            !matches!(x, ' ' | '?' | '!' | 'D') && y != 'D'
        } else {
            (x != ' ' || y != ' ') && !matches!(x, '!' | 'D') && !matches!(y, '!' | 'D')
        };

        if !should_include {
            continue;
        }

        let target_path = match raw_path.rsplit_once(" -> ") {
            Some((_, dest)) => dest.trim(),
            None => raw_path.trim(),
        };

        let clean_path = target_path
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(target_path);

        if !clean_path.is_empty() {
            files.insert(PathBuf::from(clean_path));
        }
    }

    files
}

/// Queries Git for modified/untracked or staged files relative to `root`.
pub fn get_git_changed_files(root: &Path, staged_only: bool) -> Result<HashSet<PathBuf>> {
    let toplevel_output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| {
            RepoxError::Format(format!(
                "Failed to execute git rev-parse in {}: {e}",
                root.display()
            ))
        })?;

    if !toplevel_output.status.success() {
        let stderr = String::from_utf8_lossy(&toplevel_output.stderr);
        return Err(RepoxError::Format(format!(
            "Not a git repository (or git failed) at {}: {}",
            root.display(),
            stderr.trim()
        )));
    }

    let repo_root_raw = PathBuf::from(String::from_utf8_lossy(&toplevel_output.stdout).trim());
    let canon_root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let canon_repo_root = fs::canonicalize(&repo_root_raw).unwrap_or(repo_root_raw);

    let status_output = Command::new("git")
        .arg("-C")
        .arg(&canon_repo_root)
        .args(["status", "--porcelain", "-uall"])
        .output()
        .map_err(|e| {
            RepoxError::Format(format!(
                "Failed to execute git status in {}: {e}",
                canon_repo_root.display()
            ))
        })?;

    if !status_output.status.success() {
        let stderr = String::from_utf8_lossy(&status_output.stderr);
        return Err(RepoxError::Format(format!(
            "git status failed in {}: {}",
            canon_repo_root.display(),
            stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&status_output.stdout);
    let repo_relative = parse_git_status(&stdout, staged_only);

    let mut relativized = HashSet::with_capacity(repo_relative.len());
    for rel_path in repo_relative {
        let abs_path = canon_repo_root.join(&rel_path);
        if let Ok(stripped) = abs_path.strip_prefix(&canon_root)
            && !stripped.as_os_str().is_empty()
        {
            relativized.insert(stripped.to_path_buf());
        }
    }

    Ok(relativized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_git_status_modified_and_untracked() {
        let output = [
            " M src/lib.rs",
            "M  src/main.rs",
            "MM src/both.rs",
            "A  src/added.rs",
            "?? src/untracked.rs",
            " D src/deleted_worktree.rs",
            "D  src/deleted_index.rs",
            "!! target/debug/repox",
        ]
        .join("\n");
        let changed = parse_git_status(&output, false);
        assert!(changed.contains(Path::new("src/lib.rs")));
        assert!(changed.contains(Path::new("src/main.rs")));
        assert!(changed.contains(Path::new("src/both.rs")));
        assert!(changed.contains(Path::new("src/added.rs")));
        assert!(changed.contains(Path::new("src/untracked.rs")));
        assert!(!changed.contains(Path::new("src/deleted_worktree.rs")));
        assert!(!changed.contains(Path::new("src/deleted_index.rs")));
        assert!(!changed.contains(Path::new("target/debug/repox")));
        assert_eq!(changed.len(), 5);
    }

    #[test]
    fn test_parse_git_status_staged_only() {
        let output = [
            " M src/unstaged.rs",
            "M  src/staged.rs",
            "MM src/partially_staged.rs",
            "A  src/added.rs",
            "?? src/untracked.rs",
            "D  src/deleted_staged.rs",
            "R  src/old.rs -> src/new.rs",
        ]
        .join("\n");
        let staged = parse_git_status(&output, true);
        assert!(!staged.contains(Path::new("src/unstaged.rs")));
        assert!(staged.contains(Path::new("src/staged.rs")));
        assert!(staged.contains(Path::new("src/partially_staged.rs")));
        assert!(staged.contains(Path::new("src/added.rs")));
        assert!(!staged.contains(Path::new("src/untracked.rs")));
        assert!(!staged.contains(Path::new("src/deleted_staged.rs")));
        assert!(staged.contains(Path::new("src/new.rs")));
        assert!(!staged.contains(Path::new("src/old.rs")));
        assert_eq!(staged.len(), 4);
    }

    #[test]
    fn test_parse_git_status_renamed_and_quoted() {
        let output = "\
R  \"old path.rs\" -> \"new path.rs\"
 M \"quoted file.rs\"
";
        let changed = parse_git_status(output, false);
        assert!(changed.contains(Path::new("new path.rs")));
        assert!(changed.contains(Path::new("quoted file.rs")));
        assert_eq!(changed.len(), 2);
    }
}
