//! Composes verified cached generation with pure guarded resolution reuse.
use crate::{build, build_capture::Capture, incremental::UpdateStats, storage::LoadedGeneration};
use ripple_core::{identity::ProjectKey, syntax::FileFacts};
use ripple_resolve::symbols::GraphAnalysis;
use std::collections::BTreeSet;
pub(crate) fn resolve(
    captured: &Capture,
    files: &[FileFacts],
    previous: Option<&LoadedGeneration>,
    stats: &mut UpdateStats,
) -> Result<GraphAnalysis, String> {
    if stats.rebuild_reason.is_some() {
        return build::resolve(captured, files);
    }
    let previous = previous.ok_or("selective update missing previous generation")?;
    let cache = previous
        .analysis
        .as_ref()
        .ok_or("selective update missing analysis cache")?;
    let old = GraphAnalysis {
        graph: previous.snapshot.graph().clone(),
        modules: cache.modules.clone(),
        incomplete: cache.modules.incomplete || !previous.snapshot.graph().unresolved().is_empty(),
    };
    let tests = captured
        .discovery
        .files
        .iter()
        .filter(|f| f.is_test)
        .map(|f| f.path.clone())
        .collect::<BTreeSet<_>>();
    let (analysis, reuse) = ripple_resolve::dependencies::resolve_with_reuse(
        &ProjectKey::single_project(),
        &captured.discovery.sources,
        files,
        &captured.config,
        &tests,
        &old,
        previous.snapshot.files(),
    )
    .map_err(|e| e.to_string())?;
    stats.strategy = "incremental_selective";
    stats.module_files_resolved = reuse.module_files_resolved;
    stats.reference_files_resolved = reuse.reference_files_resolved;
    stats.reference_files_reused = reuse.reference_files_reused;
    stats.module_cache_reused = reuse.modules_reused;
    stats.invalidation_candidates = reuse.invalidation_candidates;
    stats.unchanged_external_surfaces = reuse.unchanged_external_surfaces;
    Ok(analysis)
}
