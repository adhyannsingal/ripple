use clap::Parser;

/// Local change-impact analysis for TypeScript repositories.
#[derive(Parser)]
#[command(
    name = "ripple",
    version,
    arg_required_else_help = true,
    after_help = "Workspace bootstrap only: analysis commands are not implemented yet."
)]
struct Cli {}

fn main() {
    Cli::parse();
}
