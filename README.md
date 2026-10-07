# repOx

<p align="center">
  <strong>High-performance CLI & TUI repository packer for LLM context prompts. Sub-30ms execution, zero runtime dependencies.</strong>
</p>

<p align="center">
  <a href="https://crates.io/crates/repox-cli"><img src="https://img.shields.io/crates/v/repox-cli.svg" alt="Crates.io"></a>
  <a href="https://crates.io/crates/repox-cli"><img src="https://img.shields.io/crates/d/repox-cli.svg" alt="Downloads"></a>
  <a href="https://github.com/WVDYC/repOx/actions"><img src="https://img.shields.io/badge/build-passing-brightgreen.svg" alt="Build Status"></a>
  <a href="#benchmarks"><img src="https://img.shields.io/badge/speed-sub--15ms-blueviolet.svg" alt="Speed"></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg" alt="License"></a>
</p>

<p align="center">
  <img src="assets/demo.gif" alt="repOx Interactive Terminal UI Demo" width="800">
</p>

---

## The Problem

You want Claude Fable, GPT-6 Luna, Gemini 3, or DeepSeek R1 to refactor a subsystem across your codebase. You run a quick script to dump your files, paste the output into the prompt, and realize:

1. **Context window pollution**: 40,000 tokens were burned on `Cargo.lock`, `package-lock.json`, minified bundles, SVG graphics, and binary artifacts.
2. **Sluggish tooling**: Existing Node.js/Python packers take 2–5 seconds, allocate 200MB of RAM, and still don't let you pick what to exclude.
3. **No tactile control**: You either pack everything or spend ten minutes crafting custom exclude flags.

**repOx** solves this. It scans multi-thousand-file repositories in milliseconds, strips noise with Git heuristics, provides a `lazygit`-style terminal UI for instant pruning, and copies clean, model-optimized prompts straight to your clipboard.

---

## Benchmarks

Measured using [`hyperfine`](https://github.com/sharkdp/hyperfine) on Apple Silicon (M-series) traversing a 3,000+ file repository (15 runs, 3 warmups):

| Tool | Average Latency | Speedup | Memory | Interactive TUI |
| :--- | :--- | :--- | :--- | :--- |
| **`repox`** (Claude XML) | **14.2 ms ± 0.8 ms** | **1.0x (baseline)** | **~18 MB** | **Yes (`-i`)** |
| **`repox -t -p claude`** | **42.6 ms ± 1.4 ms** | **~3.0x slower** | **~42 MB** | **Yes (live gauge)** |
| `repomix` (`npx repomix`) | **1,850.4 ms ± 48.2 ms** | **~130x slower** | **~185 MB** | No |
| `files-to-prompt` (Python) | **620.1 ms ± 18.0 ms** | **~43x slower** | **~54 MB** | No |

---

## Comparison Matrix & v0.3.0 Features

| Feature | `repox` (v0.3.0) | `repomix` | `gitingest` | `files-to-prompt` |
| :--- | :--- | :--- | :--- | :--- |
| **Language & Runtime** | Rust (static binary) | Node.js (requires npm) | Python (web-first) | Python (pip) |
| **Interactive Terminal UI** | **Yes (`ratatui` + syntax highlighting)** | No | Web browser only | No |
| **Auto Token Budget Fitter** | **Yes (`-b, --budget` / `--max-tokens`)** | No | No | No |
| **AI Video & ComfyUI Prompt Compiler** | **Yes (`--video-prompt` + ComfyUI JSON minifier)** | No | No | No |
| **Streaming Secret Redaction** | **Yes (`-r, --redact-secrets`)** | Basic | No | No |
| **Architectural Signature Extraction** | **Yes (`--outline` / `--signatures-only`)** | Experimental | No | No |
| **Synthetic Agent Tool-Call Output** | **Yes (`-f tool-call` / `-f json`)** | No | No | No |
| **Smart Lockfile Summarizer** | **Yes (`--summary-locks`)** | No | No | No |
| **Git-Aware Diff Context Packing** | **Yes (`-m, --modified`, `--staged`)** | No | No | No |
| **SSH / Tmux Clipboard Fallback** | **Yes (`arboard` + OSC 52)** | No | N/A | No |
| **Offline Token Calculation** | **Yes (`tiktoken-rs`)** | Yes | Approximate | No |
| **Real-time Context Budget** | **Yes (live gauge)** | No | No | No |
| **Fuzzy File Search** | **Yes (`nucleo-matcher`)** | No | Browser find | No |
| **Git NUL-byte Binary Detection** | **Yes (8KB check)** | Basic extension check | Basic | None |
| **Secret & Lockfile Filtering** | **Automatic** | Config-based | Basic | None |
| **UNIX Pipe Friendly** | **Yes (TUI on stderr)** | Partial | Web download | Yes |

---

## Installation

### Via Cargo (Recommended)

```bash
cargo install repox-cli
```

### One-Line Installer (macOS & Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/WVDYC/repOx/main/install.sh | sh
```

### Homebrew (macOS & Linux)

```bash
brew tap WVDYC/tap https://github.com/WVDYC/repOx
brew install repox
```

### Cargo (From Source)

Requires Rust 1.85+ (2024 edition):

```bash
cargo install --git https://github.com/WVDYC/repOx.git
```

Or build locally:

```bash
git clone https://github.com/WVDYC/repOx.git
cd repOx
cargo build --release
# Binary: ./target/release/repox
```

### Shell Completions

`repOx` generates native completions for all major shells:

```bash
# Zsh
repox --completions zsh > ~/.zsh/completion/_repox

# Bash
repox --completions bash > ~/.local/share/bash-completion/completions/repox

# Fish
repox --completions fish > ~/.config/fish/completions/repox.fish
```

---

## Interactive TUI (lazygit for Prompts)

Launch the terminal interface with `repox -i` or `repox --tui`. Features zero-allocation rendering, lexical syntax highlighting in the live preview pane, and native + OSC 52 SSH/tmux clipboard integration.

```text
┌── Files ──────────────────────────────────┐┌── Preview: src/main.rs ──────────────┐
│ ▸ [x] crates/                             ││ 1 │ use clap::Parser;                 │
│ ▾ [x] src/                                ││ 2 │ use color_eyre::eyre::Result;     │
│   [x] cli.rs                     7.1 KB   ││ 3 │                                   │
│   [x] main.rs                    3.8 KB   ││ 4 │ fn main() -> Result<()> {         │
│   [x] Cargo.toml                 1.4 KB   ││ 5 │     // fast parallel walk...      │
└───────────────────────────────────────────┘└───────────────────────────────────────┘
  [Claude Fable 5.1 (1M)] 26/26 files | 45,260 / 1,000,000 tokens (4.5%) [■░░░░░░░░░] 166.0 KB
  [↑/↓] Move  [Space] Toggle  [/] Search  [a] Invert  [c] Copy  [C] CLI Cmd  [Enter] Dump  [q] Quit
```

### Keybindings

| Key | Action |
| :--- | :--- |
| `↑` / `k` or `↓` / `j` | Move selection cursor up / down |
| `←` / `h` or `→` / `l` | Collapse / Expand folder |
| `Space` | Toggle file or folder (cascading tri-state checkboxes: `[ ]`, `[-]`, `[x]`) |
| `a` | Toggle all / invert selection |
| `/` | Open live fuzzy search filter (`nucleo-matcher`) |
| `Esc` | Clear search query / reset filter |
| `c` | **Copy to clipboard**: format selected files (via system clipboard or OSC 52 over SSH/tmux) and exit |
| `C` (`Shift+C`) | **Copy reproducible CLI command**: copy exact `repox \` invocation (`-I file1 -I file2`) to clipboard and exit |
| `Enter` | On a file: dump context to stdout/file and exit; on a folder: toggle expand/collapse |
| `q` | Abort and quit without action |

---

## Quickstart & Common Workflows

### 1. Interactive selection with live token budgeting
```bash
repox -i -p fable
# Also supports: -p luna, -p gemini, -p deepseek, -p o1, -p claude, -p llama
```
Launches the TUI calibrated against the target model's context window (e.g. 1,000,000 tokens for Fable/Gemini, 1.05M for Luna).

### 2. Copy entire codebase to clipboard (headless)
```bash
repox -c
```
Copies Claude-formatted XML prompt directly to your system clipboard (automatically falls back to OSC 52 escape sequences when running inside remote SSH or `tmux` sessions):
```text
✓ Copied to clipboard: 42 files (142.6 KB) in 4.12ms
```

### 3. Include accurate offline token counts
```bash
repox -c -t -p claude
```
```text
✓ Copied to clipboard: 42 files (142.6 KB, 38,120 tokens) in 38.50ms
```

### 4. Save formatted Markdown to file
```bash
repox -f md -o context.md
```

### 5. Filter by subfolder, max file size, and depth
```bash
repox src/ -s 500KB -d 3 -o prompt.xml
```

### 6. Pipe clean prompt into another tool
```bash
repox -q | pbcopy
```

### 7. Architectural Outline / Signatures Only (`--outline` / `--signatures-only`)
```bash
repox --outline -c
```
Extracts type definitions, structs, traits, interfaces, classes, and function signatures across **Rust, Python, Go, TypeScript/JavaScript, C/C++, and Java** while stripping implementation bodies into `{ /* ... */ }`. Compresses multi-thousand-line codebases into compact token budgets (60–90% token reduction) ideal for system architecture planning and LLM codebase mapping.

### 8. Synthetic Tool-Call Output (`-f tool-call` / `-f json`)
```bash
repox -f tool-call -c
```
Formats files as an array of JSON `read_file` tool responses. Perfect for LLM agent harnesses (Ollama, OpenAI, Anthropic message loops) where models treat tool outputs as authoritative ground truth rather than user-pasted text.

### 9. Smart Lockfile & ComfyUI Workflow Summarizer (`--summary-locks`)
```bash
repox --summary-locks -c
```
Instead of completely skipping huge lockfiles (`Cargo.lock`, `package-lock.json`, `pnpm-lock.yaml`, `poetry.lock`, `yarn.lock`, `go.sum`) or burning 40,000+ tokens on raw dependency graphs and ComfyUI visual workflow JSON files, `--summary-locks` distills lockfiles into a compact `pkg @ version` manifest (`.deps.txt`) and compresses 30,000-token ComfyUI workflow graphs into sorted node-and-prompt manifests (85–95% token reduction).

### 10. Git-Aware Diff Context Packing (`-m, --modified` & `--staged`)
```bash
# Pack only modified/untracked working-tree files for a focused debugging prompt
repox -m -c

# Pack only staged files along with a unified git diff for PR description or code review
repox --staged -c
```
Queries Git status and `git diff` to pack only changed files alongside their unified diff patch, giving LLMs both the exact line changes and full surrounding file context for PR reviews and bug fixes.

### 11. Automatic Token Budget Fitter (`-b, --budget` / `--max-tokens`)
```bash
repox --budget 50k -c
# Supports exact integers or shorthand suffixes: 32k, 64k, 100k, 1m
```
Automatically fits any repository strictly inside a target token budget. `repOx` prioritizes core entrypoints (`main.rs`, `lib.rs`, `index.ts`) and Git-modified files, progressively outline-compresses large secondary modules into architectural signatures, and drops lowest-priority test/fixture payloads only if necessary.

### 12. Streaming Secret Redaction (`-r, --redact-secrets`)
```bash
repox -r -c
```
Runs a single-pass lexical scanner over all packed files to detect and replace hardcoded credentials—including OpenAI keys (`sk-...`, `sk-proj-...`), Anthropic keys (`sk-ant-...`), GitHub tokens (`ghp_...`, `gho_...`, `github_pat_...`), AWS Access Key IDs (`AKIA...`), and PEM/OpenSSH `-----BEGIN ... PRIVATE KEY-----` blocks—with `[REDACTED_SECRET]`.

### 13. AI Video Prompt & Continuity Compiler (`--video-prompt`)
```bash
repox --video-prompt "Please generate a video of a cyberpunk courier in a yellow trench coat on 35mm anamorphic lens in neon rain. Then she leaps across a rooftop as a drone tracks her." -c
```
Compiles natural-language scene descriptions into structured, attention-maximizing multi-shot prompts for **Grok Video, Kling, Sora, Veo, and Wan 2.1**. Strips conversational filler, synthesizes a `<character_lock>` block (`subject`, `lens`, `lighting`), and splits scenes into 5-second `<shot>` blocks with explicit `[Camera]`, `[Motion]`, and `[Continuity: anchor=sharpest_tail_frame]` directives backed by a <1ms 3x3 Laplacian variance sharpness scorer.

---

## Output Formats

### Claude XML Output (`-f xml`, Default)

Includes an ASCII directory tree followed by cleanly tagged file blocks:

```xml
<repository_structure>
.
├── Cargo.toml
├── crates
│   └── repox-core
│       └── src
│           └── lib.rs
└── src
    └── main.rs
</repository_structure>

<file path="Cargo.toml">
[workspace]
members = ["crates/*"]
</file>

<file path="src/main.rs">
fn main() { ... }
</file>
```

### Markdown Output (`-f md`)

Uses collision-safe backtick fencing (automatically escapes nested backticks):

````markdown
# Repository Structure

```
.
├── Cargo.toml
└── src
    └── main.rs
```

# Repository Files

## File: Cargo.toml

```toml
[workspace]
...
```

## File: src/main.rs

```rust
fn main() { ... }
```
````

### Synthetic Tool-Call Output (`-f tool-call` / `-f json`)

Outputs an authoritative JSON array formatted as tool responses for agent harnesses:

```json
[
  {
    "role": "tool",
    "tool_call_id": "call_read_file_1",
    "name": "read_file",
    "path": "src/main.rs",
    "content": "fn main() { ... }"
  }
]
```

---

## Architecture & Internals

`repOx` is engineered around three core principles: **zero-allocation rendering**, **deterministic output**, and **fail-safe ergonomics**.

### 1. Contiguous FlatTree Arena (`crates/repox-tui/src/tree.rs`)
Recursive tree widgets frequently allocate on every frame and suffer from cache thrashing. `repOx` flattens the file hierarchy into a single contiguous pre-order `Vec<Node>`:
- Subtrees occupy contiguous slices `[start..end]`.
- Expanding or collapsing is an $O(1)$ slice mask.
- Toggling selection recalculates folder aggregates in a single reverse-slice sweep with zero allocations in the render loop.

### 2. Multi-Threaded Work-Stealing Traversal (`crates/repox-core/src/scanner.rs`)
- Uses `ignore::WalkBuilder` with lock-free channel batching.
- Automatically respects `.gitignore`, `.ignore`, and `.repoxignore`.
- Evaluates binary content via Git's 8KB NUL-byte heuristic and drops lockfiles (unless `--summary-locks` is enabled), minified bundles, SVGs, and secret keys (`.env*`, `*pem`, `*id_rsa*`) before reading full payloads into memory.

### 3. TUI via `stderr` for Clean Unix Pipelines
The interactive TUI renders exclusively to `stderr`. This means `stdout` is never corrupted by escape codes, allowing clean composition:
```bash
repox -i | llm prompt "explain the architecture"
```

### 4. Terminal Safety & OSC 52 Clipboard Fallback
Uses an RAII `TerminalGuard` with a panic hook to guarantee that raw mode is disabled and the alternate screen is exited even if an abnormal termination occurs. Clipboard operations use `arboard` with automatic fallback to ANSI OSC 52 escape sequences for seamless copying over headless SSH and `tmux`.

---

## CLI Reference

```text
repox [OPTIONS] [PATH]

Arguments:
  [PATH]                     Target repository directory [default: .]

Options:
  -i, --interactive          Launch interactive terminal UI file picker (alias: --tui)
  -f, --format <FORMAT>      Format template: 'xml' (Claude), 'markdown' / 'md', or 'tool-call' / 'json' [default: xml]
  -c, --copy                 Copy output context directly to system clipboard (supports OSC 52 fallback)
  -o, --output <FILE>        Write formatted context to an output file instead of stdout
  -t, --tokens               Calculate total token count using multi-threaded tiktoken tokenizer
  -p, --token-profile <PROF> Tokenizer profile: 'fable' (Claude Fable 5.1, 1M), 'luna' (GPT-6 Luna, 1.05M), 'claude', 'o1', 'deepseek', 'gemini', 'llama', 'cl100k' [default: cl100k]
  -b, --budget <TOKENS>      Automatically fit repository into a strict token budget (e.g. 50k, 100000; alias: --max-tokens)
  -r, --redact-secrets       Scan and mask hardcoded API keys, tokens, and private keys with [REDACTED_SECRET]
      --video-prompt <TEXT>  Compile a raw video prompt into a <character_lock> + 5s <shot> continuity prompt
      --outline              Extract architectural signatures and type outlines only (alias: --signatures-only)
      --summary-locks        Summarize lockfiles and ComfyUI workflow JSONs into compact manifests
  -m, --modified             Pack only Git modified and untracked working-tree files with unified diff context
      --staged               Pack only Git staged files with unified diff context for PRs and reviews
  -s, --max-file-size <SIZE> Skip files exceeding size threshold (e.g. 500KB, 1.5MB) [default: 1MB]
  -d, --max-depth <DEPTH>    Maximum directory nesting depth to traverse
      --no-gitignore         Disable respecting .gitignore files
      --no-repoxignore       Disable respecting .repoxignore files
      --include-hidden       Include hidden files and folders
  -e, --exclude <GLOB>       Glob pattern to exclude (can be specified multiple times)
  -I, --include <GLOB>       Glob pattern to include exclusively (can be specified multiple times)
  -j, --threads <N>          Worker threads count (defaults to logical CPU core count)
      --completions <SHELL>  Generate shell completions (bash, zsh, fish, powershell, elvish)
  -q, --quiet                Silence informational statistics on stderr
  -v, --verbose              Enable verbose debug logs
  -h, --help                 Print help
  -V, --version              Print version
```

---

## Testing

```bash
cargo test --workspace
```

---

## License

Dual-licensed under either:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
