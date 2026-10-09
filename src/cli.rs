use clap::Parser;
use clap::builder::styling::{AnsiColor, Effects, Styles};
use repox_core::domain::{OutputFormat, ScanOptions, TokenProfile};
use std::path::PathBuf;

/// CLI styling for modern, beautiful terminal output.
fn cli_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Green.on_default() | Effects::BOLD)
        .usage(AnsiColor::Green.on_default() | Effects::BOLD)
        .literal(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Yellow.on_default())
}

/// Parse human-readable byte sizes (e.g., "500KB", "1.5MB", "1024").
pub fn parse_human_size(s: &str) -> std::result::Result<u64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Size cannot be empty".to_string());
    }

    let (num_part, unit_part) = match s.find(|c: char| c.is_alphabetic()) {
        Some(idx) => (&s[..idx], s[idx..].trim()),
        None => (s, ""),
    };

    let value: f64 = num_part
        .trim()
        .parse()
        .map_err(|e| format!("Invalid number '{num_part}': {e}"))?;

    if value < 0.0 {
        return Err("Size cannot be negative".to_string());
    }

    let multiplier = match unit_part.to_ascii_uppercase().as_str() {
        "" | "B" => 1.0,
        "K" | "KB" | "KIB" => 1024.0,
        "M" | "MB" | "MIB" => 1024.0 * 1024.0,
        "G" | "GB" | "GIB" => 1024.0 * 1024.0 * 1024.0,
        other => {
            return Err(format!(
                "Unknown unit '{other}'. Supported units: B, KB, MB, GB"
            ));
        }
    };

    Ok((value * multiplier) as u64)
}

/// Parse human-readable token budgets (e.g., "50000", "50k", "100K", "1m").
pub fn parse_token_budget(s: &str) -> std::result::Result<usize, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Token budget cannot be empty".to_string());
    }

    let (num_part, unit_part) = match s.find(|c: char| c.is_alphabetic()) {
        Some(idx) => (&s[..idx], s[idx..].trim()),
        None => (s, ""),
    };

    let value: f64 = num_part
        .trim()
        .parse()
        .map_err(|e| format!("Invalid token count '{num_part}': {e}"))?;

    if value <= 0.0 || !value.is_finite() {
        return Err("Token budget must be positive".to_string());
    }

    let multiplier = match unit_part.to_ascii_lowercase().as_str() {
        "" => 1.0,
        "k" => 1_000.0,
        "m" => 1_000_000.0,
        other => {
            return Err(format!(
                "Unknown token unit '{other}'. Supported units: k, m"
            ));
        }
    };

    Ok((value * multiplier) as usize)
}

#[derive(Parser, Debug)]
#[command(
    name = "repox",
    version,
    about = "High-performance CLI tool to pack source repositories into optimized LLM context prompts in milliseconds",
    long_about = "repox is a zero-runtime-dependency CLI & TUI tool built in Rust to traverse source repositories, \
filter out noise (lockfiles, binaries, SVGs, minified bundles, credentials), compute accurate offline token counts, \
and format the repository into LLM-optimized prompts (Claude XML or Markdown).",
    styles = cli_styles(),
)]
pub struct Cli {
    /// Target repository path or directory to pack.
    #[arg(default_value = ".", value_name = "PATH")]
    pub path: PathBuf,

    /// Output format for the generated prompt (xml, markdown / md, tool-call / json).
    #[arg(
        short = 'f',
        long = "format",
        default_value = "xml",
        value_name = "FORMAT",
        help = "Format template: 'xml' (Claude), 'markdown' / 'md', or 'tool-call' (agent JSON)"
    )]
    pub format: OutputFormat,

    /// Copy the formatted context directly to system clipboard.
    #[arg(
        short = 'c',
        long = "copy",
        help = "Copy output context directly to system clipboard"
    )]
    pub copy: bool,

    /// Save formatted output directly to the specified file.
    #[arg(
        short = 'o',
        long = "output",
        value_name = "FILE",
        help = "Write formatted context to an output file instead of stdout"
    )]
    pub output: Option<PathBuf>,

    /// Compute accurate token count for context window estimation.
    #[arg(
        short = 't',
        long = "tokens",
        help = "Calculate total token count using multi-threaded tiktoken tokenizer"
    )]
    pub tokens: bool,

    /// Tokenizer profile for token calculation.
    #[arg(
        short = 'p',
        long = "token-profile",
        default_value = "cl100k",
        value_name = "PROFILE",
        help = "Tokenizer profile: 'fable', 'luna', 'gemini', 'claude', 'o1', 'deepseek', 'llama', 'cl100k'"
    )]
    pub token_profile: TokenProfile,

    /// Maximum file size limit to pack (e.g. 500KB, 1MB, 2MB).
    #[arg(
        short = 's',
        long = "max-file-size",
        value_parser = parse_human_size,
        default_value = "1MB",
        value_name = "SIZE",
        help = "Skip files exceeding this size threshold (e.g. 500KB, 1.5MB)"
    )]
    pub max_file_size: u64,

    /// Maximum recursion depth for directory traversal.
    #[arg(
        short = 'd',
        long = "max-depth",
        value_name = "DEPTH",
        help = "Maximum directory nesting depth to traverse"
    )]
    pub max_depth: Option<usize>,

    /// Do not respect `.gitignore` rules during traversal.
    #[arg(long = "no-gitignore", help = "Disable respecting .gitignore files")]
    pub no_gitignore: bool,

    /// Do not respect `.repoxignore` rules during traversal.
    #[arg(
        long = "no-repoxignore",
        help = "Disable respecting .repoxignore files"
    )]
    pub no_repoxignore: bool,

    /// Include hidden files and directories (names starting with '.').
    #[arg(long = "include-hidden", help = "Include hidden files and folders")]
    pub include_hidden: bool,

    /// Additional glob pattern(s) to exclude from the prompt.
    #[arg(
        short = 'e',
        long = "exclude",
        value_name = "GLOB",
        help = "Glob pattern to exclude (can be specified multiple times)"
    )]
    pub exclude: Vec<String>,

    /// Explicit glob pattern(s) to include exclusively.
    #[arg(
        short = 'I',
        long = "include",
        value_name = "GLOB",
        help = "Glob pattern to include exclusively (can be specified multiple times)"
    )]
    pub include: Vec<String>,

    /// Number of worker threads for parallel file traversal and tokenization.
    #[arg(
        short = 'j',
        long = "threads",
        value_name = "N",
        help = "Number of worker threads (defaults to logical CPU core count)"
    )]
    pub threads: Option<usize>,

    /// Launch interactive Terminal UI (TUI) for file tree selection.
    #[arg(
        short = 'i',
        long = "interactive",
        alias = "tui",
        help = "Launch interactive terminal UI file picker (lazygit-style)"
    )]
    pub interactive: bool,

    /// Suppress stderr diagnostic messages (useful when piping stdout).
    #[arg(
        short = 'q',
        long = "quiet",
        help = "Silence informational statistics on stderr"
    )]
    pub quiet: bool,

    /// Enable verbose debug logging on stderr.
    #[arg(short = 'v', long = "verbose", help = "Enable verbose debug logs")]
    pub verbose: bool,

    /// Generate shell completions for the specified shell.
    #[arg(
        long = "completions",
        value_name = "SHELL",
        help = "Generate shell completions (bash, zsh, fish, powershell, elvish)"
    )]
    pub completions: Option<clap_complete::Shell>,

    /// Extract architectural outline/signatures only (strip implementation bodies).
    #[arg(
        long = "outline",
        alias = "signatures-only",
        help = "Extract architecture signatures and type outlines only (strips function bodies)"
    )]
    pub outline: bool,

    /// Summarize lockfiles into compact dependency manifests (.deps.txt).
    #[arg(
        long = "summary-locks",
        help = "Summarize lockfiles into compact dependency manifests instead of skipping them"
    )]
    pub summary_locks: bool,

    /// Only include Git modified and untracked files.
    #[arg(
        short = 'm',
        long = "modified",
        help = "Only pack Git modified and untracked files"
    )]
    pub modified: bool,

    /// Only include Git staged files.
    #[arg(long = "staged", help = "Only pack Git staged files")]
    pub staged: bool,

    /// Automatically compress/prune repository to fit within a strict token budget (e.g. 50k, 128k).
    #[arg(
        long = "budget",
        alias = "max-tokens",
        value_parser = parse_token_budget,
        value_name = "TOKENS",
        help = "Auto-compress (via outlines) and prune files to fit within token budget (e.g. 50k, 100k)"
    )]
    pub budget: Option<usize>,

    /// Automatically scan and redact inline secrets and API keys from file contents.
    #[arg(
        short = 'r',
        long = "redact-secrets",
        help = "Scan and redact inline secrets, API keys, and private keys with [REDACTED_SECRET]"
    )]
    pub redact_secrets: bool,

    /// Optimize a raw AI video prompt into structured <character_lock> and <shot> blocks plus paste-ready sampler prompts.
    #[arg(
        long = "video-prompt",
        value_name = "PROMPT",
        help = "Optimize an AI video generation prompt (Grok, Kling, Sora, Veo, Wan 2.1) with character lock & paste-ready clip prompts"
    )]
    pub video_prompt: Option<String>,

    /// Extract the sharpest tail frame from a video file via FFmpeg as a conditioning anchor PNG (<stem>.anchor.png).
    #[arg(
        long = "video-anchor",
        value_name = "VIDEO_FILE",
        help = "Extract the sharpest tail frame from a video clip via FFmpeg for shot-to-shot continuity (<stem>.anchor.png)"
    )]
    pub video_anchor: Option<PathBuf>,

    /// Explicit per-shot duration in seconds when optimizing AI video prompts.
    #[arg(
        long = "shot-duration",
        value_name = "SECONDS",
        help = "Explicit shot duration in seconds for --video-prompt (omitted if not specified)"
    )]
    pub shot_duration: Option<u32>,
}

impl Cli {
    /// Converts CLI options into `repox_core::ScanOptions`.
    pub fn to_scan_options(&self) -> ScanOptions {
        let threads = self.threads.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|p| p.get())
                .unwrap_or(4)
        });

        ScanOptions {
            root: self.path.clone(),
            max_depth: self.max_depth,
            max_file_size: Some(self.max_file_size),
            respect_gitignore: !self.no_gitignore,
            respect_ignore: true,
            repoxignore_name: if self.no_repoxignore {
                None
            } else {
                Some(".repoxignore".to_string())
            },
            include_hidden: self.include_hidden,
            exclude_patterns: self.exclude.clone(),
            include_patterns: self.include.clone(),
            threads,
            outline: self.outline,
            summary_locks: self.summary_locks,
            git_modified: self.modified,
            git_staged: self.staged,
            redact_secrets: self.redact_secrets,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_human_size() {
        assert_eq!(parse_human_size("500").unwrap(), 500);
        assert_eq!(parse_human_size("1KB").unwrap(), 1024);
        assert_eq!(parse_human_size("10k").unwrap(), 10 * 1024);
        assert_eq!(parse_human_size("1MB").unwrap(), 1024 * 1024);
        assert_eq!(
            parse_human_size("1.5MB").unwrap(),
            (1.5 * 1024.0 * 1024.0) as u64
        );
        assert_eq!(parse_human_size("1GB").unwrap(), 1024 * 1024 * 1024);
        assert!(parse_human_size("").is_err());
        assert!(parse_human_size("invalid").is_err());
        assert!(parse_human_size("100XYZ").is_err());
    }

    #[test]
    fn test_cli_defaults() {
        let cli = Cli::try_parse_from(["repox"]).unwrap();
        assert_eq!(cli.path, PathBuf::from("."));
        assert_eq!(cli.format, OutputFormat::Xml);
        assert_eq!(cli.token_profile, TokenProfile::Cl100kBase);
        assert_eq!(cli.max_file_size, 1024 * 1024);
        assert!(!cli.copy);
        assert!(!cli.tokens);
        assert!(!cli.interactive);
    }

    #[test]
    fn test_cli_interactive_flag() {
        let cli1 = Cli::try_parse_from(["repox", "-i"]).unwrap();
        assert!(cli1.interactive);

        let cli2 = Cli::try_parse_from(["repox", "--interactive"]).unwrap();
        assert!(cli2.interactive);

        let cli3 = Cli::try_parse_from(["repox", "--tui"]).unwrap();
        assert!(cli3.interactive);
    }

    #[test]
    fn test_cli_completions_flag() {
        let cli = Cli::try_parse_from(["repox", "--completions", "zsh"]).unwrap();
        assert_eq!(cli.completions, Some(clap_complete::Shell::Zsh));
    }

    #[test]
    fn test_cli_custom_flags() {
        let cli = Cli::try_parse_from([
            "repox",
            "src",
            "-f",
            "markdown",
            "-c",
            "-t",
            "-p",
            "claude",
            "-s",
            "500KB",
            "-d",
            "3",
            "--no-gitignore",
            "-e",
            "*.test.ts",
        ])
        .unwrap();

        assert_eq!(cli.path, PathBuf::from("src"));
        assert_eq!(cli.format, OutputFormat::Markdown);
        assert!(cli.copy);
        assert!(cli.tokens);
        assert_eq!(cli.token_profile, TokenProfile::Claude);
        assert_eq!(cli.max_file_size, 500 * 1024);
        assert_eq!(cli.max_depth, Some(3));
        assert!(cli.no_gitignore);
        assert_eq!(cli.exclude, vec!["*.test.ts"]);
    }

    #[test]
    fn test_cli_tool_call_format_and_outline() {
        let cli = Cli::try_parse_from(["repox", "-f", "tool-call", "--outline"]).unwrap();
        assert_eq!(cli.format, OutputFormat::ToolCall);
        assert!(cli.outline);
        let scan_opts = cli.to_scan_options();
        assert!(scan_opts.outline);

        let cli_json = Cli::try_parse_from(["repox", "-f", "json", "--signatures-only"]).unwrap();
        assert_eq!(cli_json.format, OutputFormat::ToolCall);
        assert!(cli_json.outline);
    }

    #[test]
    fn test_cli_git_modified_and_staged_flags() {
        let cli_mod = Cli::try_parse_from(["repox", "-m"]).unwrap();
        assert!(cli_mod.modified);
        assert!(!cli_mod.staged);
        let opts_mod = cli_mod.to_scan_options();
        assert!(opts_mod.git_modified);
        assert!(!opts_mod.git_staged);

        let cli_staged = Cli::try_parse_from(["repox", "--staged"]).unwrap();
        assert!(!cli_staged.modified);
        assert!(cli_staged.staged);
        let opts_staged = cli_staged.to_scan_options();
        assert!(!opts_staged.git_modified);
        assert!(opts_staged.git_staged);
    }

    #[test]
    fn test_parse_token_budget() {
        assert_eq!(parse_token_budget("50000").unwrap(), 50_000);
        assert_eq!(parse_token_budget("50k").unwrap(), 50_000);
        assert_eq!(parse_token_budget("100K").unwrap(), 100_000);
        assert_eq!(parse_token_budget("1m").unwrap(), 1_000_000);
        assert_eq!(parse_token_budget("1.5M").unwrap(), 1_500_000);
        assert!(parse_token_budget("").is_err());
        assert!(parse_token_budget("0").is_err());
        assert!(parse_token_budget("-10k").is_err());
        assert!(parse_token_budget("10xyz").is_err());
    }

    #[test]
    fn test_cli_budget_redact_and_video_prompt_flags() {
        let cli = Cli::try_parse_from([
            "repox",
            "--budget",
            "50k",
            "-r",
            "--video-prompt",
            "Please generate a video of a neon samurai walking in the rain.",
            "--video-anchor",
            "clip1.mp4",
            "--shot-duration",
            "5",
        ])
        .unwrap();

        assert_eq!(cli.budget, Some(50_000));
        assert!(cli.redact_secrets);
        assert_eq!(
            cli.video_prompt.as_deref(),
            Some("Please generate a video of a neon samurai walking in the rain.")
        );
        assert_eq!(cli.video_anchor, Some(PathBuf::from("clip1.mp4")));
        assert_eq!(cli.shot_duration, Some(5));

        let scan_opts = cli.to_scan_options();
        assert!(scan_opts.redact_secrets);

        let cli_alias =
            Cli::try_parse_from(["repox", "--max-tokens", "100K", "--redact-secrets"]).unwrap();
        assert_eq!(cli_alias.budget, Some(100_000));
        assert!(cli_alias.redact_secrets);
        assert!(cli_alias.video_anchor.is_none());
        assert!(cli_alias.shot_duration.is_none());
    }
}
