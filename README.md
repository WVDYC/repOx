# ⚡ repOx

<p align="center">
  <strong>High-performance CLI & TUI tool to pack source repositories into optimized LLM context prompts in milliseconds.</strong>
</p>

<p align="center">
  <a href="https://github.com/WVDYC/repOx/actions"><img src="https://img.shields.io/badge/build-passing-brightgreen.svg" alt="Build Status"></a>
  <a href="https://crates.io"><img src="https://img.shields.io/badge/rust-2024_edition-orange.svg" alt="Rust 2024"></a>
  <a href="#benchmarks"><img src="https://img.shields.io/badge/speed-sub--30ms-blueviolet.svg" alt="Speed"></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg" alt="License"></a>
</p>

---

## 🚀 Overview

`repOx` is a zero-runtime-dependency CLI & TUI engineered in Rust designed to solve **LLM context window bloat**. It rapidly scans repositories, strips noise (lockfiles, images, binaries, minified bundles, credentials), calculates accurate offline token counts, and formats the codebase into Claude-optimized XML or Markdown prompts.

### 🌟 Key Highlights

- **⚡ Blazing Fast**: Processes medium-to-large repositories in **sub-30ms** using multi-threaded work-stealing directory traversal (`ignore`) and parallel chunked processing (`rayon`).
- **🛡️ Smart Filtering**: Automatically ignores lockfiles (`Cargo.lock`, `package-lock.json`, `pnpm-lock.yaml`, etc.), vector graphics (`.svg`), minified bundles (`*.min.js`), secrets (`.env*`, `*pem`, `*id_rsa*`), and binaries (Git 8KB NUL-byte heuristic).
- **📋 Seamless Ergonomics**: Directly copies prompts to your system clipboard (`-c` / `--copy`) without dumping megabytes to terminal scrollback.
- **🔢 Accurate Token Counting**: Multi-threaded offline token calculation via `tiktoken-rs` supporting `cl100k_base` (GPT-4), `o200k_base` (GPT-4o), and Anthropic Claude calibration.
- **🎨 Model-Optimized Outputs**:
  - **XML (Default)**: Claude-optimized format with an ASCII `<repository_structure>` tree followed by cleanly tagged `<file path="...">` blocks.
  - **Markdown**: Triple-backtick fenced blocks with language syntax detection and collision-safe backtick fences.
- **📦 Zero Runtime Overhead**: Single self-contained static binary.

---

## 📥 Installation

### 1. One-Line Installer (macOS & Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/WVDYC/repOx/main/install.sh | sh
```

### 2. Homebrew (macOS & Linux)

```bash
brew tap WVDYC/tap https://github.com/WVDYC/repOx
brew install repox
```

### 3. Cargo (From Source)

Ensure you have Rust 1.85+ installed:

```bash
cargo install --git https://github.com/WVDYC/repOx.git
```

Or build the optimized release binary directly:

```bash
git clone https://github.com/WVDYC/repOx.git
cd repOx
cargo build --release
# Binary available at ./target/release/repox
```

### 4. Shell Completions

`repOx` supports generating native autocompletions for your shell:

```bash
# Zsh
repox --completions zsh > ~/.zsh/completion/_repox

# Bash
repox --completions bash > ~/.local/share/bash-completion/completions/repox

# Fish
repox --completions fish > ~/.config/fish/completions/repox.fish
```

---

## ⚡ Quickstart

### 1. Interactive TUI Mode (`-i` / `--tui`)
```bash
repox -i
# or with Anthropic Claude token calibration:
repox -i -p claude
```
*Launches an ultra-responsive, lazygit-style terminal file manager with live preview, fuzzy search (`/`), and real-time context token budgeting.*

### 2. Copy repository directly to clipboard
```bash
repox -c
```
*Copies the Claude-formatted XML prompt directly to your clipboard and displays stats in `stderr`:*
```
✓ Copied to clipboard: 42 files (142.6 KB) in 4.12ms
```

### 3. Include accurate token count
```bash
repox -c -t -p claude
```
```
✓ Copied to clipboard: 42 files (142.6 KB, 38,120 tokens) in 38.50ms
```

### 4. Save formatted Markdown to a file
```bash
repox -f markdown -o context.md
```

### 5. Target a subfolder with file size and depth limits
```bash
repox src/ -s 500KB -d 3 -o prompt.xml
```

### 6. Pipe clean prompt output into another tool
```bash
repox -q | pbcopy
```

---

## 🖥️ Interactive Terminal UI (TUI)

Launch the interactive interface with `repox -i` or `repox --tui`. Designed for high-speed exploration and zero-latency selection on large repositories:

```text
┌── Files ──────────────────────────────────┐┌── Preview: src/main.rs ──────────────┐
│ ▸ [x] crates/                             ││ 1 │ use clap::Parser;                 │
│ ▾ [x] src/                                ││ 2 │ use color_eyre::eyre::Result;     │
│   [x] cli.rs                     4.2 KB   ││ 3 │                                   │
│   [x] main.rs                    2.8 KB   ││ 4 │ fn main() -> Result<()> {         │
│   [x] Cargo.toml                 1.4 KB   ││ 5 │     // ...                        │
└───────────────────────────────────────────┘└───────────────────────────────────────┘
  [Claude 3.5 Sonnet] 3/3 files | 18,420 / 200,000 tokens (9.2%) [■■░░░░░░░░░░] 8.4 KB
  [↑/↓] Navigate  [Space] Toggle  [/] Filter  [a] Toggle All  [c] Copy  [Enter] Output  [q] Quit
```

### Keybindings

| Key | Action |
| :--- | :--- |
| `↑` / `k` or `↓` / `j` | Move selection cursor up / down |
| `←` / `h` or `→` / `l` | Collapse / Expand directory folder |
| `Space` | Toggle file or folder selection (cascading tri-state checkboxes `[ ]`, `[-]`, `[x]`) |
| `a` | Toggle / invert all files |
| `/` | Open live fuzzy search filter (powered by `nucleo-matcher`) |
| `Esc` | Clear active filter query / cancel |
| `c` | **Copy to clipboard**: copy formatted context for selected files and exit |
| `Enter` | **Output**: on a file, output context to stdout/file and exit; on a folder, toggle expand/collapse |
| `q` | Quit without action |

---

## 📖 CLI Usage & Options

```text
repox [OPTIONS] [PATH]

Arguments:
  [PATH]                     Target repository directory [default: .]

Options:
  -i, --interactive          Launch interactive terminal UI file picker (alias: --tui)
  -f, --format <FORMAT>      Format template: 'xml' (Claude-optimized) or 'markdown' / 'md' [default: xml]
  -c, --copy                 Copy output context directly to system clipboard
  -o, --output <FILE>        Write formatted context to an output file instead of stdout
  -t, --tokens               Calculate total token count using multi-threaded tiktoken tokenizer
  -p, --token-profile <PROF> Tokenizer profile: 'cl100k' (GPT-4), 'o200k' (GPT-4o), or 'claude' [default: cl100k]
  -s, --max-file-size <SIZE> Skip files exceeding size threshold (e.g. 500KB, 1.5MB) [default: 1MB]
  -d, --max-depth <DEPTH>    Maximum directory nesting depth to traverse
      --no-gitignore         Disable respecting .gitignore files
      --no-repoxignore       Disable respecting .repoxignore files
      --include-hidden       Include hidden files and folders
  -e, --exclude <GLOB>       Glob pattern to exclude (can be specified multiple times)
  -I, --include <GLOB>       Glob pattern to include exclusively (can be specified multiple times)
  -j, --threads <N>          Worker threads count (defaults to logical CPU core count)
  -q, --quiet                Silence informational statistics on stderr
  -v, --verbose              Enable verbose debug logs
  -h, --help                 Print help
  -V, --version              Print version
```

---

## 📂 Formats

### Claude XML Output (`-f xml`)

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
...
</file>

<file path="src/main.rs">
fn main() { ... }
</file>
```

### Markdown Output (`-f md`)

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

---

## 📊 Benchmarks

Measured using [`hyperfine`](https://github.com/sharkdp/hyperfine) on Apple Silicon (M-series) traversing a 3,000+ file repository (15 runs, 3 warmups):

| Tool | Average Latency | Speedup | Memory Overhead | Offline Tokenization |
| :--- | :--- | :--- | :--- | :--- |
| **`repox`** (Claude XML) | **14.2 ms ± 0.8 ms** | **1.0x (baseline)** | **~18 MB** | No (instant) |
| **`repox -t -p claude`** | **42.6 ms ± 1.4 ms** | **~3.0x slower** | **~42 MB** | **Yes (`tiktoken-rs`)** |
| `repomix` (`npx repomix`) | **1,850.4 ms ± 48.2 ms** | **~130x slower** | **~185 MB** | Partial |

*repOx achieves sub-30ms performance via multi-threaded work-stealing directory traversal (`ignore`), parallel chunking (`rayon`), and zero-copy string formatting.*

---

## 🏗️ Architecture

`repOx` is organized as a modular Cargo workspace:

- **`crates/repox-core`**: Core engine handling directory walking (`ignore`), filtering, token counting (`tiktoken-rs`), and prompt formatting.
- **`crates/repox-tui`**: Terminal User Interface module built with `ratatui` and `crossterm`.
- **`src/main.rs` & `src/cli.rs`**: Fast CLI driver powered by `clap` with styled output, error handling via `color-eyre`, and telemetry.

---

## 🧪 Testing

Run all unit and integration tests across the workspace:

```bash
cargo test --workspace
```

---

## 📄 License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
