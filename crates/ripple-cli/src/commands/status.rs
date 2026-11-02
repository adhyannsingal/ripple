use super::{CommandError, persistent};
use crate::output::{Format, terminal_json};
use std::path::Path;
pub fn run(root: &Path, format: Format) -> Result<(), CommandError> {
    let current = ripple_index::build::load_current(root, ripple_git::worktree::observe)
        .map_err(persistent::error)?;
    let metadata = current.loaded.snapshot.metadata();
    let human = format!(
        "Freshness: verified captured inputs\nGeneration: {}\nOrigin: {}\nAnalyzer: {}\nSchema: {}\nFiles: {}\nSymbols: {}\nEdges: {}\nUnresolved: {}\nIncomplete analysis: {}\n",
        metadata.generation,
        terminal_json(&metadata.origin).map_err(|e| persistent::error(e.to_string()))?,
        terminal_json(&metadata.versions.analyzer_version)
            .map_err(|e| persistent::error(e.to_string()))?,
        metadata.versions.schema_version,
        current.counts.files,
        current.counts.symbols,
        current.counts.edges,
        current.counts.unresolved,
        current.incomplete
    );
    persistent::emit(
        "status",
        &serde_json::json!({"freshness":"verified","metadata":metadata,"counts":current.counts,"config_diagnostics":current.config_diagnostics,"discovery_diagnostics":current.discovery_diagnostics}),
        current.incomplete,
        format,
        human,
    )
}
