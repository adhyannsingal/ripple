# ADR 0017: Deterministic dual-track reverse queries

Status: accepted and implemented in M2-T04.

## Decision

`ripple-impact` borrows a validated `GraphSnapshot` and builds forward/reverse adjacency using semantic edge order. No SQL, resolver, filesystem or Git access occurs during a query. Callers bind the snapshot to revision provenance/freshness; a supplied base/head/worktree label alone does not verify that binding.

Each distinct seed runs independently. Symbol seeds run a reverse CALLS/REFERENCES walk and a separate fallback walk through their actual initial DEFINES edge followed by reverse IMPORTS/REEXPORTS. File seeds run only fallback. Missing and opaque seeds fail explicitly before traversal. No containment hop enters the symbol-backed track, no semantic hop enters fallback and no unresolved target is invented.

Choose one witness per seed, track, revision and reached target by shortest distance, strongest weakest-hop confidence, then lexicographic full reverse-walk edge sequence under core `Edge` ordering. Breadth-first layers ensure shortest distance and terminate cycles. Retain up to two confidence states at each shortest-distance node, ranking equal-length path prefixes within each layer. A later candidate hop can erase a prefix's confidence advantage; discarding the weaker lexical prefix too early would produce the wrong deterministic witness. Predecessors are finalized before the next layer and full original edges are reconstructed only for returned paths.

Retain zero-hop symbol/file seed evidence. This includes an explicitly changed test file itself in conservative test selection. A symbol-to-file fallback bridge counts as one hop. Results retain both evidence tracks and every seed/revision. Distinct display rows are ordered by their best witness distance, confidence, then core `NodeId` ordering; merging display rows never merges evidence paths.

Depth limits restrict both walks, counting the bridge. Sorted boundary nodes mark visited nodes with eligible incoming dependents not reached within that track's depth budget. Edges to already reached nodes, including cycles, do not imply truncation. Display limits only slice ordered rows; complete evidence, test sets and summaries remain available. Callers must surface depth truncation when using bounded results.

Test partitions filter reached file nodes by the snapshot's TEST_FILE classification. Expose symbol-backed, module-fallback, fallback-only and combined sets plus the total classified test-file count. These are potentially related files, never sufficient test sets. Revision-aware active/deleted-file selection remains M7 work.

Retain all graph-wide unresolved records, including those on apparently unrelated owners: their unknown targets cannot establish relevance or irrelevance. They are uncertainty context, not reachable witnesses. Hub summaries use a reporting threshold of 50 distinct eligible direct dependents, exclude self loops and collapse parallel edges. Hubs never prune traversal. Per-track depth counts include the start and bridge, and therefore are not disjoint totals across seeds/tracks.

## Evidence and limits

Hand-authored fixtures cover chains, diamonds, lexical/confidence ties, cycles, imports/re-exports, exact module-level test calls, unrelated exports, hubs and limits. An independent exhaustive simple-path oracle checks all seeds of 256 deterministic five-symbol graphs. Each produced witness is validated against its graph in the acceptance suite.

No performance measurement or resource quota is claimed. Adjacency is reused across queries and traversal uses iterative layers/predecessors, but complete paths and multiple seeds can still produce large output. The library never silently drops evidence to meet a memory budget. Production CLI composition and end-to-end indexing remain M2-T05.
