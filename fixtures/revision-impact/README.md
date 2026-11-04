# Revision impact fixtures

`cases.json` defines eleven small repositories with explicit base/head contents and expected semantic/test behavior. `crates/ripple-index/src/snapshots_tests.rs` commits both endpoints in temporary Git repositories, builds independent immutable snapshots and validates every witness against its actual endpoint.

Cases cover deleted exports, removed calls/imports, added symbols, deleted versus ignored tests, inherited aliases/config invalidation, identical content, unresolved boundaries, file-only fallback and unsupported config. Separate tests cover same-commit comparisons, depth/display limits, canonical cold parity, invalid endpoint views, symlinks and preservation of staged/dirty/untracked worktree and existing index state. Git uses deterministic LF fixture inputs across platforms.

Run `cargo test -p ripple-index snapshots_tests --locked`.
