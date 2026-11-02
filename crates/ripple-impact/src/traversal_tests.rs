#[path = "../../../fixtures/impact/support.rs"]
mod support;
use crate::{graph::DependencyGraph, traversal::*};
use ripple_core::{graph::*, results::*};
use std::collections::{BTreeMap, BTreeSet};
use support::*;
fn unlimited() -> QueryLimits {
    QueryLimits {
        depth: None,
        display_limit: usize::MAX,
    }
}
fn query(graph: &GraphSnapshot, seeds: &[NodeId]) -> TraversalResult {
    let result = DependencyGraph::new(graph)
        .query(seeds, RevisionSide::Worktree, unlimited())
        .unwrap();
    for w in result
        .impact
        .evidence
        .symbol_backed()
        .iter()
        .chain(result.impact.evidence.module_fallback())
    {
        w.validate(graph).unwrap();
    }
    result
}
fn semantic<'a>(result: &'a TraversalResult, seed: &NodeId, target: &NodeId) -> &'a Witness {
    result
        .impact
        .evidence
        .symbol_backed()
        .iter()
        .find(|w| &w.seed == seed && &w.target == target)
        .unwrap()
}
#[test]
fn adjacency_preserves_original_directions_and_parallel_provenance() {
    let mut f = Fixture::default();
    let a = f.symbol("a");
    let b = f.symbol("b");
    f.edge(&b, &a, EdgeKind::Calls, Confidence::Resolved);
    f.edge(&b, &a, EdgeKind::References, Confidence::Candidate);
    let graph = f.finish();
    let indexed = DependencyGraph::new(&graph);
    assert_eq!(indexed.outgoing(&b).count(), 2);
    assert_eq!(indexed.incoming(&a).filter(|e| e.source == b).count(), 2);
    assert!(indexed.incoming(&b).all(|e| e.kind == EdgeKind::Defines));
    assert_eq!(indexed.incoming(&file_id("missing.ts")).count(), 0);
    assert!(indexed.outgoing(&b).is_sorted());
    assert_eq!(indexed.snapshot(), &graph);
}
#[test]
fn chains_use_reverse_edges_and_retain_candidate_provenance() {
    let mut f = Fixture::default();
    let a = f.symbol("a");
    let b = f.symbol("b");
    let c = f.symbol("c");
    let unrelated = f.symbol("unrelated");
    let test = f.file("tests/chain.test.ts", true);
    f.edge(&b, &a, EdgeKind::Calls, Confidence::Resolved);
    f.edge(&c, &b, EdgeKind::References, Confidence::Candidate);
    f.edge(&test, &c, EdgeKind::Calls, Confidence::Resolved);
    f.edge(&a, &unrelated, EdgeKind::Calls, Confidence::Resolved);
    let graph = f.finish();
    let r = query(&graph, std::slice::from_ref(&a));
    assert_eq!(
        r.impact.evidence.symbol_backed_nodes(),
        BTreeSet::from([a.clone(), b.clone(), c.clone(), test.clone()])
    );
    let w = semantic(&r, &a, &test);
    assert_eq!(w.distance(), 3);
    assert_eq!(w.confidence(), Confidence::Candidate);
    assert_eq!(
        w.hops.iter().map(|e| &e.source).collect::<Vec<_>>(),
        [&b, &c, &test]
    );
    assert_eq!(r.tests.symbol_backed, BTreeSet::from([test]));
    assert_eq!(r.tests.total_test_files, 1);
}
#[test]
fn shortest_distance_precedes_confidence_and_resolved_wins_equal_length() {
    let mut f = Fixture::default();
    let root = f.symbol("root");
    let a = f.symbol("a");
    let b = f.symbol("b");
    let end = f.symbol("end");
    let direct = f.symbol("direct");
    f.edge(&a, &root, EdgeKind::Calls, Confidence::Candidate);
    f.edge(&b, &root, EdgeKind::Calls, Confidence::Resolved);
    f.edge(&end, &a, EdgeKind::Calls, Confidence::Resolved);
    f.edge(&end, &b, EdgeKind::Calls, Confidence::Resolved);
    f.edge(&direct, &root, EdgeKind::References, Confidence::Candidate);
    f.edge(&direct, &end, EdgeKind::Calls, Confidence::Resolved);
    let graph = f.finish();
    let r = query(&graph, std::slice::from_ref(&root));
    let w = semantic(&r, &root, &end);
    assert_eq!(w.hops[0].source, b);
    assert_eq!(w.confidence(), Confidence::Resolved);
    assert_eq!(semantic(&r, &root, &direct).distance(), 1);
    assert_eq!(
        semantic(&r, &root, &direct).confidence(),
        Confidence::Candidate
    );
    let depth1 = r
        .rows
        .iter()
        .filter(|row| row.distance == 1)
        .collect::<Vec<_>>();
    assert!(depth1.is_sorted_by_key(|row| (row.confidence, &row.target)));
}
#[test]
fn later_candidate_hop_can_make_the_weaker_prefix_the_lexical_winner() {
    let mut f = Fixture::default();
    let root = f.symbol("root");
    let a = f.symbol("a");
    let b = f.symbol("b");
    let join = f.symbol("join");
    let leaf = f.symbol("leaf");
    f.edge(&a, &root, EdgeKind::Calls, Confidence::Candidate);
    f.edge(&b, &root, EdgeKind::Calls, Confidence::Resolved);
    f.edge(&join, &a, EdgeKind::Calls, Confidence::Resolved);
    f.edge(&join, &b, EdgeKind::Calls, Confidence::Resolved);
    f.edge(&leaf, &join, EdgeKind::Calls, Confidence::Candidate);
    let graph = f.finish();
    let r = query(&graph, std::slice::from_ref(&root));
    assert_eq!(semantic(&r, &root, &join).hops[0].source, b);
    assert_eq!(semantic(&r, &root, &leaf).hops[0].source, a);
    assert_eq!(
        semantic(&r, &root, &leaf).confidence(),
        Confidence::Candidate
    );
}
#[test]
fn lexical_ties_compare_the_whole_path_instead_of_last_parent_identity() {
    let mut f = Fixture::default();
    let s = f.symbol("s");
    let a = f.symbol("a");
    let b = f.symbol("b");
    let y = f.symbol("y");
    let z = f.symbol("z");
    let end = f.symbol("end");
    for (source, target) in [
        (&a, &s),
        (&b, &s),
        (&z, &a),
        (&y, &b),
        (&end, &z),
        (&end, &y),
    ] {
        f.edge(source, target, EdgeKind::Calls, Confidence::Resolved);
    }
    let graph = f.finish();
    let r = query(&graph, std::slice::from_ref(&s));
    let w = semantic(&r, &s, &end);
    assert_eq!(
        w.hops.iter().map(|e| &e.source).collect::<Vec<_>>(),
        [&a, &z, &end]
    );
}
#[test]
fn cycles_and_self_calls_terminate_without_cyclic_witnesses_or_false_boundary() {
    let mut f = Fixture::default();
    let a = f.symbol("a");
    let b = f.symbol("b");
    for (s, t) in [(&a, &a), (&a, &b), (&b, &a)] {
        f.edge(s, t, EdgeKind::Calls, Confidence::Candidate);
    }
    let graph = f.finish();
    let r = query(&graph, std::slice::from_ref(&a));
    assert_eq!(semantic(&r, &a, &a).distance(), 0);
    assert_eq!(semantic(&r, &a, &b).distance(), 1);
    let limited = DependencyGraph::new(&graph)
        .query(
            &[a],
            RevisionSide::Head,
            QueryLimits {
                depth: Some(1),
                display_limit: 100,
            },
        )
        .unwrap();
    // The semantic cycle is exhausted; the fallback file has no incoming imports.
    assert!(limited.impact.truncation.depth_boundary_nodes.is_empty());
}
#[test]
fn multiple_seeds_and_revisions_survive_and_input_order_does_not_matter() {
    let mut f = Fixture::default();
    let a = f.symbol("a");
    let b = f.symbol("b");
    let end = f.symbol("end");
    f.edge(&end, &a, EdgeKind::Calls, Confidence::Resolved);
    f.edge(&end, &b, EdgeKind::Calls, Confidence::Candidate);
    let graph = f.finish();
    let r = query(&graph, &[b.clone(), a.clone(), a.clone()]);
    assert_eq!(r, query(&graph, &[a.clone(), b.clone()]));
    assert_eq!(
        r.impact
            .evidence
            .symbol_backed()
            .iter()
            .filter(|w| w.target == end)
            .count(),
        2
    );
    let reversed = GraphSnapshot::new(
        graph.nodes().iter().rev().cloned().collect(),
        graph.edges().iter().rev().cloned().collect(),
        vec![],
    )
    .unwrap();
    assert_eq!(r, query(&reversed, &[a.clone(), b.clone()]));
    let index = DependencyGraph::new(&graph);
    let base = index
        .query(&[a.clone(), b.clone()], RevisionSide::Base, unlimited())
        .unwrap();
    let head = index
        .query(&[a, b], RevisionSide::Head, unlimited())
        .unwrap();
    let both = EvidenceSets::from_witnesses(
        base.impact
            .evidence
            .symbol_backed()
            .iter()
            .chain(head.impact.evidence.symbol_backed())
            .cloned(),
    );
    assert_eq!(both.symbol_backed().len(), 8);
}
#[test]
fn unrelated_exports_keep_precise_test_paths_separate_from_fallback() {
    let mut f = Fixture::default();
    let a = f.symbol_in("src/lib.ts", "a");
    let b = f.symbol_in("src/lib.ts", "b");
    let lib = a.defining_file();
    let ta = f.file("tests/a.test.ts", true);
    let tb = f.file("tests/b.test.ts", true);
    for t in [&ta, &tb] {
        f.edge(t, &lib, EdgeKind::Imports, Confidence::Resolved);
    }
    f.edge(&ta, &a, EdgeKind::Calls, Confidence::Resolved);
    f.edge(&tb, &b, EdgeKind::Calls, Confidence::Resolved);
    let graph = f.finish();
    let r = query(&graph, std::slice::from_ref(&a));
    assert_eq!(r.tests.symbol_backed, BTreeSet::from([ta.clone()]));
    assert_eq!(
        r.tests.module_fallback,
        BTreeSet::from([ta.clone(), tb.clone()])
    );
    assert_eq!(r.tests.fallback_only, BTreeSet::from([tb.clone()]));
    assert_eq!(r.tests.combined, BTreeSet::from([ta, tb]));
    assert!(!r.impact.evidence.symbol_backed_nodes().contains(&b));
    assert!(
        r.impact
            .evidence
            .module_fallback()
            .iter()
            .all(|w| w.confidence() == Confidence::Resolved)
    );
    let file_change = query(&graph, &[lib]);
    assert!(file_change.impact.evidence.symbol_backed().is_empty());
    assert_eq!(file_change.tests.combined, r.tests.combined);
}
#[test]
fn fallback_reexports_and_cycles_remain_file_level_with_weakest_confidence() {
    let mut f = Fixture::default();
    let a = f.file("src/a.ts", false);
    let b = f.file("src/b.ts", false);
    let test = f.file("tests/a.test.ts", true);
    f.edge(&b, &a, EdgeKind::Reexports, Confidence::Candidate);
    f.edge(&a, &b, EdgeKind::Imports, Confidence::Resolved);
    f.edge(&test, &b, EdgeKind::Imports, Confidence::Resolved);
    let graph = f.finish();
    let r = query(&graph, std::slice::from_ref(&a));
    let w = r
        .impact
        .evidence
        .module_fallback()
        .iter()
        .find(|w| w.target == test)
        .unwrap();
    assert_eq!(w.distance(), 2);
    assert_eq!(w.confidence(), Confidence::Candidate);
    assert_eq!(r.tests.fallback_only, BTreeSet::from([test]));
}
#[test]
fn depth_and_display_limits_are_independent_and_zero_depth_is_explicit() {
    let mut f = Fixture::default();
    let a = f.symbol("a");
    let b = f.symbol("b");
    let test = f.file("tests/limit.test.ts", true);
    f.edge(&b, &a, EdgeKind::Calls, Confidence::Resolved);
    f.edge(&test, &b, EdgeKind::Calls, Confidence::Resolved);
    let graph = f.finish();
    let full = query(&graph, std::slice::from_ref(&a));
    let index = DependencyGraph::new(&graph);
    for display_limit in [0, 1, 2, 100] {
        let r = index
            .query(
                std::slice::from_ref(&a),
                RevisionSide::Worktree,
                QueryLimits {
                    depth: None,
                    display_limit,
                },
            )
            .unwrap();
        assert_eq!(r.impact.evidence, full.impact.evidence);
        assert_eq!(r.tests, full.tests);
        assert_eq!(r.rows, full.rows);
        assert_eq!(r.displayed().len(), full.rows.len().min(display_limit));
        assert_eq!(
            r.impact.truncation.display_omitted,
            full.rows.len().saturating_sub(display_limit)
        );
    }
    let zero = index
        .query(
            std::slice::from_ref(&a),
            RevisionSide::Worktree,
            QueryLimits {
                depth: Some(0),
                display_limit: 0,
            },
        )
        .unwrap();
    assert_eq!(zero.rows.len(), 1);
    assert_eq!(
        zero.impact.truncation.depth_boundary_nodes.as_slice(),
        std::slice::from_ref(&a)
    );
    assert!(zero.tests.combined.is_empty());
    let one = index
        .query(
            std::slice::from_ref(&a),
            RevisionSide::Worktree,
            QueryLimits {
                depth: Some(1),
                display_limit: 100,
            },
        )
        .unwrap();
    assert_eq!(one.impact.truncation.depth_boundary_nodes, [b]);
    assert_eq!(one.impact.evidence.module_fallback()[0].distance(), 1);
    let huge = index
        .query(
            &[a],
            RevisionSide::Worktree,
            QueryLimits {
                depth: Some(usize::MAX),
                display_limit: usize::MAX,
            },
        )
        .unwrap();
    assert_eq!(full.impact.evidence, huge.impact.evidence);
    assert_eq!(full.rows, huge.rows);
    assert_eq!(full.tests, huge.tests);
    assert!(huge.impact.truncation.depth_boundary_nodes.is_empty());
}
#[test]
fn hubs_report_unique_direct_dependents_without_pruning_tests() {
    let mut f = Fixture::default();
    let root = f.symbol("root");
    let mut tests = BTreeSet::new();
    for i in 0..HUB_THRESHOLD {
        let t = f.file(&format!("tests/{i}.test.ts"), true);
        f.edge(&t, &root, EdgeKind::Calls, Confidence::Resolved);
        f.edge(&t, &root, EdgeKind::References, Confidence::Resolved);
        tests.insert(t);
    }
    let graph = f.finish();
    let r = DependencyGraph::new(&graph)
        .query(
            std::slice::from_ref(&root),
            RevisionSide::Worktree,
            QueryLimits {
                depth: None,
                display_limit: 1,
            },
        )
        .unwrap();
    assert_eq!(r.tests.combined, tests);
    assert_eq!(r.displayed().len(), 1);
    assert_eq!(
        r.summaries[0].hubs,
        [Hub {
            node: root,
            direct_dependents: HUB_THRESHOLD
        }]
    );
    assert_eq!(
        r.summaries[0].nodes_by_depth,
        BTreeMap::from([(0, 1), (1, HUB_THRESHOLD)])
    );
    assert_eq!(r.summaries[0].visited_nodes, HUB_THRESHOLD + 1);
}
#[test]
fn missing_opaque_and_empty_seeds_and_graph_wide_unknowns_are_explicit() {
    let mut f = Fixture::default();
    let a = f.symbol("a");
    let unrelated = f.symbol("unrelated");
    let boundary = f.unknown(&unrelated);
    let graph = f.finish();
    let index = DependencyGraph::new(&graph);
    assert!(
        index
            .query(
                &[a.clone(), file_id("missing.ts")],
                RevisionSide::Worktree,
                unlimited()
            )
            .is_err()
    );
    assert!(
        index
            .query(
                std::slice::from_ref(&boundary),
                RevisionSide::Worktree,
                unlimited()
            )
            .is_err()
    );
    let r = query(&graph, &[a]);
    assert_eq!(r.impact.unresolved, graph.unresolved());
    assert!(!r.impact.evidence.all_nodes().contains(&boundary));
    assert!(!r.impact.evidence.all_nodes().contains(&unrelated));
    let empty = query(&graph, &[]);
    assert!(empty.rows.is_empty());
    assert!(empty.tests.combined.is_empty());
    assert_eq!(empty.impact.unresolved, graph.unresolved());
    let empty_graph = GraphSnapshot::new(vec![], vec![], vec![]).unwrap();
    assert!(query(&empty_graph, &[]).rows.is_empty());
}
#[test]
fn a_changed_test_file_itself_is_in_the_conservative_selection() {
    let mut f = Fixture::default();
    let t = f.file("tests/self.test.ts", true);
    let graph = f.finish();
    let r = query(&graph, std::slice::from_ref(&t));
    assert_eq!(r.tests.fallback_only, BTreeSet::from([t]));
    assert_eq!(r.impact.evidence.module_fallback()[0].distance(), 0);
}
// Independent exhaustive simple-path enumeration on small graphs, rather than a second BFS.
fn enumerate(
    graph: &GraphSnapshot,
    current: &NodeId,
    seen: &mut BTreeSet<NodeId>,
    path: &mut Vec<Edge>,
    best: &mut BTreeMap<NodeId, Vec<Edge>>,
) {
    let key = |p: &Vec<Edge>| {
        (
            p.len(),
            p.iter()
                .map(|e| e.confidence)
                .max()
                .unwrap_or(Confidence::Resolved),
            p.clone(),
        )
    };
    if best.get(current).is_none_or(|old| key(path) < key(old)) {
        best.insert(current.clone(), path.clone());
    }
    for edge in graph.edges().iter().filter(|e| {
        &e.target == current && matches!(e.kind, EdgeKind::Calls | EdgeKind::References)
    }) {
        if seen.insert(edge.source.clone()) {
            path.push(edge.clone());
            enumerate(graph, &edge.source, seen, path, best);
            path.pop();
            seen.remove(&edge.source);
        }
    }
}
#[test]
fn bounded_generated_graphs_match_an_exhaustive_shortest_path_oracle() {
    let mut state = 0x81e293ab_u64;
    for case in 0..256 {
        let mut f = Fixture::default();
        let ids = (0..5)
            .map(|i| f.symbol(&format!("n{i}")))
            .collect::<Vec<_>>();
        for source in &ids {
            for target in &ids {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                if state >> 62 != 0 {
                    continue;
                }
                f.edge(
                    source,
                    target,
                    if state & 32 == 0 {
                        EdgeKind::Calls
                    } else {
                        EdgeKind::References
                    },
                    if state & 64 == 0 {
                        Confidence::Resolved
                    } else {
                        Confidence::Candidate
                    },
                );
            }
        }
        let graph = f.finish();
        let result = query(&graph, &ids);
        for seed in &ids {
            let mut best = BTreeMap::new();
            enumerate(
                &graph,
                seed,
                &mut BTreeSet::from([seed.clone()]),
                &mut Vec::new(),
                &mut best,
            );
            let actual = result
                .impact
                .evidence
                .symbol_backed()
                .iter()
                .filter(|w| &w.seed == seed)
                .map(|w| (w.target.clone(), w.hops.clone()))
                .collect::<BTreeMap<_, _>>();
            assert_eq!(actual, best, "generated case {case}, seed {seed:?}");
        }
    }
}
