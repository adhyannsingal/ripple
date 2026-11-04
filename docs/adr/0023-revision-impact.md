# ADR 0023 — Immutable endpoint snapshots and revision impact

Status: accepted

M4-T03 needs old dependency paths as well as current ones. Reusing the worktree index would lose deleted symbols and relationships and could overwrite the user's current generation.

`RevisionSource` binds an immutable `SourceTree` to a pinned commit. The Git tree implements this contract without resolving mutable refs again. `snapshots::build` shares the cold builder's capture, effective configuration, extraction and graph analysis, then constructs a validated in-memory snapshot. It never opens the store, checks out files or publishes sidecars. Captured input hashes, module artifacts and diagnostics remain available. Historical captures retain existing scanner/capture limits and the Git provider's tighter blob/read budgets; no total-process resource or latency guarantee follows.

`revision_impact` consumes endpoint views and the pinned pair, classifies changes, queries actual symbol seeds on the applicable graphs, and merges semantic nodes while retaining every witness's base/head label and symbol-backed/module-fallback track. Views must have the matching commit origin and compatible project/analyzer versions; captured inputs must appear in supplied physical presence. Every combined witness validates against its endpoint graph. Unresolved records and depth boundaries remain separate for each endpoint. Unresolved records are graph-wide context, not demonstrated reachability.

File-only changes seed file fallback. Global input changes conservatively select every indexed file through explicit zero-hop module-fallback witnesses and global-input seed reasons; these are policy relevance, not invented config dependency edges. Unsupported changes retain unindexed-change records and incompleteness. Display limits only slice presentation, preserving complete evidence and test selection.

Physical path presence is captured separately from indexed source discovery, including ignored and nonregular leaf paths. A selected old test is classified as active/indexed in head, physically absent in head, or present but unindexed in head. The head test denominator explicitly counts indexed test files. No test set is claimed sufficient. The metadata traversal has a 100000-entry and 128-depth bound.

Eleven real-Git fixtures cover removed/deleted relationships, additions, config inheritance, unknown boundaries and test presence. Tests establish repeated-build canonical equality, parity with a committed worktree cold index, and byte-for-byte preservation of existing index/Git state despite staged, dirty and untracked files. There is no historical persistence/cache or CLI diff integration in this task; M4-T04 composes these APIs into the CLI. Performance is not measured.
