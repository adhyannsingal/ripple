mod commands;
mod output;

use clap::{Args, Parser, Subcommand};
use commands::CommandError;
use output::Format;
use ripple_core::identity::RepoPath;
use std::{
    ffi::OsString,
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
    after_help = "Inspect reports syntax facts. Index, impact, diff, test and status are not implemented yet."
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
    /// Not implemented yet: build a persistent dependency index.
    Index(FutureArgs),
    /// Not implemented yet: query downstream dependency paths.
    Impact(FutureArgs),
    /// Not implemented yet: analyze Git base/head changes.
    Diff(FutureArgs),
    /// Not implemented yet: report potentially related test files.
    Test(FutureArgs),
    /// Not implemented yet: show persistent index freshness.
    Status(FutureArgs),
}
#[derive(Args)]
struct FutureArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
    _args: Vec<OsString>,
}
fn run(cli: Cli) -> Result<(), CommandError> {
    let report = match cli.command {
        Command::Inspect { root, file } => commands::inspect::run(&root, file)?,
        other => {
            let name = match other {
                Command::Index(_) => "index",
                Command::Impact(_) => "impact",
                Command::Diff(_) => "diff",
                Command::Test(_) => "test",
                Command::Status(_) => "status",
                Command::Inspect { .. } => unreachable!(),
            };
            return Err(CommandError::new(
                "command_not_implemented",
                format!("{name} is not implemented yet; use inspect for syntax facts"),
            ));
        }
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
