//! Independent semantic and file-fallback BFS; no resolver, database or filesystem access.
use crate::graph::DependencyGraph;
use ripple_core::{
    graph::{Confidence, Edge, EdgeKind, GraphError, NodeId, NodeKind},
    results::{
        EvidenceKind, EvidenceSets, ImpactResult, QueryLimits, RevisionSide, Truncation, Witness,
    },
};
use std::collections::{BTreeMap, BTreeSet};

/// A reporting threshold only: hubs never prune traversal.
pub const HUB_THRESHOLD: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactRow {
    pub target: NodeId,
    /// Best witness ordered by distance, confidence, then semantic path order.
    pub distance: usize,
    pub confidence: Confidence,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hub {
    pub node: NodeId,
    pub direct_dependents: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraversalSummary {
    pub seed: NodeId,
    pub revision: RevisionSide,
    pub evidence_kind: EvidenceKind,
    /// Includes the start and the initial symbol-to-file bridge when applicable.
    pub visited_nodes: usize,
    pub nodes_by_depth: BTreeMap<usize, usize>,
    pub hubs: Vec<Hub>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestReachability {
    pub symbol_backed: BTreeSet<NodeId>,
    pub module_fallback: BTreeSet<NodeId>,
    pub fallback_only: BTreeSet<NodeId>,
    pub combined: BTreeSet<NodeId>,
    pub total_test_files: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraversalResult {
    /// Complete evidence within the requested depth, independent of display limiting.
    /// Unresolved records are graph-wide uncertainty, not fabricated reachable paths.
    pub impact: ImpactResult,
    /// Complete distinct targets sorted by distance, confidence and semantic node identity.
    pub rows: Vec<ImpactRow>,
    pub tests: TestReachability,
    pub summaries: Vec<TraversalSummary>,
}
impl TraversalResult {
    pub fn displayed(&self) -> &[ImpactRow] {
        &self.rows[..self.rows.len().min(self.impact.limits.display_limit)]
    }
}

// At most two labels per node at its shortest distance. Both are necessary: a later
// candidate hop can erase a confidence advantage, making an earlier weaker but
// lexicographically smaller prefix the correct tie-break winner.
#[derive(Debug, Clone, Copy)]
struct PathState {
    previous: Option<(usize, usize, usize)>, // node, confidence label, original edge index
    order: (usize, usize),                   // predecessor path rank, edge index
    rank: usize,
}
struct Walk {
    distances: Vec<Option<usize>>,
    states: Vec<[Option<PathState>; 2]>,
    boundary: BTreeSet<NodeId>,
}
fn allowed(edge: &Edge, track: EvidenceKind, at_seed: bool) -> bool {
    match track {
        EvidenceKind::SymbolBacked => matches!(edge.kind, EdgeKind::Calls | EdgeKind::References),
        EvidenceKind::ModuleFallback => {
            matches!(edge.kind, EdgeKind::Imports | EdgeKind::Reexports)
                || (at_seed && edge.kind == EdgeKind::Defines)
        }
    }
}
impl DependencyGraph<'_> {
    /// Missing or opaque seeds fail explicitly. Repeated seeds are deduplicated.
    /// Zero-hop seed evidence is retained; file seeds run only the fallback track.
    pub fn query(
        &self,
        seeds: &[NodeId],
        revision: RevisionSide,
        limits: QueryLimits,
    ) -> Result<TraversalResult, GraphError> {
        let seeds = seeds.iter().collect::<BTreeSet<_>>();
        for seed in &seeds {
            if self.position(seed).is_none() {
                return Err(GraphError("query seed is absent from this revision graph"));
            }
            if matches!(seed, NodeId::Opaque { .. }) {
                return Err(GraphError(
                    "opaque boundaries cannot seed dependency traversal",
                ));
            }
        }
        let mut witnesses = Vec::new();
        let mut summaries = Vec::new();
        let mut boundary = BTreeSet::new();
        for seed in seeds {
            let seed_index = self.position(seed).expect("checked seed");
            for track in [EvidenceKind::SymbolBacked, EvidenceKind::ModuleFallback] {
                if track == EvidenceKind::SymbolBacked && !matches!(seed, NodeId::Symbol(_)) {
                    continue;
                }
                let walk = self.walk(seed_index, track, limits.depth);
                boundary.extend(walk.boundary.iter().cloned());
                let mut counts = BTreeMap::new();
                let mut hubs = Vec::new();
                for (i, distance) in walk.distances.iter().enumerate() {
                    let Some(distance) = distance else { continue };
                    *counts.entry(*distance).or_default() += 1;
                    let id = &self.snapshot.nodes()[i].id;
                    let fanout = self.reverse[i]
                        .iter()
                        .map(|&e| &self.snapshot.edges()[e])
                        .filter(|e| allowed(e, track, i == seed_index) && e.source != *id)
                        .map(|e| &e.source)
                        .collect::<BTreeSet<_>>()
                        .len();
                    if fanout >= HUB_THRESHOLD {
                        hubs.push(Hub {
                            node: id.clone(),
                            direct_dependents: fanout,
                        });
                    }
                    if track == EvidenceKind::ModuleFallback && !matches!(id, NodeId::File { .. }) {
                        continue;
                    }
                    let confidence = usize::from(walk.states[i][0].is_none());
                    let mut hops = Vec::with_capacity(*distance);
                    let (mut node, mut label) = (i, confidence);
                    while let Some((previous, previous_label, edge)) = walk.states[node][label]
                        .expect("visited path label")
                        .previous
                    {
                        hops.push(self.snapshot.edges()[edge].clone());
                        (node, label) = (previous, previous_label);
                    }
                    hops.reverse();
                    witnesses.push(Witness {
                        seed: seed.clone(),
                        target: id.clone(),
                        revision,
                        evidence_kind: track,
                        hops,
                    });
                }
                summaries.push(TraversalSummary {
                    seed: seed.clone(),
                    revision,
                    evidence_kind: track,
                    visited_nodes: walk.distances.iter().flatten().count(),
                    nodes_by_depth: counts,
                    hubs,
                });
            }
        }
        let evidence = EvidenceSets::from_witnesses(witnesses);
        let is_test = |id: &NodeId| {
            self.snapshot
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
        let tests = TestReachability {
            combined: symbol_backed.union(&module_fallback).cloned().collect(),
            fallback_only: module_fallback
                .difference(&symbol_backed)
                .cloned()
                .collect(),
            symbol_backed,
            module_fallback,
            total_test_files: self
                .snapshot
                .nodes()
                .iter()
                .filter(|n| n.kind == NodeKind::TestFile)
                .count(),
        };
        let mut best = BTreeMap::<NodeId, (usize, Confidence)>::new();
        for witness in evidence
            .symbol_backed()
            .iter()
            .chain(evidence.module_fallback())
        {
            let value = (witness.distance(), witness.confidence());
            best.entry(witness.target.clone())
                .and_modify(|old| *old = (*old).min(value))
                .or_insert(value);
        }
        let mut rows = best
            .into_iter()
            .map(|(target, (distance, confidence))| ImpactRow {
                target,
                distance,
                confidence,
            })
            .collect::<Vec<_>>();
        rows.sort_by(|a, b| {
            (a.distance, a.confidence, &a.target).cmp(&(b.distance, b.confidence, &b.target))
        });
        let display_omitted = rows.len().saturating_sub(limits.display_limit);
        Ok(TraversalResult {
            impact: ImpactResult {
                evidence,
                unresolved: self.snapshot.unresolved().to_vec(),
                limits,
                truncation: Truncation {
                    depth_boundary_nodes: boundary.into_iter().collect(),
                    display_omitted,
                },
            },
            rows,
            tests,
            summaries,
        })
    }
    fn walk(&self, seed: usize, track: EvidenceKind, depth: Option<usize>) -> Walk {
        let n = self.snapshot.nodes().len();
        let mut result = Walk {
            distances: vec![None; n],
            states: vec![[None; 2]; n],
            boundary: BTreeSet::new(),
        };
        result.distances[seed] = Some(0);
        result.states[seed][0] = Some(PathState {
            previous: None,
            order: (0, 0),
            rank: 0,
        });
        let mut frontier = vec![seed];
        let mut distance = 0;
        while !frontier.is_empty() {
            let mut next = BTreeSet::new();
            for &current in &frontier {
                for &edge_index in &self.reverse[current] {
                    let edge = &self.snapshot.edges()[edge_index];
                    if !allowed(edge, track, current == seed) {
                        continue;
                    }
                    let target = self.position(&edge.source).expect("validated endpoint");
                    if depth.is_some_and(|limit| distance >= limit) {
                        // Cycles and nodes already reached at a shorter distance do not imply truncation.
                        if result.distances[target].is_none() {
                            result
                                .boundary
                                .insert(self.snapshot.nodes()[current].id.clone());
                        }
                        continue;
                    }
                    if result.distances[target].is_some_and(|d| d < distance + 1) {
                        continue;
                    }
                    result.distances[target] = Some(distance + 1);
                    next.insert(target);
                    for label in 0..2 {
                        let Some(parent) = result.states[current][label] else {
                            continue;
                        };
                        let next_label =
                            label.max(usize::from(edge.confidence == Confidence::Candidate));
                        let state = PathState {
                            previous: Some((current, label, edge_index)),
                            order: (parent.rank, edge_index),
                            rank: 0,
                        };
                        let old = &mut result.states[target][next_label];
                        if old.is_none_or(|old| state.order < old.order) {
                            *old = Some(state)
                        }
                    }
                }
            }
            // Rank equal-length full paths, not just final edges or parent node names.
            let orders = next
                .iter()
                .flat_map(|&i| result.states[i].iter().flatten().map(|s| s.order))
                .collect::<BTreeSet<_>>();
            let ranks = orders
                .into_iter()
                .enumerate()
                .map(|(rank, order)| (order, rank))
                .collect::<BTreeMap<_, _>>();
            for &i in &next {
                for state in result.states[i].iter_mut().flatten() {
                    state.rank = ranks[&state.order]
                }
            }
            frontier = next.into_iter().collect();
            distance += 1;
        }
        result
    }
}
