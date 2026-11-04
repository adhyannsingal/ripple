//! Conservative refresh: reuse only content-bound facts; resolve the complete graph.
use crate::{build, build_capture::Capture, storage::LoadedGeneration};
use ripple_core::{identity::RepoPath, index::InputRole, source::SourceFile, syntax::FileFacts};
use serde::Serialize;
use std::{collections::BTreeMap, path::Path};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RebuildReason {
    ExplicitFull,
    EmptyIndex,
    IncompatibleVersions,
    MissingAnalysis,
    GlobalInputs,
    MissingProbes,
    Configuration,
    DiscoveryPolicy,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateStats {
    pub strategy: &'static str,
    pub rebuild_reason: Option<RebuildReason>,
    /// Work in the accepted attempt; total_parsed_files includes rejected attempts.
    pub parsed_files: usize,
    pub reused_files: usize,
    pub module_files_resolved: usize,
    pub reference_files_resolved: usize,
    pub reference_files_reused: usize,
    pub module_cache_reused: bool,
    pub invalidation_candidates: Vec<RepoPath>,
    pub unchanged_external_surfaces: Vec<RepoPath>,
    pub total_parsed_files: usize,
    pub source_delta_available: bool,
    pub added: Vec<RepoPath>,
    pub changed: Vec<RepoPath>,
    pub deleted: Vec<RepoPath>,
}
pub fn refresh(root: &Path, provenance: build::Provenance) -> Result<build::IndexOutcome, String> {
    build::refresh_inner(root, provenance, |_, _| {})
}
pub(crate) fn prepare(
    captured: &Capture,
    previous: Option<&LoadedGeneration>,
    initial_reason: Option<RebuildReason>,
    mut parse: impl FnMut(&SourceFile) -> Result<FileFacts, String>,
) -> Result<(Vec<FileFacts>, UpdateStats), String> {
    let mut reason = initial_reason;
    if let Some(old) = previous {
        let before = old
            .snapshot
            .inputs()
            .entries()
            .iter()
            .filter(|i| i.role != InputRole::Source)
            .collect::<Vec<_>>();
        let after = captured
            .manifest
            .entries()
            .iter()
            .filter(|i| i.role != InputRole::Source)
            .collect::<Vec<_>>();
        if reason.is_none() {
            reason = if before != after {
                Some(RebuildReason::GlobalInputs)
            } else if let Some(cache) = &old.analysis {
                if old.snapshot.metadata().effective_config_hash.as_ref()
                    != Some(captured.config.fingerprint())
                {
                    Some(RebuildReason::Configuration)
                } else if cache.missing_inputs != captured.missing.iter().cloned().collect() {
                    Some(RebuildReason::MissingProbes)
                } else {
                    let mut diagnostics = captured
                        .discovery
                        .diagnostics
                        .iter()
                        .map(|d| format!("{}: {}: {}", d.path, d.code, d.message))
                        .collect::<Vec<_>>();
                    diagnostics.sort();
                    diagnostics.dedup();
                    (diagnostics != cache.discovery_diagnostics)
                        .then_some(RebuildReason::DiscoveryPolicy)
                }
            } else {
                Some(RebuildReason::MissingAnalysis)
            };
        }
    } else if reason.is_none() {
        reason = Some(RebuildReason::EmptyIndex);
    }
    let old_inputs = previous
        .map(|p| {
            p.snapshot
                .inputs()
                .entries()
                .iter()
                .filter(|i| i.role == InputRole::Source)
                .map(|i| (&i.path, i))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let old_facts = previous
        .map(|p| {
            p.snapshot
                .files()
                .iter()
                .map(|f| (&f.path, f))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let new_inputs = captured
        .manifest
        .entries()
        .iter()
        .filter(|i| i.role == InputRole::Source)
        .map(|i| (&i.path, i))
        .collect::<BTreeMap<_, _>>();
    let mut stats = UpdateStats {
        strategy: if reason.is_some() {
            "full"
        } else {
            "incremental_full_resolution"
        },
        rebuild_reason: reason,
        parsed_files: 0,
        reused_files: 0,
        module_files_resolved: new_inputs.len(),
        reference_files_resolved: new_inputs.len(),
        reference_files_reused: 0,
        module_cache_reused: false,
        invalidation_candidates: new_inputs.keys().map(|p| (*p).clone()).collect(),
        unchanged_external_surfaces: vec![],
        total_parsed_files: 0,
        source_delta_available: previous.is_some() || reason == Some(RebuildReason::EmptyIndex),
        added: vec![],
        changed: vec![],
        deleted: vec![],
    };
    if stats.source_delta_available {
        for (path, input) in &new_inputs {
            match old_inputs.get(path) {
                None => stats.added.push((*path).clone()),
                Some(old) if old != input => stats.changed.push((*path).clone()),
                _ => {}
            }
        }
        for path in old_inputs.keys() {
            if !new_inputs.contains_key(path) {
                stats.deleted.push((*path).clone());
            }
        }
    }
    let mut files = Vec::new();
    for source in captured.discovery.sources.files() {
        let same = old_inputs
            .get(source.path())
            .zip(new_inputs.get(source.path()))
            .is_some_and(|(a, b)| a == b);
        if reason.is_none() && same {
            files.push(
                (*old_facts
                    .get(source.path())
                    .ok_or("cached source has no facts")?)
                .clone(),
            );
            stats.reused_files += 1;
        } else {
            files.push(parse(source)?);
            stats.parsed_files += 1;
        }
    }
    stats.total_parsed_files = stats.parsed_files;
    Ok((files, stats))
}
