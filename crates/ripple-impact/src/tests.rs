//! Pure, revision-aware potentially related test selection. Never a sufficiency proof.
use crate::revision_impact::RevisionImpact;
use ripple_core::{graph::NodeId, identity::RepoPath, results::Witness};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct TestPartitions {
    pub symbol_backed: BTreeSet<RepoPath>,
    pub module_fallback: BTreeSet<RepoPath>,
    pub fallback_only: BTreeSet<RepoPath>,
    pub combined: BTreeSet<RepoPath>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TestPresence {
    ActiveHead,
    AbsentHead,
    PresentUnindexedHead,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TestFileEvidence {
    pub file: NodeId,
    pub presence: TestPresence,
    /// All seed/revision witnesses are retained, independently for both tracks.
    pub symbol_backed: Vec<Witness>,
    pub module_fallback: Vec<Witness>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TestCompleteness {
    /// Display limits never drop machine-readable paths or witnesses.
    pub selected_paths_complete_within_depth: bool,
    pub requested_depth: Option<usize>,
    pub depth_truncated: bool,
    pub analysis_incomplete: bool,
    /// Context only; details remain in the corresponding RevisionImpact endpoint.
    pub reasons: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TestImpact {
    /// Union of base/head evidence restricted to supported indexed head test files.
    pub active_head: TestPartitions,
    pub base_only: TestPartitions,
    pub present_unindexed_head: TestPartitions,
    pub total_head_test_files: usize,
    pub files: Vec<TestFileEvidence>,
    pub completeness: TestCompleteness,
}
fn partition(result: &RevisionImpact, membership: &BTreeSet<NodeId>) -> TestPartitions {
    let tests = result.tests();
    let paths = |set: &BTreeSet<NodeId>| {
        set.intersection(membership)
            .map(|id| id.path().clone())
            .collect()
    };
    TestPartitions {
        symbol_backed: paths(&tests.symbol_backed),
        module_fallback: paths(&tests.module_fallback),
        fallback_only: paths(&tests.fallback_only),
        combined: paths(&tests.combined),
    }
}
/// Projection of already validated revision evidence; no filesystem, resolver or Git access.
/// Old paths that survive at head remain selected even if their old relationship was removed.
pub fn select(result: &RevisionImpact) -> TestImpact {
    let tests = result.tests();
    let mut by_file = tests
        .combined
        .iter()
        .map(|file| {
            (
                file.clone(),
                TestFileEvidence {
                    file: file.clone(),
                    presence: if tests.active_head.contains(file) {
                        TestPresence::ActiveHead
                    } else if tests.base_only.contains(file) {
                        TestPresence::AbsentHead
                    } else {
                        TestPresence::PresentUnindexedHead
                    },
                    symbol_backed: Vec::new(),
                    module_fallback: Vec::new(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    for witness in result.evidence().symbol_backed() {
        if let Some(file) = by_file.get_mut(&witness.target) {
            file.symbol_backed.push(witness.clone());
        }
    }
    for witness in result.evidence().module_fallback() {
        if let Some(file) = by_file.get_mut(&witness.target) {
            file.module_fallback.push(witness.clone());
        }
    }
    let files = by_file.into_values().collect::<Vec<_>>();
    let mut reasons = BTreeSet::new();
    let mut depth_truncated = false;
    for (name, side) in [("base", result.base()), ("head", result.head())] {
        if side.snapshot_incomplete {
            reasons.insert(format!("{name}: snapshot analysis incomplete"));
        }
        if !side.query.impact.unresolved.is_empty() {
            reasons.insert(format!("{name}: graph-wide unresolved references"));
        }
        if !side.unindexed_changes.is_empty() {
            reasons.insert(format!("{name}: changes outside indexed graph"));
        }
        if !side.query.impact.truncation.depth_boundary_nodes.is_empty() {
            depth_truncated = true;
            reasons.insert(format!(
                "{name}: depth boundary reached; selection may omit related tests"
            ));
        }
    }
    if result.changes().incomplete() {
        reasons.insert("change classification incomplete".into());
    }
    if !tests.present_unindexed_head.is_empty() {
        reasons.insert("selected old test paths are present but unindexed at head".into());
    }
    if files
        .iter()
        .flat_map(|f| f.symbol_backed.iter().chain(&f.module_fallback))
        .any(|w| w.confidence() == ripple_core::graph::Confidence::Candidate)
    {
        reasons.insert("candidate-confidence test witnesses".into());
    }
    let analysis_incomplete = result.incomplete() || !reasons.is_empty();
    TestImpact {
        active_head: partition(result, &tests.active_head),
        base_only: partition(result, &tests.base_only),
        present_unindexed_head: partition(result, &tests.present_unindexed_head),
        total_head_test_files: tests.total_head_indexed_test_files,
        files,
        completeness: TestCompleteness {
            selected_paths_complete_within_depth: true,
            requested_depth: result.base().query.impact.limits.depth,
            depth_truncated,
            analysis_incomplete,
            reasons: reasons.into_iter().collect(),
        },
    }
}
