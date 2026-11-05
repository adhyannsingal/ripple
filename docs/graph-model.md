# Graph and evidence contracts

M0-T04 implements validated graph/result records in `ripple-core`. M2-T04 implements forward/reverse adjacency and impact BFS in `ripple-impact`; resolution and CLI composition remain separate. Queries consume a validated snapshot without SQL at each hop.

## Nodes, edges and provenance

`NodeId` is a repository-local file identity, structured `SymbolId`, or opaque reference identity. File/test classification is metadata on one file node, never two competing identities. Functions/classes/methods use their corresponding symbol kind. Opaque nodes carry a deterministic reference-occurrence key and span; unlike symbol IDs, boundary occurrence keys need not survive source-layout edits. Every non-file node has a defining file. Every symbol has exactly one resolved `DEFINES` edge from that file. Every opaque node has one unresolved record with a reason/provenance and no invented target edge.

| Edge | Direction | Allowed endpoints |
|---|---|---|
| DEFINES | owning file → declaration | FILE/TEST_FILE → symbol in that file |
| IMPORTS / REEXPORTS | consuming file → provider file | FILE/TEST_FILE → FILE/TEST_FILE |
| CALLS / REFERENCES | dependent → dependency | symbol or exact module-level reference owner → symbol |

Edges retain confidence, source-file/span provenance and resolution rule. Cross-project edges are outside initial support. Duplicate nodes/edges, missing endpoints, inconsistent kinds, missing definitions, orphan boundaries and provenance outside a source owner are invalid. Snapshot construction/deserialization validates and sorts records by semantic content. This checks internal consistency; it does not prove a resolver rule is semantically correct. Coordinate/source-byte verification also belongs at the source/index boundary.

## Confidence and evidence kind are independent

`RESOLVED` means a supported deterministic resolver rule identifies one target. `CANDIDATE` retains a justified plausible target without pretending certainty. `UNRESOLVED` belongs to explicit unresolved records/opaque boundaries; a targeted edge with that confidence is rejected. Structural definition edges are resolved. Confidence order is strongest to weakest, and a witness reports its weakest hop.

`symbol_backed` paths reverse only `CALLS`/`REFERENCES`, starting at a symbol. Module-level direct calls/references can therefore reach test-file nodes without expanding through an entire file. References inside top-level test callbacks remain owned by the file according to the source contract.

`module_fallback` paths start from a changed file, or first reverse a symbol's actual `DEFINES` edge to its file, then reverse `IMPORTS`/`REEXPORTS`. Results in this track are files. Definitions are not allowed midway through this track; semantic edges are not silently mixed into it. File/config/top-level changes without a concrete changed symbol seed this conservative track and cannot claim a symbol-backed path. Unsupported boundaries remain visible alongside either track; they are not traversed as invented call edges.

An all-resolved import path is still module fallback: certainty about the import is not certainty about a particular changed export's relevance. Both tracks must be retained. `EvidenceSets` preserves distinct seeds/revisions and both witness types, deterministically deduplicating only exact duplicate witnesses. The default potentially-related selection is the union; fallback-only results are the difference from symbol-backed results. Neither the union nor a narrower view guarantees test sufficiency.

Example: `src/lib.ts` exports unrelated `a` and `b`; `a.test.ts` imports/calls `a`, and `b.test.ts` imports/calls `b`. For an `a` change, symbol-backed test evidence reaches only `a.test.ts`, while module fallback reaches both. The existing unrelated-exports fixture independently specifies these sets.

## Witness, revision and output contracts

A `Witness` records a seed, target, revision side (worktree/base/head), evidence kind and original dependency edges in reverse-walk order. Validation checks membership in the chosen revision graph, continuity, endpoint identity, evidence policy and absence of repeated nodes. The graph itself may contain cycles; explanation paths are simple. The M2-T04 BFS chooses shortest witnesses with deterministic tie-breaking; core validation alone does not choose or prove shortestness. Immutable SHA/origin binding arrives in M0-T05.

Distance is hop count within a track. A fallback containment hop counts; it must not be presented as a semantic call distance. There can be an exact and fallback witness to the same result, or base/head witnesses to it; keep these rather than deduplicating just by display name.

Query defaults are unlimited depth and 100 displayed results. `QueryLimits` and `Truncation` explicitly separate depth boundaries from omitted display rows. Display limiting must not limit traversal or the complete test selection. `ImpactResult` is a transport contract: deserialize/partition evidence structurally, then validate each witness against its actual revision graph before rendering. The M2-T04 query engine populates depth/display completeness data; CLI presentation remains M2-T05.

## Acceptance evidence

`cargo test --locked -p ripple-core --test graph_contracts` exercises independently authored graph records: unrelated exports, direct file-owned calls, candidates, unresolved boundaries, graph cycles, disconnected/forged witnesses, provenance errors, serialization and evidence partitioning. Tests do not compare a real resolver or claim measured TypeScript accuracy. Actual extraction/resolution fixtures and oracle metrics belong to their later tasks.

## Query library (M2-T04)

```rust,ignore
let graph = ripple_impact::graph::DependencyGraph::new(snapshot);
let result = graph.query(&seeds, revision_side, QueryLimits::default())?;
let visible_rows = result.displayed();
let complete_evidence = &result.impact.evidence;
let potentially_related_tests = &result.tests.combined;
```

`DependencyGraph` borrows a core `GraphSnapshot`; forward/reverse iterators retain original edges in semantic order. `query` rejects absent/opaque seeds, deduplicates repeated seeds and preserves independent witnesses for distinct seeds and revision labels. Callers must establish snapshot freshness and revision provenance; the query does not read live inputs or Git. A base and head query can be combined using `EvidenceSets::from_witnesses` without erasing revision labels; automatic revision-aware selection is later work.

For each seed/track/target choose distance, then strongest weakest-hop confidence, then the full reverse-walk edge sequence under core ordering. Zero-hop seed witnesses are retained, so a changed test file includes itself. Display rows choose the best distance/confidence across witnesses and tie-break on `NodeId`; complete evidence remains separate even when one row summarizes both tracks.

`rows` and `impact.evidence` are complete within the depth budget. `displayed()` applies only `display_limit`; `display_omitted` counts hidden distinct rows. `depth_boundary_nodes` is the sorted union of per-track frontier nodes with eligible not-yet-reached dependents. Exhausted cycles do not create a false boundary. Truncated depth means the test selection can be incomplete. `unresolved` retains graph-wide unknowns and must not be displayed as reachable paths.

`tests` exposes symbol-backed, module-fallback, fallback-only and combined test-file sets, with counts available via their lengths and the full snapshot test-file denominator. `summaries` provides per-seed/track visited counts, depth distributions and hubs at 50 distinct direct dependents. Starts and the fallback bridge count in depth summaries; overlapping seeds/tracks must not be summed as distinct impact counts. Hubs never prune results. See [ADR 0017](adr/0017-reverse-traversal.md) for tie-breaking and scope.

Run `cargo test --locked -p ripple-impact`. The 14 tests include independent expected paths and a bounded exhaustive simple-path comparison on 256 generated five-symbol graphs, for all five seeds. They validate every witness against its revision graph; this is query-contract evidence, not measured TypeScript resolver accuracy. Performance is not measured; no traversal benchmark harness exists yet. Complete paths may require substantial memory on large/deep graphs or many seeds.

## Revision-aware test selection

`ripple_impact::tests::select` projects a validated `RevisionImpact` into `TestImpact` without filesystem, Git or resolver access. Each selected test retains every seed/revision witness independently in the symbol-backed and module-fallback tracks. Tests reached only through an import/re-export path are labelled fallback-only; a resolved import is not a symbol dependency. The default combined set is their conservative union.

Partitions are separate for supported indexed head tests, paths absent at head, and old test paths physically present but unindexed at head. Old dependency relationships can select a surviving head test even when that relationship was removed. Only indexed supported head tests enter the selection-ratio denominator; absent/unindexed paths remain evidence and never inflate that numerator. A missing or ignored head path is not silently represented as an executable head test.

Display limits never remove machine paths or witnesses. Explicit depth limits can omit related tests and produce depth-boundary incompleteness. Snapshot diagnostics, graph-wide unknown references, unindexed changes and present-unindexed old tests remain visible; graph-wide uncertainty is context, not fabricated reachability. A complete set within a requested traversal depth is never a claim that the selected tests are sufficient. See [test-selection decision](adr/0030-test-impact.md) and [fixtures](../fixtures/test-impact/README.md).
