use std::path::PathBuf;
use std::time::Duration;

/// Represents an in-memory processed source code file from the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoFile {
    /// Relative path from the repository root (normalized with '/' separators).
    pub relative_path: PathBuf,
    /// Absolute canonical path on the host filesystem.
    pub absolute_path: PathBuf,
    /// Size of the raw file content in bytes.
    pub size_bytes: u64,
    /// UTF-8 decoded source code contents.
    pub content: String,
    /// Cached token count for this specific file, if calculated.
    pub token_count: Option<usize>,
}

impl RepoFile {
    /// Creates a new `RepoFile`.
    #[inline]
    pub fn new(
        relative_path: PathBuf,
        absolute_path: PathBuf,
        size_bytes: u64,
        content: String,
    ) -> Self {
        Self {
            relative_path,
            absolute_path,
            size_bytes,
            content,
            token_count: None,
        }
    }

    /// Formats the relative path using standard UNIX forward slashes for prompt consistency.
    pub fn display_path(&self) -> String {
        self.relative_path
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    }

    /// Guesses the syntax highlighting language identifier based on the file extension.
    pub fn language_hint(&self) -> &'static str {
        match self.relative_path.extension().and_then(|s| s.to_str()) {
            Some("rs") => "rust",
            Some("toml") => "toml",
            Some("yaml") | Some("yml") => "yaml",
            Some("json") => "json",
            Some("md") | Some("markdown") => "markdown",
            Some("js") | Some("mjs") | Some("cjs") => "javascript",
            Some("ts") | Some("mts") | Some("cts") => "typescript",
            Some("tsx") => "tsx",
            Some("jsx") => "jsx",
            Some("py") | Some("pyi") => "python",
            Some("go") => "go",
            Some("c") | Some("h") => "c",
            Some("cpp") | Some("hpp") | Some("cc") | Some("cxx") => "cpp",
            Some("java") => "java",
            Some("kt") | Some("kts") => "kotlin",
            Some("scala") => "scala",
            Some("swift") => "swift",
            Some("rb") => "ruby",
            Some("php") => "php",
            Some("sh") | Some("bash") | Some("zsh") => "bash",
            Some("sql") => "sql",
            Some("html") | Some("htm") => "html",
            Some("css") | Some("scss") | Some("sass") => "css",
            Some("lua") => "lua",
            Some("zig") => "zig",
            Some("dockerfile") | Some("Dockerfile") => "dockerfile",
            Some("proto") => "protobuf",
            _ => "",
        }
    }
}

/// Output prompt formats supported by repox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Claude-optimized XML format: `<repository_structure>` tree + `<file path="...">content</file>`.
    #[default]
    Xml,
    /// Markdown format: triple-backtick fenced blocks with language syntax hints.
    Markdown,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "xml" => Ok(Self::Xml),
            "markdown" | "md" => Ok(Self::Markdown),
            other => Err(format!(
                "Unknown format '{other}'. Supported formats: xml, markdown (md)"
            )),
        }
    }
}

/// Tokenizer profile for calculating context window consumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TokenProfile {
    /// OpenAI GPT-4 / ChatGPT profile (cl100k_base, 128k context).
    #[default]
    Cl100kBase,
    /// OpenAI GPT-4o / o1 / o3-mini modern Omni profile (o200k_base, 200k context).
    O200kBase,
    /// Anthropic Claude profile (Claude 3.7 / 3.5 Sonnet / Haiku, 200k context).
    Claude,
    /// DeepSeek profile (DeepSeek V3 / R1, 128k context).
    DeepSeek,
    /// Google Gemini profile (Gemini 2.0 / 2.5 Pro & Flash, 1M context).
    Gemini,
    /// Anthropic Claude Fable profile (Fable 5 / 5.1 Mythos-class, 1M context).
    Fable,
    /// OpenAI GPT Luna profile (GPT-5.6 / GPT-6 Luna, 1.05M context).
    Luna,
}

impl TokenProfile {
    /// Human-readable name of the model family this profile approximates.
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Cl100kBase => "GPT-4 Turbo",
            Self::O200kBase => "GPT-4o / o1 / o3",
            Self::Claude => "Claude 3.7 / 3.5 Sonnet",
            Self::DeepSeek => "DeepSeek V3 / R1",
            Self::Gemini => "Gemini 2.0 / 2.5 (1M)",
            Self::Fable => "Claude Fable 5.1 (1M)",
            Self::Luna => "GPT-6 Luna (1.05M)",
        }
    }

    /// Context window size (in tokens) of the model family this profile approximates.
    pub const fn context_window(self) -> usize {
        match self {
            Self::Cl100kBase => 128_000,
            Self::O200kBase => 200_000,
            Self::Claude => 200_000,
            Self::DeepSeek => 128_000,
            Self::Gemini => 1_000_000,
            Self::Fable => 1_000_000,
            Self::Luna => 1_050_000,
        }
    }
}

impl std::str::FromStr for TokenProfile {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "cl100k" | "cl100k_base" | "gpt4" | "gpt-4" => Ok(Self::Cl100kBase),
            "o200k" | "o200k_base" | "gpt4o" | "gpt-4o" | "o1" | "o3" | "o3-mini" => {
                Ok(Self::O200kBase)
            }
            "claude" | "claude37" | "claude-3.7" | "claude35" | "claude-3.5" | "anthropic" => {
                Ok(Self::Claude)
            }
            "fable" | "fable5" | "fable-5" | "fable51" | "claude-fable" => Ok(Self::Fable),
            "luna" | "gpt-luna" | "gpt6-luna" | "gpt-6-luna" | "gpt5-luna" => Ok(Self::Luna),
            "deepseek" | "r1" | "v3" | "deepseek-r1" | "deepseek-v3" => Ok(Self::DeepSeek),
            "gemini" | "gemini2" | "gemini-2.0" | "gemini-2.5" | "google" => Ok(Self::Gemini),
            other => Err(format!(
                "Unknown token profile '{other}'. Supported: fable, luna, claude, o1, o3-mini, gpt4o, deepseek, gemini"
            )),
        }
    }
}

/// Configurable scanner options for repository traversal and filtering.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Root path of the repository or directory to scan.
    pub root: PathBuf,
    /// Maximum directory recursion depth.
    pub max_depth: Option<usize>,
    /// Maximum file size allowed in bytes (larger files are skipped).
    pub max_file_size: Option<u64>,
    /// Respect `.gitignore` files.
    pub respect_gitignore: bool,
    /// Respect `.ignore` files.
    pub respect_ignore: bool,
    /// Name of custom ignore file (defaults to `Some(".repoxignore".into())`).
    pub repoxignore_name: Option<String>,
    /// Include hidden files and directories (names starting with '.').
    pub include_hidden: bool,
    /// Additional glob patterns to exclude.
    pub exclude_patterns: Vec<String>,
    /// Explicit glob patterns to include exclusively.
    pub include_patterns: Vec<String>,
    /// Number of worker threads for parallel file traversal and reading.
    pub threads: usize,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            max_depth: None,
            max_file_size: Some(1024 * 1024), // 1MB default threshold
            respect_gitignore: true,
            respect_ignore: true,
            repoxignore_name: Some(".repoxignore".to_string()),
            include_hidden: false,
            exclude_patterns: Vec::new(),
            include_patterns: Vec::new(),
            threads: std::thread::available_parallelism()
                .map(|p| p.get())
                .unwrap_or(4),
        }
    }
}

impl ScanOptions {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            ..Default::default()
        }
    }

    pub fn with_max_depth(mut self, depth: Option<usize>) -> Self {
        self.max_depth = depth;
        self
    }

    pub fn with_max_file_size(mut self, size: Option<u64>) -> Self {
        self.max_file_size = size;
        self
    }

    pub fn with_respect_gitignore(mut self, respect: bool) -> Self {
        self.respect_gitignore = respect;
        self
    }

    pub fn with_repoxignore(mut self, name: Option<String>) -> Self {
        self.repoxignore_name = name;
        self
    }

    pub fn with_include_hidden(mut self, include: bool) -> Self {
        self.include_hidden = include;
        self
    }

    pub fn with_exclude_patterns(mut self, patterns: Vec<String>) -> Self {
        self.exclude_patterns = patterns;
        self
    }

    pub fn with_include_patterns(mut self, patterns: Vec<String>) -> Self {
        self.include_patterns = patterns;
        self
    }

    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = if threads == 0 { 1 } else { threads };
        self
    }
}

/// Statistics and summary information for a scan operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanSummary {
    /// Total number of files scanned and accepted.
    pub file_count: usize,
    /// Total byte size of all accepted files.
    pub total_bytes: u64,
    /// Total token count across all accepted files (if computed).
    pub total_tokens: Option<usize>,
    /// Time taken to scan, filter, and process files.
    pub elapsed: Duration,
}
