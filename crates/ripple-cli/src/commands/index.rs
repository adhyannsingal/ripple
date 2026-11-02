use super::{CommandError, persistent};
use crate::output::{Format, terminal_json};
use std::path::Path;
pub fn run(root: &Path, format: Format) -> Result<(), CommandError> {
    let result = ripple_index::build::cold_index(root, ripple_git::worktree::observe)
        .map_err(persistent::error)?;
    let human = format!(
        "Index generation: {}\nFiles: {}\nSymbols: {}\nEdges: {}\nUnresolved: {}\nCapture attempts: {}\nMetadata: {}\nModule cache: {}\nIncomplete analysis: {}\n",
        result.generation,
        result.counts.files,
        result.counts.symbols,
        result.counts.edges,
        result.counts.unresolved,
        result.attempts,
        terminal_json(&result.metadata).map_err(|e| persistent::error(e.to_string()))?,
        terminal_json(&result.module_cache).map_err(|e| persistent::error(e.to_string()))?,
        result.incomplete
    );
    let human = format!(
        "{human}Config diagnostics: {}\nDiscovery diagnostics: {}\n",
        terminal_json(&result.config_diagnostics).map_err(|e| persistent::error(e.to_string()))?,
        terminal_json(&result.discovery_diagnostics)
            .map_err(|e| persistent::error(e.to_string()))?
    );
    persistent::emit("index", &result, result.incomplete, format, human)
}
