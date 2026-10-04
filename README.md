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

### From Source (Cargo)

Ensure you have Rust 1.85+ installed:

```bash
git clone https://github.com/WVDYC/repOx.git
cd repOx
cargo install --path .
```

Or build the optimized release binary directly:

```bash
cargo build --release
# Binary available at ./target/release/repox
```

---

## ⚡ Quickstart

### 1. Copy repository directly to clipboard
```bash
repox -c
```
*Copies the Claude-formatted XML prompt directly to your clipboard and displays stats in `stderr`:*
```
✓ Copied to clipboard: 42 files (142.6 KB) in 4.12ms
```

### 2. Include accurate token count
```bash
repox -c -t -p claude
```
```
✓ Copied to clipboard: 42 files (142.6 KB, 38,120 tokens) in 38.50ms
```

### 3. Save formatted Markdown to a file
```bash
repox -f markdown -o context.md
```

### 4. Target a subfolder with file size and depth limits
```bash
repox src/ -s 500KB -d 3 -o prompt.xml
```

### 5. Pipe clean prompt output into another tool
```bash
repox -q | pbcopy
```

---

## 📖 CLI Usage & Options

```text
repox [OPTIONS] [PATH]

Arguments:
  [PATH]                     Target repository directory [default: .]

Options:
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
  -i, --include <GLOB>       Glob pattern to include exclusively (can be specified multiple times)
  -j, --threads <N>          Worker threads count (defaults to logical CPU core count)
      --tui                  Launch interactive terminal UI file picker
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
