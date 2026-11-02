//! Borrowed, immutable adjacency built once from a validated graph snapshot.
use ripple_core::graph::{Edge, GraphSnapshot, NodeId};

#[derive(Debug)]
pub struct DependencyGraph<'g> {
    pub(crate) snapshot: &'g GraphSnapshot,
    pub(crate) forward: Vec<Vec<usize>>,
    pub(crate) reverse: Vec<Vec<usize>>,
}
impl<'g> DependencyGraph<'g> {
    pub fn new(snapshot: &'g GraphSnapshot) -> Self {
        let mut graph = Self {
            snapshot,
            forward: vec![Vec::new(); snapshot.nodes().len()],
            reverse: vec![Vec::new(); snapshot.nodes().len()],
        };
        // Snapshot validation guarantees endpoint membership and semantic edge ordering.
        for (edge_index, edge) in snapshot.edges().iter().enumerate() {
            let source = graph.position(&edge.source).expect("validated source");
            let target = graph.position(&edge.target).expect("validated target");
            graph.forward[source].push(edge_index);
            graph.reverse[target].push(edge_index);
        }
        graph
    }
    pub fn snapshot(&self) -> &'g GraphSnapshot {
        self.snapshot
    }
    pub(crate) fn position(&self, id: &NodeId) -> Option<usize> {
        self.snapshot
            .nodes()
            .binary_search_by(|n| n.id.cmp(id))
            .ok()
    }
    /// Original dependency direction; unknown nodes have no adjacency.
    pub fn outgoing(&self, id: &NodeId) -> impl Iterator<Item = &'g Edge> + '_ {
        self.position(id)
            .into_iter()
            .flat_map(|i| self.forward[i].iter())
            .map(|&e| &self.snapshot.edges()[e])
    }
    /// Original edges targeting this node, in deterministic semantic order.
    pub fn incoming(&self, id: &NodeId) -> impl Iterator<Item = &'g Edge> + '_ {
        self.position(id)
            .into_iter()
            .flat_map(|i| self.reverse[i].iter())
            .map(|&e| &self.snapshot.edges()[e])
    }
}
