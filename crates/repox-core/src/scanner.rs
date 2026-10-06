use crate::domain::{RepoFile, ScanOptions, ScanSummary};
use crate::error::{RepoxError, Result};
use crate::filter::{is_binary_content, should_skip_path};
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::{WalkBuilder, WalkState};
use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Internal helper for batched parallel directory walking.
struct ParallelCollector {
    local_buffer: Vec<PathBuf>,
    shared_entries: Arc<Mutex<Vec<PathBuf>>>,
    batch_size: usize,
}

impl ParallelCollector {
    fn new(shared: Arc<Mutex<Vec<PathBuf>>>, batch_size: usize) -> Self {
        Self {
            local_buffer: Vec::with_capacity(batch_size),
            shared_entries: shared,
            batch_size,
        }
    }

    fn push(&mut self, path: PathBuf) {
        self.local_buffer.push(path);
        if self.local_buffer.len() >= self.batch_size {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if self.local_buffer.is_empty() {
            return;
        }
        if let Ok(mut guard) = self.shared_entries.lock() {
            guard.extend(self.local_buffer.drain(..));
        }
    }
}

impl Drop for ParallelCollector {
    fn drop(&mut self) {
        self.flush();
    }
}

/// Builds compiled glob matchers for include and exclude patterns.
fn build_glob_matchers(
    exclude_patterns: &[String],
    include_patterns: &[String],
) -> Result<(Option<GlobSet>, Option<GlobSet>)> {
    let exclude_set = if !exclude_patterns.is_empty() {
        let mut builder = GlobSetBuilder::new();
        for pat in exclude_patterns {
            let glob = Glob::new(pat)
                .map_err(|e| RepoxError::Format(format!("Invalid exclude pattern '{pat}': {e}")))?;
            builder.add(glob);
        }
        Some(
            builder
                .build()
                .map_err(|e| RepoxError::Format(format!("Failed to build exclude matcher: {e}")))?,
        )
    } else {
        None
    };

    let include_set = if !include_patterns.is_empty() {
        let mut builder = GlobSetBuilder::new();
        for pat in include_patterns {
            let glob = Glob::new(pat)
                .map_err(|e| RepoxError::Format(format!("Invalid include pattern '{pat}': {e}")))?;
            builder.add(glob);
        }
        Some(
            builder
                .build()
                .map_err(|e| RepoxError::Format(format!("Failed to build include matcher: {e}")))?,
        )
    } else {
        None
    };

    Ok((exclude_set, include_set))
}

/// Executes high-performance multi-threaded repository scanning.
pub fn scan_repository(options: &ScanOptions) -> Result<(Vec<RepoFile>, ScanSummary)> {
    let start_time = Instant::now();
    let root = match fs::canonicalize(&options.root) {
        Ok(canon) => canon,
        Err(_) => options.root.clone(),
    };

    if !root.exists() {
        return Err(RepoxError::InvalidPath(format!(
            "Directory does not exist: {}",
            root.display()
        )));
    }

    let (exclude_matcher, include_matcher) =
        build_glob_matchers(&options.exclude_patterns, &options.include_patterns)?;

    // 1. Build and configure the multithreaded directory walker
    let mut builder = WalkBuilder::new(&root);
    builder
        .hidden(!options.include_hidden)
        .git_ignore(options.respect_gitignore)
        .git_global(options.respect_gitignore)
        .git_exclude(options.respect_gitignore)
        .ignore(options.respect_ignore)
        .max_depth(options.max_depth)
        .threads(options.threads);

    if let Some(ref custom_ignore) = options.repoxignore_name {
        builder.add_custom_ignore_filename(custom_ignore);
    }

    // 2. Traverse the repository filesystem using parallel visitor threads
    let walker = builder.build_parallel();
    let raw_entries = Arc::new(Mutex::new(Vec::new()));

    walker.run(|| {
        let shared = Arc::clone(&raw_entries);
        let mut collector = ParallelCollector::new(shared, 512);

        Box::new(move |entry_result| match entry_result {
            Ok(entry) => {
                if let Some(file_type) = entry.file_type()
                    && file_type.is_file()
                {
                    collector.push(entry.into_path());
                }
                WalkState::Continue
            }
            Err(err) => {
                tracing::debug!("Filesystem walk warning: {err}");
                WalkState::Continue
            }
        })
    });

    let paths = match Arc::try_unwrap(raw_entries) {
        Ok(mutex) => mutex.into_inner().unwrap_or_default(),
        Err(arc) => arc.lock().map(|g| g.clone()).unwrap_or_default(),
    };

    let total_scanned = paths.len();

    // 3. Process collected paths concurrently using rayon
    let root_ref = &root;
    let max_size = options.max_file_size;

    let mut accepted_files: Vec<RepoFile> = paths
        .into_par_iter()
        .filter_map(|full_path| {
            let relative_path = match full_path.strip_prefix(root_ref) {
                Ok(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
                _ => {
                    if full_path.is_file() {
                        full_path
                            .file_name()
                            .map(PathBuf::from)
                            .unwrap_or_else(|| full_path.clone())
                    } else {
                        full_path.clone()
                    }
                }
            };

            let is_lock = crate::filter::is_lockfile(&relative_path);

            // Default heuristic skip rules (lockfiles, svgs, minified, secrets, binary extensions)
            if is_lock {
                if !options.summary_locks {
                    return None;
                }
            } else if should_skip_path(&relative_path) {
                return None;
            }

            // User-specified glob filters
            if let Some(ref exclude) = exclude_matcher
                && exclude.is_match(&relative_path)
            {
                return None;
            }

            if let Some(ref include) = include_matcher
                && !include.is_match(&relative_path)
            {
                return None;
            }

            // File size check
            let metadata = match fs::metadata(&full_path) {
                Ok(meta) => meta,
                Err(err) => {
                    tracing::debug!("Failed to read metadata for {}: {err}", full_path.display());
                    return None;
                }
            };

            let file_size = metadata.len();
            if !is_lock
                && let Some(limit) = max_size
                && file_size > limit
            {
                tracing::debug!(
                    "Skipping {} due to size limit ({} > {})",
                    relative_path.display(),
                    file_size,
                    limit
                );
                return None;
            }

            // Read file bytes
            let bytes = match fs::read(&full_path) {
                Ok(b) => b,
                Err(err) => {
                    tracing::debug!("Failed to read file {}: {err}", full_path.display());
                    return None;
                }
            };

            // Inspect content bytes for binary signatures (NUL byte)
            if is_binary_content(&bytes) {
                return None;
            }

            // Convert to UTF-8
            let content = match String::from_utf8(bytes) {
                Ok(text) => text,
                Err(_) => {
                    // Non-UTF8 content is considered binary
                    return None;
                }
            };

            // When summary_locks is enabled, convert lockfiles into compact dependency manifests (.deps.txt)
            if is_lock && options.summary_locks {
                if let Some(summary) = crate::lockfile::summarize_lockfile(&relative_path, &content) {
                    let summary_len = summary.len() as u64;
                    if let Some(limit) = max_size
                        && summary_len > limit
                    {
                        return None;
                    }
                    let summary_path = relative_path.with_extension("deps.txt");
                    return Some(RepoFile::new(
                        summary_path,
                        full_path,
                        summary_len,
                        summary,
                    ));
                } else {
                    return None;
                }
            }

            let (final_content, final_size) = if options.outline {
                let dummy_file = RepoFile::new(
                    relative_path.clone(),
                    full_path.clone(),
                    file_size,
                    String::new(),
                );
                let lang = dummy_file.language_hint();
                let outline_text = crate::outline::extract_outline(&content, lang);
                let outline_len = outline_text.len() as u64;
                (outline_text, outline_len)
            } else {
                (content, file_size)
            };

            Some(RepoFile::new(
                relative_path,
                full_path,
                final_size,
                final_content,
            ))
        })
        .collect();

    // 4. Deterministic sorting for prompt reproducibility
    accepted_files.par_sort_unstable_by(|a, b| a.relative_path.cmp(&b.relative_path));

    let total_bytes = accepted_files.iter().map(|f| f.size_bytes).sum();
    let file_count = accepted_files.len();
    let elapsed = start_time.elapsed();

    let summary = ScanSummary {
        file_count,
        total_bytes,
        total_tokens: None,
        elapsed,
    };

    tracing::info!(
        "Scanned {} candidates, accepted {} files ({} bytes) in {:.2?}",
        total_scanned,
        file_count,
        total_bytes,
        elapsed
    );

    Ok((accepted_files, summary))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_scan_repository_filtering() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // 1. Normal source file
        let src_dir = root.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        let main_file = src_dir.join("main.rs");
        let mut f1 = File::create(&main_file).unwrap();
        writeln!(f1, "fn main() {{ println!(\"hello\"); }}").unwrap();

        // 2. Lockfile (should be skipped)
        let lock_file = root.join("Cargo.lock");
        let mut f2 = File::create(&lock_file).unwrap();
        writeln!(f2, "# Lockfile content").unwrap();

        // 3. SVG file (should be skipped)
        let svg_file = root.join("logo.svg");
        let mut f3 = File::create(&svg_file).unwrap();
        writeln!(f3, "<svg></svg>").unwrap();

        // 4. Sensitive file (should be skipped)
        let env_file = root.join(".env.production");
        let mut f4 = File::create(&env_file).unwrap();
        writeln!(f4, "DATABASE_URL=secret").unwrap();

        // 5. Binary file with NUL byte (should be skipped)
        let bin_file = root.join("data.dat");
        let mut f5 = File::create(&bin_file).unwrap();
        f5.write_all(&[0x48, 0x65, 0x00, 0x6c, 0x6f]).unwrap();

        // 6. Custom .repoxignore file
        let repoxignore = root.join(".repoxignore");
        let mut f6 = File::create(&repoxignore).unwrap();
        writeln!(f6, "ignored_dir/").unwrap();

        let ignored_dir = root.join("ignored_dir");
        fs::create_dir_all(&ignored_dir).unwrap();
        let ignored_file = ignored_dir.join("test.txt");
        let mut f7 = File::create(&ignored_file).unwrap();
        writeln!(f7, "This should be ignored by .repoxignore").unwrap();

        let options = ScanOptions::new(root);
        let (files, summary) = scan_repository(&options).unwrap();

        // Only main.rs should be accepted
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, PathBuf::from("src/main.rs"));
        assert!(files[0].content.contains("fn main()"));
        assert_eq!(summary.file_count, 1);
    }

    #[test]
    fn test_scan_repository_max_file_size() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let small_file = root.join("small.txt");
        fs::write(&small_file, "small content").unwrap();

        let big_file = root.join("big.txt");
        fs::write(&big_file, "a".repeat(10_000)).unwrap();

        let options = ScanOptions::new(root).with_max_file_size(Some(500));
        let (files, _) = scan_repository(&options).unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, PathBuf::from("small.txt"));
    }

    #[test]
    fn test_scan_repository_outline_mode() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let rust_code = r#"
pub struct User {
    pub id: u64,
}

impl User {
    pub fn new(id: u64) -> Self {
        let computed = id * 2;
        Self { id: computed }
    }
}
"#;
        let rs_file = root.join("user.rs");
        fs::write(&rs_file, rust_code).unwrap();

        let options = ScanOptions::new(root).with_outline(true);
        let (files, _) = scan_repository(&options).unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].content.contains("pub struct User"));
        assert!(
            files[0]
                .content
                .contains("pub fn new(id: u64) -> Self { /* ... */ }")
        );
        assert!(!files[0].content.contains("let computed = id * 2;"));
    }
}
