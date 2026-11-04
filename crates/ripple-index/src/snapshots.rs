//! Isolated immutable cold builds. No database/local-index publication or Git dependency.
use crate::{build, build_capture};
use ripple_core::{
    identity::{ProjectKey, RepoPath},
    index::{IndexMetadata, IndexSnapshot, InputRole, SnapshotData},
    revision::{RevisionSource, SnapshotOrigin},
    source::{EntryKind, SourceSnapshot, SourceTree},
};
use ripple_resolve::config::ConfigDiagnostic;
use std::{
    collections::BTreeSet,
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Debug)]
pub struct RevisionSnapshot {
    index: IndexSnapshot,
    modules: ripple_resolve::reexports::ModuleAnalysis,
    diagnostics: Vec<String>,
    sources: SourceSnapshot,
    global_inputs: BTreeSet<RepoPath>,
    existing_paths: BTreeSet<RepoPath>,
    incomplete: bool,
    config_diagnostics: Vec<ConfigDiagnostic>,
    discovery_diagnostics: Vec<String>,
}
impl RevisionSnapshot {
    pub fn modules(&self) -> &ripple_resolve::reexports::ModuleAnalysis {
        &self.modules
    }
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }
    pub fn index(&self) -> &IndexSnapshot {
        &self.index
    }
    pub fn sources(&self) -> &SourceSnapshot {
        &self.sources
    }
    pub fn global_inputs(&self) -> &BTreeSet<RepoPath> {
        &self.global_inputs
    }
    pub fn existing_paths(&self) -> &BTreeSet<RepoPath> {
        &self.existing_paths
    }
    pub fn incomplete(&self) -> bool {
        self.incomplete
    }
    pub fn config_diagnostics(&self) -> &[ConfigDiagnostic] {
        &self.config_diagnostics
    }
    pub fn discovery_diagnostics(&self) -> &[String] {
        &self.discovery_diagnostics
    }
}
// Presence is separate from supported discovery: ignored or nonregular tests are not deleted.
fn presence(tree: &impl SourceTree) -> Result<BTreeSet<RepoPath>, String> {
    let mut stack = vec![(None, 0usize)];
    let mut seen = BTreeSet::new();
    let mut files = BTreeSet::new();
    while let Some((parent, depth)) = stack.pop() {
        if depth > 128 {
            return Err("revision path depth exceeds 128".into());
        }
        let entries = tree
            .entries(parent.as_ref(), 100_000 - seen.len())
            .map_err(|e| e.to_string())?;
        for e in entries {
            let actual = e.path.as_str().rsplit_once('/').map(|(p, _)| p);
            if actual != parent.as_ref().map(RepoPath::as_str) || !seen.insert(e.path.clone()) {
                return Err("invalid revision presence tree entry".into());
            }
            if seen.len() > 100_000 {
                return Err("revision presence exceeds 100000 entries".into());
            }
            if e.kind == EntryKind::Directory {
                stack.push((Some(e.path), depth + 1));
            } else {
                files.insert(e.path);
            }
        }
    }
    Ok(files)
}
/// Supply the provider pinned to this exact commit (e.g. ripple-git's RevisionTree).
/// Capture/config/extraction/resolution match cold worktree indexing; no worktree is materialized.
pub fn build(tree: &impl RevisionSource) -> Result<RevisionSnapshot, String> {
    let revision = tree.revision();
    let captured = build_capture::capture_tree(
        tree,
        SnapshotOrigin::Commit {
            revision: revision.clone(),
        },
    )?;
    let existing_paths = presence(tree)?;
    let (files, analysis) = build::analyze(&captured)?;
    let global_inputs = captured
        .manifest
        .entries()
        .iter()
        .filter(|i| i.role != InputRole::Source)
        .map(|i| i.path.clone())
        .chain(captured.missing.iter().cloned())
        .chain(captured.config.inputs().iter().map(|i| i.path.clone()))
        .collect();
    let config_diagnostics = captured.config.diagnostics().to_vec();
    let discovery_diagnostics = captured
        .discovery
        .diagnostics
        .iter()
        .map(|d| format!("{}: {}", d.path, d.code))
        .collect::<Vec<_>>();
    let incomplete = analysis.incomplete
        || !discovery_diagnostics.is_empty()
        || files.iter().any(|f| {
            f.parse_status != ripple_core::syntax::ParseStatus::Clean || !f.diagnostics.is_empty()
        });
    let mut diagnostics = captured
        .discovery
        .diagnostics
        .iter()
        .map(|d| format!("{}: {}: {}", d.path, d.code, d.message))
        .chain(
            config_diagnostics
                .iter()
                .map(|d| format!("{}: {}: {}", d.file, d.code, d.detail)),
        )
        .chain(files.iter().flat_map(|f| {
            f.diagnostics
                .iter()
                .map(|d| format!("{}: {}: {}", f.path, d.code, d.message))
        }))
        .collect::<Vec<_>>();
    if analysis.modules.incomplete {
        diagnostics.push("module/export analysis incomplete; detailed boundaries retained in the module artifact".into());
    }
    diagnostics.sort();
    diagnostics.dedup();

    let index = SnapshotData {
        metadata: IndexMetadata {
            project: ProjectKey::single_project(),
            origin: captured.origin,
            versions: build::versions(),
            effective_config_hash: Some(captured.config.fingerprint().clone()),
            generation: 1,
            created_at: format!(
                "unix:{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| e.to_string())?
                    .as_secs()
            ),
        },
        inputs: captured.manifest,
        files,
        graph: analysis.graph,
    }
    .validate(&captured.discovery.sources)
    .map_err(|e| e.to_string())?;
    Ok(RevisionSnapshot {
        index,
        modules: analysis.modules,
        diagnostics,
        sources: captured.discovery.sources,
        global_inputs,
        existing_paths,
        incomplete,
        config_diagnostics,
        discovery_diagnostics,
    })
}
