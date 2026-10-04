mod cli;

use clap::Parser;
use cli::Cli;
use color_eyre::eyre::{self, Context};
use repox_core::{format_repository, scan_repository, TokenCounter};
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

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

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

    // 2. Launch interactive TUI if requested
    if cli.tui {
        files = repox_tui::run_tui(&files).wrap_err("TUI session encountered an error")?;
    }

    // 3. Compute tokens if requested or if copying with stats
    if cli.tokens {
        let counter = TokenCounter::new(cli.token_profile)
            .map_err(|e| eyre::eyre!("Failed to initialize token counter: {e}"))?;
        let total_tokens = counter.count_files_tokens(&mut files);
        summary.total_tokens = Some(total_tokens);
    }

    // 4. Format repository into LLM prompt
    let formatted = format_repository(&files, cli.format);

    // 5. Output actions
    let mut performed_action = false;

    if cli.copy {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| eyre::eyre!("Failed to access system clipboard: {e}"))?;
        clipboard
            .set_text(&formatted)
            .map_err(|e| eyre::eyre!("Failed to copy prompt to clipboard: {e}"))?;
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

        if cli.copy {
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
