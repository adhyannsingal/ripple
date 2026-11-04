use super::{CommandError, persistent};
use crate::output::{Format, terminal_json};
use std::path::Path;
pub fn run(root: &Path, full: bool, format: Format) -> Result<(), CommandError> {
    let result = if full {
        ripple_index::build::cold_index(root, ripple_git::worktree::observe)
    } else {
        ripple_index::incremental::refresh(root, ripple_git::worktree::observe)
    }
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
    let update = &result.update;
    let deltas = if update.source_delta_available {
        format!(
            "{} added, {} changed, {} deleted",
            update.added.len(),
            update.changed.len(),
            update.deleted.len()
        )
    } else {
        "unavailable for this rebuild".into()
    };
    let human = format!(
        "{human}Update strategy: {}\nFull rebuild reason: {}\nAccepted attempt: {} files parsed, {} parse facts reused\nModule analysis: {} files resolved (cache reused: {})\nReference analysis: {} files resolved, {} files reused\nTotal files parsed across attempts: {}\nSource changes: {}\nInvalidation candidates: {}\n",
        terminal_json(&update.strategy).map_err(|e| persistent::error(e.to_string()))?,
        terminal_json(&update.rebuild_reason).map_err(|e| persistent::error(e.to_string()))?,
        update.parsed_files,
        update.reused_files,
        update.module_files_resolved,
        update.module_cache_reused,
        update.reference_files_resolved,
        update.reference_files_reused,
        update.total_parsed_files,
        deltas,
        update.invalidation_candidates.len()
    );
    persistent::emit("index", &result, result.incomplete, format, human)
}
