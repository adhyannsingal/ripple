//! Explainable result contracts; no traversal algorithm or test sufficiency claim.
use crate::graph::{
    Confidence, Edge, EdgeKind, GraphError, GraphSnapshot, NodeId, UnresolvedReference,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    SymbolBacked,
    ModuleFallback,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionSide {
    Worktree,
    Base,
    Head,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Witness {
    pub seed: NodeId,
    pub target: NodeId,
    pub revision: RevisionSide,
    pub evidence_kind: EvidenceKind,
    pub hops: Vec<Edge>,
}
impl Witness {
    pub fn confidence(&self) -> Confidence {
        self.hops
            .iter()
            .map(|h| h.confidence)
            .max()
            .unwrap_or(Confidence::Resolved)
    }
    pub fn distance(&self) -> usize {
        self.hops.len()
    }
    pub fn validate(&self, graph: &GraphSnapshot) -> Result<(), GraphError> {
        if graph.node(&self.seed).is_none() || graph.node(&self.target).is_none() {
            return Err(GraphError(
                "witness endpoint absent from its revision graph",
            ));
        }
        if self.evidence_kind == EvidenceKind::SymbolBacked
            && !matches!(self.seed, NodeId::Symbol(_))
        {
            return Err(GraphError("file-level changes are module fallback seeds"));
        }
        if self.evidence_kind == EvidenceKind::ModuleFallback
            && !matches!(self.target, NodeId::File { .. })
        {
            return Err(GraphError("module fallback results must be files"));
        }
        if matches!(self.seed, NodeId::Opaque { .. }) {
            return Err(GraphError("opaque boundaries cannot seed proven paths"));
        }
        let mut current = &self.seed;
        let mut visited = BTreeSet::from([current]);
        for (i, hop) in self.hops.iter().enumerate() {
            if !graph.contains_edge(hop) || &hop.target != current || !visited.insert(&hop.source) {
                return Err(GraphError("witness is missing, disconnected, or cyclic"));
            }
            let allowed = match self.evidence_kind {
                EvidenceKind::SymbolBacked => {
                    matches!(hop.kind, EdgeKind::Calls | EdgeKind::References)
                }
                EvidenceKind::ModuleFallback => {
                    matches!(hop.kind, EdgeKind::Imports | EdgeKind::Reexports)
                        || (i == 0 && hop.kind == EdgeKind::Defines)
                }
            };
            if !allowed {
                return Err(GraphError("witness edge does not match evidence track"));
            }
            current = &hop.source;
        }
        if current != &self.target {
            return Err(GraphError("witness does not terminate at result"));
        }
        Ok(())
    }
}

/// Keeps both tracks, seeds and revisions. Exact duplicate paths are deduplicated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawEvidence")]
pub struct EvidenceSets {
    symbol_backed: Vec<Witness>,
    module_fallback: Vec<Witness>,
}
#[derive(Deserialize)]
struct RawEvidence {
    symbol_backed: Vec<Witness>,
    module_fallback: Vec<Witness>,
}
impl TryFrom<RawEvidence> for EvidenceSets {
    type Error = GraphError;
    fn try_from(raw: RawEvidence) -> Result<Self, Self::Error> {
        if raw
            .symbol_backed
            .iter()
            .any(|w| w.evidence_kind != EvidenceKind::SymbolBacked)
            || raw
                .module_fallback
                .iter()
                .any(|w| w.evidence_kind != EvidenceKind::ModuleFallback)
        {
            return Err(GraphError("mislabelled evidence partition"));
        }
        Ok(Self::from_witnesses(
            raw.symbol_backed.into_iter().chain(raw.module_fallback),
        ))
    }
}
impl EvidenceSets {
    /// Structural partitioning only. Validate against the appropriate graph before use.
    pub fn from_witnesses(witnesses: impl IntoIterator<Item = Witness>) -> Self {
        let (mut symbol_backed, mut module_fallback): (Vec<_>, Vec<_>) = witnesses
            .into_iter()
            .partition(|w| w.evidence_kind == EvidenceKind::SymbolBacked);
        symbol_backed.sort();
        symbol_backed.dedup();
        module_fallback.sort();
        module_fallback.dedup();
        Self {
            symbol_backed,
            module_fallback,
        }
    }
    pub fn symbol_backed(&self) -> &[Witness] {
        &self.symbol_backed
    }
    pub fn module_fallback(&self) -> &[Witness] {
        &self.module_fallback
    }
    pub fn symbol_backed_nodes(&self) -> BTreeSet<NodeId> {
        self.symbol_backed
            .iter()
            .map(|w| w.target.clone())
            .collect()
    }
    pub fn module_fallback_nodes(&self) -> BTreeSet<NodeId> {
        self.module_fallback
            .iter()
            .map(|w| w.target.clone())
            .collect()
    }
    pub fn all_nodes(&self) -> BTreeSet<NodeId> {
        self.symbol_backed_nodes()
            .union(&self.module_fallback_nodes())
            .cloned()
            .collect()
    }
    pub fn fallback_only_nodes(&self) -> BTreeSet<NodeId> {
        self.module_fallback_nodes()
            .difference(&self.symbol_backed_nodes())
            .cloned()
            .collect()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryLimits {
    pub depth: Option<usize>,
    pub display_limit: usize,
}
impl Default for QueryLimits {
    fn default() -> Self {
        Self {
            depth: None,
            display_limit: 100,
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Truncation {
    pub depth_boundary_nodes: Vec<NodeId>,
    pub display_omitted: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactResult {
    pub evidence: EvidenceSets,
    pub unresolved: Vec<UnresolvedReference>,
    pub limits: QueryLimits,
    pub truncation: Truncation,
}
