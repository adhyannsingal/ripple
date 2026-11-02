//! Independent hand-authored graph construction; no extractor/resolver or query implementation.
use ripple_core::{
    graph::*,
    identity::{ProjectKey, QualifiedName, RepoPath, SymbolId, SymbolKind},
    source::SourceSpan,
    syntax::ReferenceKind,
};
use std::collections::BTreeMap;

pub fn file_id(path: &str) -> NodeId {
    NodeId::file(ProjectKey::single_project(), RepoPath::new(path).unwrap())
}
pub fn span() -> SourceSpan {
    SourceSpan::from_offsets("x", 0, 1).unwrap()
}
#[derive(Default)]
pub struct Fixture {
    pub nodes: BTreeMap<NodeId, GraphNode>,
    pub edges: Vec<Edge>,
    pub unresolved: Vec<UnresolvedReference>,
}
impl Fixture {
    pub fn file(&mut self, path: &str, test: bool) -> NodeId {
        let node = GraphNode::file(
            ProjectKey::single_project(),
            RepoPath::new(path).unwrap(),
            test,
        );
        let id = node.id.clone();
        self.nodes.entry(id.clone()).or_insert(node);
        id
    }
    pub fn symbol(&mut self, name: &str) -> NodeId {
        self.symbol_in(&format!("src/{name}.ts"), name)
    }
    pub fn symbol_in(&mut self, path: &str, name: &str) -> NodeId {
        let file = self.file(path, false);
        let symbol = GraphNode::symbol(
            SymbolId::new(
                ProjectKey::single_project(),
                RepoPath::new(path).unwrap(),
                QualifiedName::new([name.to_string()]).unwrap(),
                SymbolKind::Function,
            ),
            span(),
        );
        let id = symbol.id.clone();
        if self.nodes.insert(id.clone(), symbol).is_none() {
            self.edge(&file, &id, EdgeKind::Defines, Confidence::Resolved);
        }
        id
    }
    pub fn edge(
        &mut self,
        source: &NodeId,
        target: &NodeId,
        kind: EdgeKind,
        confidence: Confidence,
    ) {
        self.edges.push(Edge {
            source: source.clone(),
            target: target.clone(),
            kind,
            confidence,
            provenance: SourceLocation {
                file: source.path().clone(),
                span: span(),
            },
            resolution_rule: "independently_authored_fixture".into(),
        });
    }
    pub fn unknown(&mut self, owner: &NodeId) -> NodeId {
        let boundary = NodeId::Opaque {
            project: owner.project().clone(),
            path: owner.path().clone(),
            reference_key: "computed:0".into(),
        };
        self.nodes.insert(
            boundary.clone(),
            GraphNode {
                id: boundary.clone(),
                kind: NodeKind::OpaqueBoundary,
                span: Some(span()),
            },
        );
        self.unresolved.push(UnresolvedReference {
            source: owner.clone(),
            boundary: boundary.clone(),
            text: "handlers[event]()".into(),
            reference_kind: ReferenceKind::ComputedCall,
            provenance: SourceLocation {
                file: owner.path().clone(),
                span: span(),
            },
            reason: "computed target is unknown".into(),
        });
        boundary
    }
    pub fn finish(self) -> GraphSnapshot {
        GraphSnapshot::new(
            self.nodes.into_values().collect(),
            self.edges,
            self.unresolved,
        )
        .unwrap()
    }
}
