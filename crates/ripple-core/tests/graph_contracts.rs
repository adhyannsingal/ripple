use ripple_core::{
    graph::*,
    identity::{ProjectKey, QualifiedName, RepoPath, SymbolId, SymbolKind},
    results::*,
    source::SourceSpan,
    syntax::ReferenceKind,
};

const LIB: &str = include_str!("../../../fixtures/unrelated-exports/project/src/lib.ts");
const TEST_A: &str = include_str!("../../../fixtures/unrelated-exports/project/tests/a.test.ts");
const TEST_B: &str = include_str!("../../../fixtures/unrelated-exports/project/tests/b.test.ts");
fn path(s: &str) -> RepoPath {
    RepoPath::new(s).unwrap()
}
fn file(p: &str, is_test: bool) -> GraphNode {
    GraphNode::file(ProjectKey::single_project(), path(p), is_test)
}
fn span(s: &str, a: usize, b: usize) -> SourceSpan {
    SourceSpan::from_offsets(s, a, b).unwrap()
}
fn symbol(name: &str, location: SourceSpan) -> GraphNode {
    GraphNode::symbol(
        SymbolId::new(
            ProjectKey::single_project(),
            path("src/lib.ts"),
            QualifiedName::new([name.into()]).unwrap(),
            SymbolKind::Function,
        ),
        location,
    )
}
fn edge(source: &GraphNode, target: &GraphNode, kind: EdgeKind, location: SourceSpan) -> Edge {
    Edge {
        source: source.id.clone(),
        target: target.id.clone(),
        kind,
        confidence: Confidence::Resolved,
        provenance: SourceLocation {
            file: source.id.path().clone(),
            span: location,
        },
        resolution_rule: "fixture_rule".into(),
    }
}
fn example() -> (
    GraphSnapshot,
    NodeId,
    NodeId,
    NodeId,
    Edge,
    Edge,
    Edge,
    Edge,
) {
    let lib = file("src/lib.ts", false);
    let a = symbol("a", span(LIB, 0, LIB.find('\n').unwrap()));
    let b = symbol("b", span(LIB, LIB.find('\n').unwrap() + 1, LIB.len()));
    let ta = file("tests/a.test.ts", true);
    let tb = file("tests/b.test.ts", true);
    let da = edge(&lib, &a, EdgeKind::Defines, a.span.unwrap());
    let db = edge(&lib, &b, EdgeKind::Defines, b.span.unwrap());
    let ca = edge(
        &ta,
        &a,
        EdgeKind::Calls,
        span(TEST_A, TEST_A.rfind("a()").unwrap(), TEST_A.len() - 1),
    );
    let cb = edge(
        &tb,
        &b,
        EdgeKind::Calls,
        span(TEST_B, TEST_B.rfind("b()").unwrap(), TEST_B.len() - 1),
    );
    let ia = edge(
        &ta,
        &lib,
        EdgeKind::Imports,
        span(TEST_A, 0, TEST_A.find('\n').unwrap()),
    );
    let ib = edge(
        &tb,
        &lib,
        EdgeKind::Imports,
        span(TEST_B, 0, TEST_B.find('\n').unwrap()),
    );
    let graph = GraphSnapshot::new(
        vec![tb.clone(), b, lib, ta.clone(), a.clone()],
        vec![ib.clone(), ca.clone(), db, ia.clone(), cb, da.clone()],
        vec![],
    )
    .unwrap();
    (graph, a.id, ta.id, tb.id, ca, da, ia, ib)
}
fn witness(seed: &NodeId, target: &NodeId, kind: EvidenceKind, hops: Vec<Edge>) -> Witness {
    Witness {
        seed: seed.clone(),
        target: target.clone(),
        revision: RevisionSide::Worktree,
        evidence_kind: kind,
        hops,
    }
}

#[test]
fn unrelated_exports_keep_precise_and_fallback_evidence_separate() {
    let (graph, seed, ta, tb, ca, da, ia, ib) = example();
    let precise = witness(&seed, &ta, EvidenceKind::SymbolBacked, vec![ca]);
    let fa = witness(
        &seed,
        &ta,
        EvidenceKind::ModuleFallback,
        vec![da.clone(), ia],
    );
    let fb = witness(&seed, &tb, EvidenceKind::ModuleFallback, vec![da, ib]);
    for w in [&precise, &fa, &fb] {
        w.validate(&graph).unwrap();
    }
    let sets = EvidenceSets::from_witnesses([fb, precise.clone(), fa, precise]);
    assert_eq!(
        sets.symbol_backed_nodes().into_iter().collect::<Vec<_>>(),
        [ta]
    );
    assert_eq!(
        sets.fallback_only_nodes().into_iter().collect::<Vec<_>>(),
        [tb]
    );
    assert_eq!(sets.all_nodes().len(), 2);
    assert_eq!(sets.symbol_backed().len(), 1);
    assert_eq!(sets.module_fallback().len(), 2);
    assert!(
        sets.module_fallback()
            .iter()
            .all(|w| w.confidence() == Confidence::Resolved)
    );
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/unrelated-exports/expected.json"
    ))
    .unwrap();
    assert_eq!(
        sets.symbol_backed_nodes()
            .iter()
            .map(|n| n.path().as_str())
            .collect::<Vec<_>>(),
        [expected["symbol_backed_tests"][0].as_str().unwrap()]
    );
    assert_eq!(
        sets,
        serde_json::from_str(&serde_json::to_string(&sets).unwrap()).unwrap()
    );
}

#[test]
fn fallback_and_file_change_cannot_masquerade_as_symbol_paths() {
    let (graph, seed, ta, _, _, da, ia, _) = example();
    assert!(
        witness(&seed, &ta, EvidenceKind::SymbolBacked, vec![da, ia.clone()])
            .validate(&graph)
            .is_err()
    );
    let lib = ia.target.clone();
    assert!(
        witness(&lib, &ta, EvidenceKind::ModuleFallback, vec![ia.clone()])
            .validate(&graph)
            .is_ok()
    );
    assert!(
        witness(&lib, &ta, EvidenceKind::SymbolBacked, vec![ia])
            .validate(&graph)
            .is_err()
    );
}

#[test]
fn graph_is_deterministic_round_trips_and_has_no_numeric_storage_ids() {
    let (graph, _, _, _, _, _, _, _) = example();
    let other = GraphSnapshot::new(
        graph.nodes().iter().rev().cloned().collect(),
        graph.edges().iter().rev().cloned().collect(),
        vec![],
    )
    .unwrap();
    assert_eq!(graph, other);
    assert_eq!(
        serde_json::to_string(&graph).unwrap(),
        serde_json::to_string(&other).unwrap()
    );
    assert_eq!(
        graph,
        serde_json::from_str(&serde_json::to_string(&graph).unwrap()).unwrap()
    );
    let test_node = graph
        .node(&NodeId::file(
            ProjectKey::single_project(),
            path("tests/a.test.ts"),
        ))
        .unwrap();
    assert_eq!(test_node.kind, NodeKind::TestFile);
    assert!(test_node.id.semantic_key().starts_with("file:v1:"));
}

#[test]
fn candidates_keep_the_weakest_hop_and_revision_evidence_survives_dedup() {
    let (graph, seed, ta, _, mut ca, _, _, _) = example();
    let mut edges = graph.edges().to_vec();
    edges.retain(|e| e != &ca);
    ca.confidence = Confidence::Candidate;
    edges.push(ca.clone());
    let graph = GraphSnapshot::new(graph.nodes().to_vec(), edges, vec![]).unwrap();
    let w = witness(&seed, &ta, EvidenceKind::SymbolBacked, vec![ca]);
    w.validate(&graph).unwrap();
    assert_eq!(w.confidence(), Confidence::Candidate);
    assert_eq!(w.distance(), 1);
    let base = Witness {
        revision: RevisionSide::Base,
        ..w.clone()
    };
    let sets = EvidenceSets::from_witnesses([w, base]);
    assert_eq!(sets.symbol_backed().len(), 2);
    assert_eq!(sets.symbol_backed_nodes().len(), 1);
}

#[test]
fn witnesses_reject_disconnected_paths_fake_edges_wrong_targets_and_cycles() {
    let (graph, seed, ta, tb, ca, da, ia, _) = example();
    assert!(
        witness(&seed, &ta, EvidenceKind::ModuleFallback, vec![ia, da])
            .validate(&graph)
            .is_err()
    );
    let mut forged = ca.clone();
    forged.resolution_rule = "not_in_graph".into();
    assert!(
        witness(&seed, &ta, EvidenceKind::SymbolBacked, vec![forged])
            .validate(&graph)
            .is_err()
    );
    assert!(
        witness(&seed, &tb, EvidenceKind::SymbolBacked, vec![ca.clone()])
            .validate(&graph)
            .is_err()
    );
    let b = graph
        .nodes()
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.id != seed)
        .unwrap();
    let a = graph.node(&seed).unwrap();
    let ab = edge(a, b, EdgeKind::Calls, a.span.unwrap());
    let ba = edge(b, a, EdgeKind::Calls, b.span.unwrap());
    let mut edges = graph.edges().to_vec();
    edges.extend([ab.clone(), ba.clone()]);
    let cyclic_graph = GraphSnapshot::new(graph.nodes().to_vec(), edges, vec![]).unwrap();
    assert!(
        witness(&seed, &seed, EvidenceKind::SymbolBacked, vec![ba, ab])
            .validate(&cyclic_graph)
            .is_err()
    );
}

#[test]
fn unresolved_references_survive_round_trip_without_inventing_a_target() {
    let source = file("src/dynamic.ts", false);
    let text = "handlers[event](payload)";
    let location = span(text, 0, text.len());
    let boundary = GraphNode {
        id: NodeId::Opaque {
            project: ProjectKey::single_project(),
            path: path("src/dynamic.ts"),
            reference_key: "computed-call:0".into(),
        },
        kind: NodeKind::OpaqueBoundary,
        span: Some(location),
    };
    let unresolved = UnresolvedReference {
        source: source.id.clone(),
        boundary: boundary.id.clone(),
        text: text.into(),
        reference_kind: ReferenceKind::ComputedCall,
        provenance: SourceLocation {
            file: path("src/dynamic.ts"),
            span: location,
        },
        reason: "computed receiver has no supported static target".into(),
    };
    let graph = GraphSnapshot::new(
        vec![source.clone(), boundary.clone()],
        vec![],
        vec![unresolved],
    )
    .unwrap();
    assert_eq!(graph.unresolved()[0].confidence(), Confidence::Unresolved);
    assert!(graph.edges().is_empty());
    assert_eq!(
        graph,
        serde_json::from_str(&serde_json::to_string(&graph).unwrap()).unwrap()
    );
    assert!(GraphSnapshot::new(vec![source, boundary], vec![], vec![]).is_err());
}

#[test]
fn malformed_graph_rows_cannot_bypass_validation_through_json() {
    let (graph, _, _, _, ca, _, _, _) = example();
    let mut nodes = graph.nodes().to_vec();
    nodes.push(nodes[0].clone());
    assert!(GraphSnapshot::new(nodes, graph.edges().to_vec(), vec![]).is_err());
    let mut edges = graph.edges().to_vec();
    edges.push(ca);
    assert!(GraphSnapshot::new(graph.nodes().to_vec(), edges, vec![]).is_err());
    let mut json = serde_json::to_value(&graph).unwrap();
    json["edges"][0]["confidence"] = serde_json::json!("UNRESOLVED");
    assert!(serde_json::from_value::<GraphSnapshot>(json).is_err());
    let missing = graph
        .nodes()
        .iter()
        .filter(|n| n.id.path().as_str() != "tests/a.test.ts")
        .cloned()
        .collect();
    assert!(GraphSnapshot::new(missing, graph.edges().to_vec(), vec![]).is_err());
    let mut wrong = graph.nodes().to_vec();
    wrong[0].kind = NodeKind::Method;
    assert!(GraphSnapshot::new(wrong, graph.edges().to_vec(), vec![]).is_err());
    let mut edges = graph.edges().to_vec();
    edges
        .iter_mut()
        .find(|e| e.kind == EdgeKind::Calls)
        .unwrap()
        .provenance
        .file = path("elsewhere.ts");
    assert!(GraphSnapshot::new(graph.nodes().to_vec(), edges, vec![]).is_err());
}

#[test]
fn evidence_partition_tags_are_checked_and_output_controls_are_explicit() {
    let (_, seed, ta, _, ca, _, _, _) = example();
    let w = witness(&seed, &ta, EvidenceKind::SymbolBacked, vec![ca]);
    let bad = serde_json::json!({"symbol_backed": [], "module_fallback": [w]});
    assert!(serde_json::from_value::<EvidenceSets>(bad).is_err());
    assert_eq!(
        QueryLimits::default(),
        QueryLimits {
            depth: None,
            display_limit: 100
        }
    );
    assert_eq!(Truncation::default().display_omitted, 0);
}
