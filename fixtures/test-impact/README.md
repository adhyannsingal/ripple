# Revision-aware test selection fixtures

`cases.json` is an independently expected finite set of real Git before/after projects. The index integration test builds immutable endpoint snapshots and validates every selected witness against its own revision graph.

Cases cover helper/caller/test chains, two unrelated exports (one symbol-backed test and one fallback-only addition), imported calls inside test callbacks, side-effect imports, deleted tests, ignored/present-unindexed old tests, cyclic barrels, ambiguous star-export boundaries, dynamic boundaries, zero display limit and explicit zero depth. No fetched scripts or user worktree are used.

```sh
cargo test --locked -p ripple-index --test test_impact -- --nocapture
```

These test potentially related reachability and evidence preservation, not runtime test discovery or sufficiency. Candidate and unknown boundaries remain uncertain. Display-zero must preserve full selection; depth-zero must disclose truncation.

The candidate-contract case explicitly changes a validated call edge to Candidate on both snapshot graphs to verify confidence preservation. It is a graph-contract adversary, not evidence that the current resolver emits candidate edges for ambiguous star exports (those remain unresolved).
