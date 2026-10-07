use crate::domain::{RepoFile, TokenProfile};
use crate::error::Result;
use crate::outline::extract_outline;
use crate::tokenizer::TokenCounter;

/// Assigns a compression/pruning priority tier to a repository file.
///
/// Lower tier numbers are compressed (Phase 1) and pruned (Phase 2) first:
/// - Tier 0: Tests, benchmarks, examples, fixtures
/// - Tier 1: Documentation, markdown, and summarized lockfiles (`.deps.txt`)
/// - Tier 2: Configuration and data manifests
/// - Tier 3: Standard implementation source files
/// - Tier 4: Primary entrypoints (`lib.rs`, `main.rs`, `mod.rs`, `index.ts`, etc.)
fn compression_tier(file: &RepoFile) -> u8 {
    let path_str = file.display_path().to_ascii_lowercase();
    let file_name = file
        .relative_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // Tier 0: Tests, benchmarks, examples
    if path_str.starts_with("tests/")
        || path_str.starts_with("test/")
        || path_str.starts_with("benches/")
        || path_str.starts_with("bench/")
        || path_str.starts_with("examples/")
        || path_str.starts_with("example/")
        || path_str.contains("/tests/")
        || path_str.contains("/test/")
        || path_str.contains("/benches/")
        || path_str.contains("/examples/")
        || file_name.ends_with("_test.rs")
        || file_name.ends_with("_tests.rs")
        || file_name.ends_with("_test.go")
        || file_name.ends_with("_test.py")
        || file_name.starts_with("test_")
        || file_name.contains(".test.")
        || file_name.contains(".spec.")
    {
        return 0;
    }

    // Tier 1: Documentation and lockfile summaries
    if path_str.starts_with("docs/")
        || path_str.starts_with("doc/")
        || path_str.contains("/docs/")
        || file_name.ends_with(".md")
        || file_name.ends_with(".markdown")
        || file_name.ends_with(".txt")
        || file_name.ends_with(".rst")
        || file_name.ends_with(".deps.txt")
    {
        return 1;
    }

    // Tier 4: Primary architectural entrypoints (preserve longest)
    if matches!(
        file_name.as_str(),
        "lib.rs" | "main.rs" | "mod.rs" | "index.ts" | "index.js" | "app.py" | "main.go"
    ) {
        return 4;
    }

    // Tier 2: Configuration files
    if matches!(
        file.language_hint(),
        "json" | "yaml" | "toml" | "dockerfile"
    ) {
        return 2;
    }

    // Tier 3: Standard source code files
    3
}

/// Enforces a strict token budget across `files` using a two-phase strategy:
///
/// 1. **Phase 1 (Smart Outline Compression)**: Sorts candidate files by priority
///    (test/bench/example/doc files and largest files first) and converts their bodies
///    into architectural signatures via [`extract_outline`], recounting tokens until
///    the total fits within `max_tokens`.
/// 2. **Phase 2 (Graceful Pruning)**: If the repository still exceeds `max_tokens`
///    after outline compression, drops the lowest-priority / largest peripheral files
///    until total tokens <= `max_tokens`.
///
/// Returns the number of files that were compressed or pruned.
pub fn apply_token_budget(
    files: &mut Vec<RepoFile>,
    max_tokens: usize,
    profile: TokenProfile,
) -> Result<usize> {
    if files.is_empty() {
        return Ok(0);
    }

    let counter = TokenCounter::new(profile)?;

    let mut total_tokens = if files.iter().any(|f| f.token_count.is_none()) {
        counter.count_files_tokens(files)
    } else {
        files.iter().map(|f| f.token_count.unwrap_or(0)).sum()
    };

    if total_tokens <= max_tokens {
        return Ok(0);
    }

    let len = files.len();
    let mut affected = vec![false; len];

    // Order candidate indices: lowest priority tier first, then largest token count first.
    let mut candidates: Vec<usize> = (0..len).collect();
    candidates.sort_by(|&a, &b| {
        let tier_a = compression_tier(&files[a]);
        let tier_b = compression_tier(&files[b]);
        tier_a
            .cmp(&tier_b)
            .then_with(|| {
                let tok_a = files[a].token_count.unwrap_or(0);
                let tok_b = files[b].token_count.unwrap_or(0);
                tok_b.cmp(&tok_a)
            })
            .then_with(|| files[b].size_bytes.cmp(&files[a].size_bytes))
            .then_with(|| files[b].relative_path.cmp(&files[a].relative_path))
    });

    // Phase 1: Smart Outline Compression
    for &idx in &candidates {
        if total_tokens <= max_tokens {
            break;
        }

        let lang = files[idx].language_hint();
        let outlined = extract_outline(&files[idx].content, lang);

        if outlined.len() < files[idx].content.len() {
            let old_tokens = files[idx].token_count.unwrap_or(0);
            let mut single = vec![RepoFile::new(
                files[idx].relative_path.clone(),
                files[idx].absolute_path.clone(),
                outlined.len() as u64,
                outlined,
            )];
            let new_tokens = counter.count_files_tokens(&mut single);

            if new_tokens < old_tokens
                && let Some(updated_file) = single.pop()
            {
                files[idx] = updated_file;
                total_tokens = total_tokens.saturating_sub(old_tokens) + new_tokens;
                affected[idx] = true;
            }
        }
    }

    if total_tokens <= max_tokens {
        return Ok(affected.into_iter().filter(|&x| x).count());
    }

    // Phase 2: Graceful Pruning of lowest-priority / largest files
    let mut prune_order: Vec<usize> = (0..len).collect();
    prune_order.sort_by(|&a, &b| {
        let tier_a = compression_tier(&files[a]);
        let tier_b = compression_tier(&files[b]);
        tier_a
            .cmp(&tier_b)
            .then_with(|| {
                let tok_a = files[a].token_count.unwrap_or(0);
                let tok_b = files[b].token_count.unwrap_or(0);
                tok_b.cmp(&tok_a)
            })
            .then_with(|| files[b].size_bytes.cmp(&files[a].size_bytes))
            .then_with(|| files[b].relative_path.cmp(&files[a].relative_path))
    });

    let mut dropped = vec![false; len];
    for idx in prune_order {
        if total_tokens <= max_tokens {
            break;
        }
        let tok = files[idx].token_count.unwrap_or(0);
        dropped[idx] = true;
        affected[idx] = true;
        total_tokens = total_tokens.saturating_sub(tok);
    }

    let mut i = 0;
    files.retain(|_| {
        let keep = !dropped[i];
        i += 1;
        keep
    });

    Ok(affected.into_iter().filter(|&x| x).count())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_rust_file(path: &str, body_lines: usize) -> RepoFile {
        let mut content = String::from(
            "pub struct Engine {\n    pub id: u64,\n}\n\nimpl Engine {\n    pub fn compute(&self) -> u64 {\n",
        );
        for i in 0..body_lines {
            content.push_str(&format!(
                "        let step_{i} = self.id.wrapping_add({i});\n"
            ));
        }
        content.push_str("        self.id\n    }\n}\n");
        let len = content.len() as u64;
        RepoFile::new(PathBuf::from(path), PathBuf::from(path), len, content)
    }

    #[test]
    fn test_budget_no_op_when_under_limit() {
        let mut files = vec![make_rust_file("src/lib.rs", 5)];
        let affected = apply_token_budget(&mut files, 10_000, TokenProfile::Cl100kBase).unwrap();
        assert_eq!(affected, 0);
        assert_eq!(files.len(), 1);
        assert!(files[0].content.contains("let step_0"));
    }

    #[test]
    fn test_budget_phase1_smart_outline_compression() {
        let core_file = make_rust_file("src/lib.rs", 10);
        let heavy_test = make_rust_file("tests/integration_test.rs", 80);

        let counter = TokenCounter::new(TokenProfile::Cl100kBase).unwrap();
        let mut probe = vec![core_file.clone(), heavy_test.clone()];
        let initial_total = counter.count_files_tokens(&mut probe);

        // Set a budget that is smaller than initial_total, but large enough to hold
        // src/lib.rs in full + tests/integration_test.rs as an outline.
        let core_tokens = probe[0].token_count.unwrap();
        let target_budget = core_tokens + 60;
        assert!(target_budget < initial_total);

        let mut files = vec![core_file, heavy_test];
        let affected =
            apply_token_budget(&mut files, target_budget, TokenProfile::Cl100kBase).unwrap();

        assert_eq!(affected, 1);
        assert_eq!(files.len(), 2);

        // Core file remains untouched
        assert!(files[0].content.contains("let step_0"));
        // Test file was automatically compressed to an outline
        assert!(files[1].content.contains("/* ... */"));
        assert!(!files[1].content.contains("let step_0"));

        let final_total: usize = files.iter().map(|f| f.token_count.unwrap()).sum();
        assert!(final_total <= target_budget);
    }

    #[test]
    fn test_budget_phase2_graceful_pruning_when_outlines_still_exceed() {
        let core_file = make_rust_file("src/lib.rs", 10);
        let test_file = make_rust_file("tests/heavy_test.rs", 50);
        let doc_file = RepoFile::new(
            PathBuf::from("docs/guide.md"),
            PathBuf::from("docs/guide.md"),
            500,
            "Architecture guide ".repeat(40),
        );

        let mut files = vec![core_file, test_file, doc_file];
        // Very tight budget that only fits outlined src/lib.rs
        let affected = apply_token_budget(&mut files, 45, TokenProfile::Cl100kBase).unwrap();

        assert!(affected >= 2);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, PathBuf::from("src/lib.rs"));

        let final_total: usize = files.iter().map(|f| f.token_count.unwrap()).sum();
        assert!(final_total <= 45);
    }
}
