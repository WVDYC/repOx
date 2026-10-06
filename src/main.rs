mod cli;

use clap::Parser;
use cli::Cli;
use color_eyre::eyre::{self, Context};
use repox_core::{TokenCounter, format_repository, scan_repository};
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::time::Instant;
use tracing_subscriber::EnvFilter;

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.2} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Cross-platform clipboard copy with Wayland, X11, and ANSI OSC 52 fallbacks.
fn copy_to_clipboard(text: &str) -> eyre::Result<()> {
    if let Ok(mut clipboard) = arboard::Clipboard::new()
        && clipboard.set_text(text).is_ok()
    {
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::{Command, Stdio};
        if let Ok(mut child) = Command::new("wl-copy").stdin(Stdio::piped()).spawn() {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if child.wait().map(|s| s.success()).unwrap_or(false) {
                return Ok(());
            }
        }

        if let Ok(mut child) = Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if child.wait().map(|s| s.success()).unwrap_or(false) {
                return Ok(());
            }
        }
    }

    if text.len() <= 100_000 && io::stderr().is_terminal() {
        let b64 = base64_encode(text.as_bytes());
        let osc52 = format!("\x1b]52;c;{}\x07", b64);
        let mut stderr = io::stderr().lock();
        let _ = stderr.write_all(osc52.as_bytes());
        let _ = stderr.flush();
        return Ok(());
    }

    eyre::bail!("Failed to access system clipboard (arboard, Wayland wl-copy, or X11 xclip)")
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    // Handle shell completions generation early
    if let Some(shell) = cli.completions {
        use clap::CommandFactory;
        let mut cmd = Cli::command();
        clap_complete::generate(shell, &mut cmd, "repox", &mut io::stdout());
        return Ok(());
    }

    // Initialize tracing logger
    let log_level = if cli.verbose {
        "repox=debug,repox_core=debug"
    } else {
        "repox=warn,repox_core=warn"
    };

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(io::stderr)
        .init();

    let total_start = Instant::now();

    // 1. Scan and filter files
    let scan_opts = cli.to_scan_options();
    let (mut files, mut summary) =
        scan_repository(&scan_opts).wrap_err("Failed to scan repository files")?;

    if files.is_empty() {
        if !cli.quiet {
            eprintln!("Warning: No matching source files found in target path.");
        }
        return Ok(());
    }

    let mut should_copy = cli.copy;

    // 2. Launch interactive TUI if requested
    if cli.interactive {
        // Pre-calculate token counts so the TUI can display real-time token metrics and gauge
        let counter = TokenCounter::new(cli.token_profile)
            .map_err(|e| eyre::eyre!("Failed to initialize token counter: {e}"))?;
        let _ = counter.count_files_tokens(&mut files);

        let tui_options = repox_tui::TuiOptions::new(
            cli.token_profile.display_name(),
            cli.token_profile.context_window(),
        );

        match repox_tui::run(files, tui_options).wrap_err("TUI session encountered an error")? {
            repox_tui::TuiOutcome::Cancelled => {
                if !cli.quiet {
                    eprintln!("Aborted.");
                }
                return Ok(());
            }
            repox_tui::TuiOutcome::Selected {
                action,
                files: selected,
            } => {
                if selected.is_empty() {
                    if !cli.quiet {
                        eprintln!("No files selected.");
                    }
                    return Ok(());
                }
                if action == repox_tui::TuiAction::Copy {
                    should_copy = true;
                } else if action == repox_tui::TuiAction::CopyCommand {
                    let cmd = if selected.len() == 1 {
                        format!("repox {}", selected[0].relative_path.display())
                    } else {
                        let mut s = String::from("repox");
                        for f in &selected {
                            s.push_str(&format!(" \\\n  {}", f.relative_path.display()));
                        }
                        s
                    };
                    copy_to_clipboard(&cmd)?;
                    if !cli.quiet {
                        eprintln!("✓ Copied command to clipboard:\n{}", cmd);
                    }
                    return Ok(());
                }
                files = selected;
            }
        }

        // Update summary with selected file metrics
        summary.file_count = files.len();
        summary.total_bytes = files.iter().map(|f| f.size_bytes).sum();
        summary.total_tokens = Some(files.iter().map(|f| f.token_count.unwrap_or(0)).sum());
    } else if cli.tokens {
        // 3. Compute tokens in non-interactive mode if requested
        let counter = TokenCounter::new(cli.token_profile)
            .map_err(|e| eyre::eyre!("Failed to initialize token counter: {e}"))?;
        let total_tokens = counter.count_files_tokens(&mut files);
        summary.total_tokens = Some(total_tokens);
    }

    // 4. Format repository into LLM prompt
    let formatted = format_repository(&files, cli.format);

    // 5. Output actions
    let mut performed_action = false;

    if should_copy {
        copy_to_clipboard(&formatted)?;
        performed_action = true;
    }

    if let Some(ref out_path) = cli.output {
        fs::write(out_path, &formatted)
            .wrap_err_with(|| format!("Failed to write output to {}", out_path.display()))?;
        performed_action = true;
    }

    // If neither clipboard copy nor file output was selected, print formatted prompt to stdout
    if !performed_action {
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(formatted.as_bytes())
            .wrap_err("Failed to write output to stdout")?;
        stdout.flush().wrap_err("Failed to flush stdout")?;
    }

    // 6. Diagnostic summary to stderr
    if !cli.quiet {
        let total_elapsed = total_start.elapsed();
        let token_info = match summary.total_tokens {
            Some(toks) => format!(", {toks} tokens"),
            None => String::new(),
        };

        if should_copy {
            eprintln!(
                "✓ Copied to clipboard: {} files ({}{}) in {:.2?}",
                summary.file_count,
                format_bytes(summary.total_bytes),
                token_info,
                total_elapsed
            );
        } else if let Some(ref out_path) = cli.output {
            eprintln!(
                "✓ Written to {}: {} files ({}{}) in {:.2?}",
                out_path.display(),
                summary.file_count,
                format_bytes(summary.total_bytes),
                token_info,
                total_elapsed
            );
        } else if io::stderr().is_terminal() {
            // When stdout was printed to terminal, print short stats on stderr
            eprintln!(
                "\n[repox] Packed {} files ({}{}) in {:.2?}",
                summary.file_count,
                format_bytes(summary.total_bytes),
                token_info,
                total_elapsed
            );
        }
    }

    Ok(())
}
