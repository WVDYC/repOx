use crate::domain::{RepoFile, TokenProfile};
use crate::error::{RepoxError, Result};
use rayon::prelude::*;
use tiktoken_rs::{CoreBPE, cl100k_base, o200k_base};

/// Tokenizer wrapper supporting multiple LLM token encoding profiles.
pub struct TokenCounter {
    profile: TokenProfile,
    bpe: CoreBPE,
}

impl TokenCounter {
    /// Creates a new `TokenCounter` for the specified `TokenProfile`.
    pub fn new(profile: TokenProfile) -> Result<Self> {
        let bpe = match profile {
            TokenProfile::Cl100kBase | TokenProfile::Claude => cl100k_base()
                .map_err(|e| RepoxError::Tokenizer(format!("Failed to load cl100k_base: {e}")))?,
            TokenProfile::O200kBase | TokenProfile::DeepSeek | TokenProfile::Gemini => {
                o200k_base()
                    .map_err(|e| RepoxError::Tokenizer(format!("Failed to load o200k_base: {e}")))?
            }
        };

        Ok(Self { profile, bpe })
    }

    /// Counts tokens for a single text string according to the profile.
    pub fn count_tokens(&self, text: &str) -> usize {
        let base_count = self.bpe.encode_with_special_tokens(text).len();

        match self.profile {
            TokenProfile::Cl100kBase | TokenProfile::O200kBase => base_count,
            TokenProfile::Claude => {
                // Anthropic Claude tokenizer empirical calibration (~1.08x cl100k baseline for code & technical prompts)
                ((base_count as f64) * 1.08).round() as usize
            }
            TokenProfile::DeepSeek => {
                // DeepSeek V3/R1 BPE tokenization calibration
                ((base_count as f64) * 1.02).round() as usize
            }
            TokenProfile::Gemini => {
                // Google Gemini tokenization calibration
                ((base_count as f64) * 1.05).round() as usize
            }
        }
    }

    /// Concurrently calculates token counts for an entire slice of `RepoFile`s using `rayon`.
    pub fn count_files_tokens(&self, files: &mut [RepoFile]) -> usize {
        // Parallel map over files to populate cached token counts
        let counts: Vec<usize> = files
            .par_iter()
            .map(|file| self.count_tokens(&file.content))
            .collect();

        let mut total = 0;
        for (file, count) in files.iter_mut().zip(counts) {
            file.token_count = Some(count);
            total += count;
        }

        total
    }
}

/// Helper function to count tokens of a given text with a chosen profile.
pub fn count_text_tokens(text: &str, profile: TokenProfile) -> Result<usize> {
    let counter = TokenCounter::new(profile)?;
    Ok(counter.count_tokens(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_tokenizer_cl100k() {
        let text = "fn main() {\n    println!(\"Hello, world!\");\n}";
        let count = count_text_tokens(text, TokenProfile::Cl100kBase).unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_tokenizer_o200k() {
        let text = "fn main() {\n    println!(\"Hello, world!\");\n}";
        let count = count_text_tokens(text, TokenProfile::O200kBase).unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_tokenizer_claude() {
        let text = "fn main() {\n    println!(\"Hello, world!\");\n}";
        let count_cl100k = count_text_tokens(text, TokenProfile::Cl100kBase).unwrap();
        let count_claude = count_text_tokens(text, TokenProfile::Claude).unwrap();
        assert!(count_claude >= count_cl100k);
    }

    #[test]
    fn test_tokenizer_deepseek() {
        let text = "fn main() {\n    println!(\"Hello, DeepSeek!\");\n}";
        let count = count_text_tokens(text, TokenProfile::DeepSeek).unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_tokenizer_gemini() {
        let text = "fn main() {\n    println!(\"Hello, Gemini!\");\n}";
        let count = count_text_tokens(text, TokenProfile::Gemini).unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_count_files_tokens() {
        let mut files = vec![
            RepoFile::new(
                PathBuf::from("a.rs"),
                PathBuf::from("/a.rs"),
                10,
                "let a = 1;".to_string(),
            ),
            RepoFile::new(
                PathBuf::from("b.rs"),
                PathBuf::from("/b.rs"),
                10,
                "let b = 2;".to_string(),
            ),
        ];

        let counter = TokenCounter::new(TokenProfile::Cl100kBase).unwrap();
        let total = counter.count_files_tokens(&mut files);

        assert!(files[0].token_count.is_some());
        assert!(files[1].token_count.is_some());
        assert_eq!(
            total,
            files[0].token_count.unwrap() + files[1].token_count.unwrap()
        );
    }
}
