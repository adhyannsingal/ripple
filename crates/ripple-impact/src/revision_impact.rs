//! Pure revision-bound query composition. No Git, SQLite, parser or filesystem dependency.
use crate::{
    changes::{self, ChangeKind, ChangeSet, Endpoint},
    graph::DependencyGraph,
    traversal::TraversalResult,
};
use ripple_core::{
    graph::{Confidence, NodeId, NodeKind},
    identity::RepoPath,
    index::{IndexSnapshot, InputRole},
    results::{EvidenceKind, EvidenceSets, QueryLimits, RevisionSide, Witness},
    revision::{RevisionChanges, RevisionGraphs},
    source::SourceSnapshot,
    syntax::ParseStatus,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
pub struct RevisionView<'a> {
    pub index: &'a IndexSnapshot,
    pub sources: &'a SourceSnapshot,
    pub global_inputs: &'a BTreeSet<RepoPath>,
    /// All committed non-directory paths, including ignored/unsupported paths. Not just indexed files.
    pub existing_paths: &'a BTreeSet<RepoPath>,
    pub incomplete: bool,
    pub diagnostics: &'a [String],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SeedReason {
    Added,
    Deleted,
    BodyModified,
    SignatureModified,
    Uncertain,
    FileLevel,
    GlobalInput,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SeedSelection {
    pub node: NodeId,
    pub revision: RevisionSide,
    pub reasons: Vec<SeedReason>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnindexedChange {
    pub path: RepoPath,
    pub revision: RevisionSide,
    pub reason: &'static str,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SideImpact {
    pub query: TraversalResult,
    /// Policy-selected whole-snapshot file fallback, not fabricated dependency edges.
    pub global_fallback: Vec<Witness>,
    pub seeds: Vec<SeedSelection>,
    pub unindexed_changes: Vec<UnindexedChange>,
    pub snapshot_incomplete: bool,
    pub snapshot_diagnostics: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RevisionRow {
    pub target: NodeId,
    pub distance: usize,
    pub confidence: Confidence,
    pub revisions: BTreeSet<RevisionSide>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RevisionTests {
    pub symbol_backed: BTreeSet<NodeId>,
    pub module_fallback: BTreeSet<NodeId>,
    pub fallback_only: BTreeSet<NodeId>,
    pub combined: BTreeSet<NodeId>,
    pub active_head: BTreeSet<NodeId>,
    pub base_only: BTreeSet<NodeId>,
    pub present_unindexed_head: BTreeSet<NodeId>,
    pub total_head_indexed_test_files: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RevisionImpact {
    changes: ChangeSet,
    base: SideImpact,
    head: SideImpact,
    evidence: EvidenceSets,
    rows: Vec<RevisionRow>,
    tests: RevisionTests,
    limits: QueryLimits,
    display_omitted: usize,
    incomplete: bool,
}
impl RevisionImpact {
    pub fn changes(&self) -> &ChangeSet {
        &self.changes
    }
    pub fn base(&self) -> &SideImpact {
        &self.base
    }
    pub fn head(&self) -> &SideImpact {
        &self.head
    }
    pub fn evidence(&self) -> &EvidenceSets {
        &self.evidence
    }
    pub fn rows(&self) -> &[RevisionRow] {
        &self.rows
    }
    pub fn displayed(&self) -> &[RevisionRow] {
        &self.rows[..self.rows.len().min(self.limits.display_limit)]
    }
    pub fn tests(&self) -> &RevisionTests {
        &self.tests
    }
    pub fn incomplete(&self) -> bool {
        self.incomplete
    }
    pub fn display_omitted(&self) -> usize {
        self.display_omitted
    }
}
fn endpoint<'a>(view: &'a RevisionView<'a>) -> Result<Endpoint<'a>, String> {
    let ripple_core::revision::SnapshotOrigin::Commit { revision } = &view.index.metadata().origin
    else {
        return Err("revision impact requires immutable commit snapshots".into());
    };
    Ok(Endpoint {
        revision,
        analyzer_version: &view.index.metadata().versions.analyzer_version,
        sources: view.sources,
        facts: view.index.files(),
        global_inputs: view.global_inputs,
    })
}
fn validate_view(view: &RevisionView<'_>) -> Result<(), String> {
    for input in view
        .index
        .inputs()
        .entries()
        .iter()
        .filter(|i| i.role != InputRole::Source)
    {
        if !view.global_inputs.contains(&input.path) {
            return Err("revision view omitted captured global inputs".into());
        }
    }
    for f in view.index.files() {
        if !view.existing_paths.contains(&f.path) {
            return Err("indexed file missing from revision path presence".into());
        }
    }
    Ok(())
}
fn side(
    view: &RevisionView<'_>,
    changes: &ChangeSet,
    revision: RevisionSide,
    limits: QueryLimits,
) -> Result<SideImpact, String> {
    let graph = view.index.graph();
    let mut seeds = BTreeMap::<NodeId, BTreeSet<SeedReason>>::new();
    for change in changes.symbols() {
        let reason = match change.kind {
            ChangeKind::Added => SeedReason::Added,
            ChangeKind::Deleted => SeedReason::Deleted,
            ChangeKind::BodyModified => SeedReason::BodyModified,
            ChangeKind::SignatureModified => SeedReason::SignatureModified,
            ChangeKind::Uncertain => SeedReason::Uncertain,
            ChangeKind::Unchanged => continue,
        };
        let fact = match revision {
            RevisionSide::Base => change.base.as_ref(),
            RevisionSide::Head => change.head.as_ref(),
            _ => unreachable!("two sides only"),
        };
        if let Some(fact) = fact {
            let node = NodeId::Symbol(fact.identity.clone());
            if graph.node(&node).is_none() {
                return Err("changed symbol missing from its declared graph".into());
            }
            seeds.entry(node).or_default().insert(reason);
        }
    }
    let mut unindexed_changes = Vec::new();
    for file in changes.files() {
        if file.reasons.is_empty() {
            continue;
        }
        let path = match revision {
            RevisionSide::Base => file.change.old_path.as_ref(),
            RevisionSide::Head => file.change.new_path.as_ref(),
            _ => unreachable!("two sides only"),
        };
        if let Some(path) = path {
            let node = NodeId::file(view.index.metadata().project.clone(), path.clone());
            if graph.node(&node).is_some() {
                seeds.entry(node).or_default().insert(SeedReason::FileLevel);
            } else if !changes.global_invalidation() {
                unindexed_changes.push(UnindexedChange {
                    path: path.clone(),
                    revision,
                    reason: "changed path outside this revision's indexed graph",
                });
            }
        }
    }
    let selected = seeds.keys().cloned().collect::<Vec<_>>();
    let query = DependencyGraph::new(graph)
        .query(&selected, revision, limits)
        .map_err(|e| e.to_string())?;
    let mut global_fallback = Vec::new();
    if changes.global_invalidation() {
        // All files are policy candidates already; walking reverse imports from each would
        // add redundant paths without adding candidates. Keep explicit zero-hop fallback.
        for node in graph
            .nodes()
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::File | NodeKind::TestFile))
        {
            seeds
                .entry(node.id.clone())
                .or_default()
                .insert(SeedReason::GlobalInput);
            global_fallback.push(Witness {
                seed: node.id.clone(),
                target: node.id.clone(),
                revision,
                evidence_kind: EvidenceKind::ModuleFallback,
                hops: vec![],
            });
        }
    }
    let seeds = seeds
        .into_iter()
        .map(|(node, reasons)| SeedSelection {
            node,
            revision,
            reasons: reasons.into_iter().collect(),
        })
        .collect();
    let snapshot_incomplete = view.incomplete
        || !view.diagnostics.is_empty()
        || !graph.unresolved().is_empty()
        || view
            .index
            .files()
            .iter()
            .any(|f| f.parse_status != ParseStatus::Clean || !f.diagnostics.is_empty());
    Ok(SideImpact {
        query,
        global_fallback,
        seeds,
        unindexed_changes,
        snapshot_incomplete,
        snapshot_diagnostics: view.diagnostics.to_vec(),
    })
}
/// Complete evidence is retained independently of display limits. Candidate tests are never sufficient.
pub fn analyze(
    changes: &RevisionChanges,
    base: RevisionView<'_>,
    head: RevisionView<'_>,
    limits: QueryLimits,
) -> Result<RevisionImpact, String> {
    let bound = RevisionGraphs::new(changes.revisions().clone(), base.index, head.index)
        .map_err(|e| e.to_string())?;
    validate_view(&base)?;
    validate_view(&head)?;
    let change_set = changes::classify(changes, endpoint(&base)?, endpoint(&head)?)
        .map_err(|e| e.to_string())?;
    let old = side(&base, &change_set, RevisionSide::Base, limits)?;
    let new = side(&head, &change_set, RevisionSide::Head, limits)?;
    let mut witnesses = Vec::new();
    for s in [&old, &new] {
        witnesses.extend(s.query.impact.evidence.symbol_backed().iter().cloned());
        witnesses.extend(s.query.impact.evidence.module_fallback().iter().cloned());
        witnesses.extend(s.global_fallback.iter().cloned());
    }
    let evidence = EvidenceSets::from_witnesses(witnesses);
    let mut rows = BTreeMap::<NodeId, ((usize, Confidence), BTreeSet<RevisionSide>)>::new();
    for witness in evidence
        .symbol_backed()
        .iter()
        .chain(evidence.module_fallback())
    {
        bound.validate_witness(witness).map_err(|e| e.to_string())?;
        let value = (witness.distance(), witness.confidence());
        let entry = rows
            .entry(witness.target.clone())
            .or_insert((value, BTreeSet::new()));
        entry.0 = entry.0.min(value);
        entry.1.insert(witness.revision);
    }
    let mut rows = rows
        .into_iter()
        .map(
            |(target, ((distance, confidence), revisions))| RevisionRow {
                target,
                distance,
                confidence,
                revisions,
            },
        )
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        (a.distance, a.confidence, &a.target).cmp(&(b.distance, b.confidence, &b.target))
    });
    let is_test = |id: &NodeId| {
        base.index
            .graph()
            .node(id)
            .is_some_and(|n| n.kind == NodeKind::TestFile)
            || head
                .index
                .graph()
                .node(id)
                .is_some_and(|n| n.kind == NodeKind::TestFile)
    };
    let symbol_backed = evidence
        .symbol_backed_nodes()
        .into_iter()
        .filter(is_test)
        .collect::<BTreeSet<_>>();
    let module_fallback = evidence
        .module_fallback_nodes()
        .into_iter()
        .filter(is_test)
        .collect::<BTreeSet<_>>();
    let combined = symbol_backed
        .union(&module_fallback)
        .cloned()
        .collect::<BTreeSet<_>>();
    let active_head = combined
        .iter()
        .filter(|id| {
            head.index
                .graph()
                .node(id)
                .is_some_and(|n| n.kind == NodeKind::TestFile)
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let base_only = combined
        .iter()
        .filter(|id| !head.existing_paths.contains(id.path()))
        .cloned()
        .collect();
    let present_unindexed_head = combined
        .difference(&active_head)
        .filter(|id| head.existing_paths.contains(id.path()))
        .cloned()
        .collect::<BTreeSet<_>>();
    let tests = RevisionTests {
        fallback_only: module_fallback
            .difference(&symbol_backed)
            .cloned()
            .collect(),
        symbol_backed,
        module_fallback,
        combined,
        active_head,
        base_only,
        present_unindexed_head,
        total_head_indexed_test_files: head
            .index
            .graph()
            .nodes()
            .iter()
            .filter(|n| n.kind == NodeKind::TestFile)
            .count(),
    };
    let incomplete = change_set.incomplete()
        || old.snapshot_incomplete
        || new.snapshot_incomplete
        || !old.unindexed_changes.is_empty()
        || !new.unindexed_changes.is_empty()
        || !old.query.impact.truncation.depth_boundary_nodes.is_empty()
        || !new.query.impact.truncation.depth_boundary_nodes.is_empty()
        || !tests.present_unindexed_head.is_empty();
    let display_omitted = rows.len().saturating_sub(limits.display_limit);
    Ok(RevisionImpact {
        changes: change_set,
        base: old,
        head: new,
        evidence,
        rows,
        tests,
        limits,
        display_omitted,
        incomplete,
    })
}
