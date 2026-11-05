mod commands;
mod output;

use clap::{Parser, Subcommand};
use commands::CommandError;
use output::Format;
use ripple_core::identity::RepoPath;
use std::{
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

/// Local change-impact analysis for TypeScript repositories.
#[derive(Parser)]
#[command(
    name = "ripple",
    version,
    arg_required_else_help = true,
    after_help = "Index refreshes verified inputs (--full rebuilds); status/impact verify freshness. Diff and test compare immutable A..B endpoints; test reports potentially related files without running them."
)]
struct Cli {
    /// Output style for implemented commands.
    #[arg(long, global = true, value_enum, default_value = "human")]
    format: Format,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Inspect captured TS/TSX syntax facts without resolving dependencies.
    Inspect {
        /// File relative to --root; must be discovered and not ignored/excluded.
        #[arg(value_parser=commands::inspect::file_argument)]
        file: RepoPath,
        /// Explicit project boundary; defaults to the current directory.
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Refresh a verified persistent index, reusing unchanged analysis where proven.
    Index {
        #[arg(default_value = ".")]
        root: PathBuf,
        /// Force cold parsing and resolution; does not silently migrate incompatible formats.
        #[arg(long)]
        full: bool,
    },
    /// Query explainable downstream paths from a fresh index.
    Impact {
        symbol: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long)]
        depth: Option<usize>,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    /// Compare explicit immutable Git endpoints and explain downstream impact.
    Diff {
        /// Explicit A..B tree endpoints; three-dot merge-base semantics are unsupported.
        comparison: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Maximum traversal depth; the default is unlimited.
        #[arg(long)]
        depth: Option<usize>,
        /// Maximum human display rows per section; JSON retains complete results.
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    /// Report potentially related test files without executing them.
    Test {
        /// Explicit A..B immutable tree endpoints.
        comparison: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Maximum traversal depth; omissions are explicitly reported.
        #[arg(long)]
        depth: Option<usize>,
        /// Maximum human rows per section; JSON retains complete selection.
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    /// Verify index freshness and show provenance/counts.
    Status {
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
}
fn run(cli: Cli) -> Result<(), CommandError> {
    let report = match cli.command {
        Command::Index { root, full } => return commands::index::run(&root, full, cli.format),
        Command::Status { root } => return commands::status::run(&root, cli.format),
        Command::Impact {
            symbol,
            root,
            depth,
            limit,
        } => return commands::impact::run(&root, &symbol, depth, limit, cli.format),
        Command::Diff {
            comparison,
            root,
            depth,
            limit,
        } => return commands::diff::run(&root, &comparison, depth, limit, cli.format),
        Command::Inspect { root, file } => commands::inspect::run(&root, file)?,
        Command::Test {
            comparison,
            root,
            depth,
            limit,
        } => return commands::test::run(&root, &comparison, depth, limit, cli.format),
    };
    let bytes = report
        .render(cli.format)
        .map_err(|e| CommandError::new("output_failed", e))?;
    let diagnostics = report
        .stderr()
        .map_err(|e| CommandError::new("output_failed", e.to_string()))?;
    io::stdout()
        .lock()
        .write_all(&bytes)
        .map_err(|e| CommandError::new("output_failed", e.to_string()))?;
    io::stderr()
        .lock()
        .write_all(diagnostics.as_bytes())
        .map_err(|e| CommandError::new("output_failed", e.to_string()))?;
    Ok(())
}
fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(
                io::stderr().lock(),
                "error[{}]: {:?}",
                error.code,
                error.message
            );
            ExitCode::FAILURE
        }
    }
}
