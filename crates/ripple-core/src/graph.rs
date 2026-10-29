//! Durable, backend-independent graph records. Adjacency/traversal belong to ripple-impact.
use crate::{
    identity::{ProjectKey, RepoPath, SymbolId, SymbolKind},
    source::SourceSpan,
    syntax::ReferenceKind,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "identity",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum NodeId {
    File {
        project: ProjectKey,
        path: RepoPath,
    },
    Symbol(SymbolId),
    Opaque {
        project: ProjectKey,
        path: RepoPath,
        reference_key: String,
    },
}
impl NodeId {
    pub fn file(project: ProjectKey, path: RepoPath) -> Self {
        Self::File { project, path }
    }
    pub fn path(&self) -> &RepoPath {
        match self {
            Self::File { path, .. } | Self::Opaque { path, .. } => path,
            Self::Symbol(id) => id.path(),
        }
    }
    pub fn project(&self) -> &ProjectKey {
        match self {
            Self::File { project, .. } | Self::Opaque { project, .. } => project,
            Self::Symbol(id) => id.project(),
        }
    }
    pub fn defining_file(&self) -> Self {
        Self::file(self.project().clone(), self.path().clone())
    }
    pub fn semantic_key(&self) -> String {
        let (prefix, fields) = match self {
            Self::Symbol(id) => return id.semantic_key(),
            Self::File { project, path } => ("file:v1:", vec![project.as_str(), path.as_str()]),
            Self::Opaque {
                project,
                path,
                reference_key,
            } => (
                "opaque:v1:",
                vec![project.as_str(), path.as_str(), reference_key.as_str()],
            ),
        };
        let mut key = prefix.to_owned();
        for field in fields {
            key.push_str(&format!("{}:{}", field.len(), field));
        }
        key
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NodeKind {
    File,
    Function,
    Class,
    Method,
    TestFile,
    OpaqueBoundary,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: NodeId,
    pub kind: NodeKind,
    pub span: Option<SourceSpan>,
}
impl GraphNode {
    pub fn file(project: ProjectKey, path: RepoPath, is_test: bool) -> Self {
        Self {
            id: NodeId::file(project, path),
            kind: if is_test {
                NodeKind::TestFile
            } else {
                NodeKind::File
            },
            span: None,
        }
    }
    pub fn symbol(id: SymbolId, span: SourceSpan) -> Self {
        let kind = match id.kind() {
            SymbolKind::Function => NodeKind::Function,
            SymbolKind::Class => NodeKind::Class,
            SymbolKind::Method => NodeKind::Method,
        };
        Self {
            id: NodeId::Symbol(id),
            kind,
            span: Some(span),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EdgeKind {
    Defines,
    Imports,
    Calls,
    References,
    Reexports,
}
/// Ordered strongest to weakest; unresolved references have no guessed target edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Confidence {
    Resolved,
    Candidate,
    Unresolved,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: RepoPath,
    pub span: SourceSpan,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Edge {
    pub source: NodeId,
    pub target: NodeId,
    pub kind: EdgeKind,
    pub confidence: Confidence,
    pub provenance: SourceLocation,
    pub resolution_rule: String,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UnresolvedReference {
    pub source: NodeId,
    pub boundary: NodeId,
    pub text: String,
    pub reference_kind: ReferenceKind,
    pub provenance: SourceLocation,
    pub reason: String,
}
impl UnresolvedReference {
    pub fn confidence(&self) -> Confidence {
        Confidence::Unresolved
    }
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid graph contract: {0}")]
pub struct GraphError(pub &'static str);

/// Sorted semantic records, with no DB IDs or backend handles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawGraph")]
pub struct GraphSnapshot {
    nodes: Vec<GraphNode>,
    edges: Vec<Edge>,
    unresolved: Vec<UnresolvedReference>,
}
#[derive(Deserialize)]
struct RawGraph {
    nodes: Vec<GraphNode>,
    edges: Vec<Edge>,
    unresolved: Vec<UnresolvedReference>,
}
impl TryFrom<RawGraph> for GraphSnapshot {
    type Error = GraphError;
    fn try_from(raw: RawGraph) -> Result<Self, Self::Error> {
        Self::new(raw.nodes, raw.edges, raw.unresolved)
    }
}
impl GraphSnapshot {
    pub fn new(
        mut nodes: Vec<GraphNode>,
        mut edges: Vec<Edge>,
        mut unresolved: Vec<UnresolvedReference>,
    ) -> Result<Self, GraphError> {
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        edges.sort();
        unresolved.sort();
        if nodes.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(GraphError("duplicate semantic node"));
        }
        if edges.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(GraphError("duplicate edge/provenance"));
        }
        let graph = Self {
            nodes,
            edges,
            unresolved,
        };
        for node in &graph.nodes {
            let valid = match (&node.id, node.kind) {
                (NodeId::File { .. }, NodeKind::File | NodeKind::TestFile) => true,
                (NodeId::Symbol(id), kind) => {
                    node.span.is_some()
                        && matches!(
                            (id.kind(), kind),
                            (SymbolKind::Function, NodeKind::Function)
                                | (SymbolKind::Class, NodeKind::Class)
                                | (SymbolKind::Method, NodeKind::Method)
                        )
                }
                (NodeId::Opaque { reference_key, .. }, NodeKind::OpaqueBoundary) => {
                    node.span.is_some()
                        && !reference_key.is_empty()
                        && !reference_key.chars().any(char::is_control)
                }
                _ => false,
            };
            if !valid {
                return Err(GraphError("node identity/kind/span mismatch"));
            }
            if graph.node(&node.id.defining_file()).is_none() {
                return Err(GraphError("node has no defining file"));
            }
        }
        let mut definitions = BTreeMap::<NodeId, usize>::new();
        for edge in &graph.edges {
            if graph.node(&edge.target).is_none() {
                return Err(GraphError("missing target node"));
            }
            graph.validate_location(&edge.source, &edge.provenance)?;
            if edge.source.project() != edge.target.project() {
                return Err(GraphError("cross-project edge unsupported"));
            }
            if edge.resolution_rule.trim().is_empty() || edge.confidence == Confidence::Unresolved {
                return Err(GraphError(
                    "targeted edge needs a rule and resolved/candidate evidence",
                ));
            }
            let valid = match edge.kind {
                EdgeKind::Defines => {
                    matches!(
                        (&edge.source, &edge.target),
                        (NodeId::File { .. }, NodeId::Symbol(_))
                    ) && edge.source == edge.target.defining_file()
                        && edge.confidence == Confidence::Resolved
                }
                EdgeKind::Imports | EdgeKind::Reexports => matches!(
                    (&edge.source, &edge.target),
                    (NodeId::File { .. }, NodeId::File { .. })
                ),
                EdgeKind::Calls | EdgeKind::References => matches!(
                    (&edge.source, &edge.target),
                    (NodeId::File { .. } | NodeId::Symbol(_), NodeId::Symbol(_))
                ),
            };
            if !valid {
                return Err(GraphError("edge endpoints do not match edge kind"));
            }
            if edge.kind == EdgeKind::Defines {
                *definitions.entry(edge.target.clone()).or_default() += 1;
            }
        }
        let mut boundaries = BTreeSet::new();
        for reference in &graph.unresolved {
            graph.validate_location(&reference.source, &reference.provenance)?;
            let boundary = graph
                .node(&reference.boundary)
                .ok_or(GraphError("missing opaque boundary"))?;
            if !matches!(reference.source, NodeId::File { .. } | NodeId::Symbol(_))
                || !matches!(boundary.id, NodeId::Opaque { .. })
                || boundary.id.defining_file() != reference.source.defining_file()
                || boundary.span != Some(reference.provenance.span)
                || reference.reason.trim().is_empty()
                || reference.text.trim().is_empty()
                || !boundaries.insert(&reference.boundary)
            {
                return Err(GraphError(
                    "invalid or duplicate unresolved boundary evidence",
                ));
            }
        }
        for node in &graph.nodes {
            if matches!(node.id, NodeId::Symbol(_)) && definitions.get(&node.id) != Some(&1) {
                return Err(GraphError("symbol requires exactly one defining edge"));
            }
            if matches!(node.id, NodeId::Opaque { .. }) && !boundaries.contains(&node.id) {
                return Err(GraphError("opaque node requires unresolved evidence"));
            }
        }
        Ok(graph)
    }
    fn validate_location(
        &self,
        source: &NodeId,
        location: &SourceLocation,
    ) -> Result<(), GraphError> {
        let node = self.node(source).ok_or(GraphError("missing source node"))?;
        if source.path() != &location.file
            || node.span.is_some_and(|span| !span.encloses(location.span))
        {
            return Err(GraphError("provenance outside source owner"));
        }
        Ok(())
    }
    pub fn nodes(&self) -> &[GraphNode] {
        &self.nodes
    }
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }
    pub fn unresolved(&self) -> &[UnresolvedReference] {
        &self.unresolved
    }
    pub fn node(&self, id: &NodeId) -> Option<&GraphNode> {
        self.nodes
            .binary_search_by(|node| node.id.cmp(id))
            .ok()
            .map(|i| &self.nodes[i])
    }
    pub fn contains_edge(&self, edge: &Edge) -> bool {
        self.edges.binary_search(edge).is_ok()
    }
}
