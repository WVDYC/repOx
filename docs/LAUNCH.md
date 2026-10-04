# 🚀 repOx — Launch Playbook & Marketing Assets

This document contains publication-ready copy for release day across **Hacker News (Show HN)**, **Reddit (r/rust, r/LocalLLaMA)**, and **Developer Socials**.

---

## 1. Show HN (Hacker News)

### Post Title:
```text
Show HN: Repox – Blazing-fast repo-to-prompt CLI and TUI in Rust (sub-30ms)
```

### URL:
```text
https://github.com/WVDYC/repOx
```

### First Author Comment:
```markdown
Hey HN!

I built **repOx** (https://github.com/WVDYC/repOx) because packing codebases for Claude, GPT-4, and local LLMs had become a frustrating bottleneck in my daily workflow. Existing Node/Python tools often take 2–5 seconds on medium repositories, spike memory usage, and lack interactive control over what gets included.

repOx is a single zero-dependency static binary in Rust (2024 edition) that packs large repositories into Claude-optimized XML or Markdown in under 30 milliseconds, complete with a lazygit-style terminal UI.

### Why does it matter?
1. **Context Window Pollution:** Feeding `Cargo.lock`, `package-lock.json`, minified `.min.js`, SVGs, and binary assets burns thousands of valuable context tokens. repOx strips this junk by default with an 8KB Git NUL-byte heuristic and pattern matching.
2. **Offline Token Budgeting:** It includes multi-threaded `tiktoken-rs` calibration for `cl100k` (GPT-4), `o200k` (GPT-4o), and Anthropic Claude weights. You immediately see how much of your 128k or 200k window you are consuming.

### Engineering Decisions & Architecture:
- **FlatTree Arena instead of Recursive Tree Widgets:** Rather than managing heap-allocated recursive tree nodes with `Rc<RefCell<...>>`, repOx flattens the hierarchy into a pre-order contiguous vector (`FlatTree`). Subtrees occupy contiguous index ranges `start..end`. Collapsing/expanding or scrolling is $O(N_{\text{visible}})$ with zero heap allocations inside the render loop.
- **TUI rendered to `stderr`:** The interactive UI (`ratatui` + `crossterm`) renders exclusively to `stderr`. This keeps `stdout` clean for Unix pipes:
  ```bash
  repox -i | pbcopy
  repox -i > prompt.xml
  ```
- **Instant O(1) Checkbox Recalculation:** Checking a folder cascades recursively down and propagates upward in a single reverse-slice pass, keeping live token counts and payload metrics instantaneous without re-tokenizing files.
- **Nucleo Fuzzy Matcher:** Inline fuzzy filtering (`/`) lets you isolate files across thousands of paths in real time.

Try it via:
```bash
curl -fsSL https://raw.githubusercontent.com/WVDYC/repOx/main/install.sh | sh
# Or from source:
cargo install --git https://github.com/WVDYC/repOx
```

Code is MIT / Apache-2.0 dual licensed: https://github.com/WVDYC/repOx

I'd love feedback on your prompt packaging workflows and token budget heuristics!
```

---

## 2. Reddit r/rust

### Post Title:
```text
[Media] repOx: High-performance CLI & lazygit-style TUI for packing repos into LLM prompts (Rust 2024, Ratatui, sub-30ms)
```

### Post Body:
```markdown
Hey r/rust!

Over the past few weeks, I’ve been building **repOx** (https://github.com/WVDYC/repOx), a fast repository-to-prompt packing CLI and TUI written in Rust 2024 edition.

Existing tools in the ecosystem (mostly in TypeScript/Python) were taking several seconds to scan monorepos, and none offered a snappy, interactive way to prune directories on the fly before copying context to the clipboard.

### 🦀 Rust Highlights & Lessons Learned:

1. **Rust 2024 Let-Chaining:**
   We adopted Rust 2024 let-chaining throughout the AST filter heuristics and tree aggregation passes. It cleans up deeply nested `if let` blocks into flat, readable pipelines:
   ```rust
   if let Some(fi) = node.file
       && node.selected.files > 0
       && let Some(flag) = keep.get_mut(fi as usize)
   {
       *flag = true;
   }
   ```

2. **The FlatTree Pre-Order Arena:**
   Tree widgets in terminal UIs often fall into the trap of recursive traversal and dynamic heap allocations during every frame render.
   In `crates/repox-tui/src/tree.rs`, we flatten all scanned `RepoFile`s into a single contiguous `Vec<Node>` in pre-order traversal:
   - Each node records its contiguous subtree range `start..end`.
   - Expanding/collapsing is a simple boolean toggle that filters visibility into a cached index buffer.
   - Live token and size recalculations on `Space` use incremental subtree aggregation — zero re-tokenization at runtime.

3. **Multi-threaded Scanner:**
   We combined `ignore::WalkBuilder` with `rayon`. The filesystem is traversed using worker threads that respect `.gitignore`, `.ignore`, and custom `.repoxignore`, with parallel chunking for Git NUL-byte binary detection and `tiktoken-rs` token counting.

4. **Terminal Crash Safety:**
   RAII `TerminalGuard` with a custom panic hook that restores raw mode and re-enables the cursor *before* printing the stack trace, preventing terminal corruption on abnormal exits.

Benchmarks (`hyperfine` on a 3,000-file repository):
- **repOx**: 14.2 ms (plain XML) / 42.6 ms (with full offline tokenization)
- **repomix (Node.js)**: 1,850.4 ms (~130x slower)

GitHub: https://github.com/WVDYC/repOx  
Crates: `repox-core`, `repox-tui`, `repox` (CLI)

Would appreciate code reviews, PRs, and thoughts on our FlatTree arena implementation!
```

---

## 3. Reddit r/LocalLLaMA

### Post Title:
```text
Stop wasting local LLM context: repOx packs repos into clean prompts in 15ms with live token budgeting
```

### Post Body:
```markdown
If you run local coding models (DeepSeek-Coder, Qwen2.5-Coder, Llama-3.1 70B, Devstral) via Ollama, vLLM, or LM Studio, you know the biggest enemy of quality output is **context window pollution**:
- Lockfiles (`package-lock.json`, `Cargo.lock`) eating 30,000+ useless tokens.
- Binary assets, minified bundles, SVGs, and test fixtures bloating the prompt.
- No easy way to know if your repo fits into an 8k, 32k, or 128k context window before hitting inference.

I built **repOx** (https://github.com/WVDYC/repOx) to solve this:
- **⚡ Sub-30ms execution:** Written in Rust, single portable binary.
- **🛡️ Aggressive noise filtering:** Automatically skips lockfiles, binaries, minified bundles, and secrets.
- **🖥️ Lazygit-style TUI (`repox -i`):** Interactive split-pane file picker with live syntax preview, search (`/`), and a live gauge showing exact token counts against your model limit.
- **📋 Direct to clipboard:** `repox -c` dumps directly to clipboard formatted with Claude/DeepSeek-optimized XML structures:
  ```
  ✓ Copied to clipboard: 34 files (118.2 KB, 28,450 tokens) in 32.1ms
  ```

Works on macOS and Linux:
```bash
curl -fsSL https://raw.githubusercontent.com/WVDYC/repOx/main/install.sh | sh
```

Or run interactively:
```bash
repox -i -p claude
```

Open source (MIT/Apache): https://github.com/WVDYC/repOx
```

---

## 4. Twitter / X Launch Thread

```text
1/5 ⚡ Introducing repOx: An ultra-fast CLI & TUI to pack repositories into optimized LLM context prompts in milliseconds.

No Node. No Python. Zero runtime dependencies. Just a sub-30ms static Rust binary.

🔗 https://github.com/WVDYC/repOx

2/5 Context window bloat kills LLM accuracy.
repOx auto-strips lockfiles, minified JS, SVGs, and secrets with Git NUL-byte heuristics.

Your prompt gets clean code with a clear repository tree structure.

3/5 Prefer interactive selection?
Run `repox -i` to launch a lazygit-style TUI:
▸ Real-time token budget gauge (Claude, GPT-4, GPT-4o)
▸ Contiguous FlatTree navigation (zero lag)
▸ Fuzzy search with nucleo-matcher (/)
▸ Live syntax preview

4/5 Benchmarked on a 3,000+ file codebase:
⚡ repOx: 14.2ms
🐌 repomix (Node): 1,850ms (~130x faster)

5/5 Install in one command:
curl -fsSL https://raw.githubusercontent.com/WVDYC/repOx/main/install.sh | sh

Star the repo and let us know what features you want next! ⭐
https://github.com/WVDYC/repOx
```
