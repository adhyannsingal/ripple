# Foundation review — 2026-10-29

Scope: the implemented M0 foundation, reviewed against `ripple_buildspec.md`, the implementation plan and execution protocol. Baseline: `8d0a4f3478f7bebcb0990a8f204697777253cfea`. This is a source review with regression testing, not proof of absence of defects.

## Findings and resolutions

| Priority | Finding | Resolution and evidence |
|---|---|---|
| Medium | RevisionChanges accepted overlapping/crossing hunks and populated nonexistent file sides. Derived ordering also placed an empty anchor before a replacement of the same numbered line. | Validate absent sides, sort by normalized Git intervals and reject overlap/crossing. New constructor/JSON regressions cover malformed hunks and valid adjacent replacement/insertion ordering. The overlap regression failed before the fix. ADR 0005 specifies the semantics. |
| Medium | Directory ownership could evade conflicts when an existing directory lacked its required trailing slash; repeated trailing slashes also produced ambiguous prefixes. | Require directory suffixes, reject noncanonical separators and retain cross-task subtree conflict tests. Existing-directory regression failed before the fix. |
| Low | A task's own directory/file claims conflicted; control characters were accepted in claimed paths. | Compare overlap across different tasks and reject Unicode control characters. Both new regression tests failed before the fix; all planning tests now pass. |
| Low | Inherited GIT_DEFAULT_HASH=sha256 changed disposable Git fixtures to 64-character IDs and failed their deterministic SHA-1 expectation. | Remove that variable from fixture Git commands. The hostile-environment test failed before the fix (64 versus 40); a child-process regression now preserves the evidence without mutating the test process environment. |
| Low | Fixture paths admitted normalized aliases, while catalog assertions assumed exactly three entries and positional lookup. | Reject dot/empty/trailing components and locate required cases by ID, allowing future cases. Alias regression failed before the fix. |

## Coverage

Reviewed the Rust source in all seven crates: CLI bootstrap; identity, source/span, syntax, graph, witness/result, input/index, revision and canonical-state contracts; parser and SQLite smoke tests; intentionally staged Git/resolver/impact modules. Reviewed all Rust integration tests, shared fixture loader and Git/process helpers, fixture manifest and expectations, Python ledger validator/tests, Cargo manifests/toolchain/lockfile setup, CI, README, instructions, execution protocol, task cards and ADR contracts.

Checked crate dependency direction, validated-deserialization boundaries, deterministic ordering, symbol identity, source coordinates, graph endpoints/provenance, confidence and fallback separation, revision-labelled evidence, canonical equality versus runtime validation, CLI exit behavior, test-process isolation and planning ownership. No additional actionable defect was established in those areas during this pass. Product naming is consistent with Ripple.

## Verification

On macOS with pinned Rust 1.99.1: `cargo fmt --all -- --check`, `cargo build --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `python3 -m unittest discover -s scripts/tests`, `python3 scripts/plan.py validate`, and CLI help/version passed. Final suite: 48 Rust tests and 18 Python tests. The entire infrastructure suite also passed with `GIT_DEFAULT_HASH=sha256`. [Integrated platform CI](https://github.com/adhyannsingal/ripple/actions/runs/40994011389) passed on Linux, macOS and Windows; evidence is recorded in the task ledger.

## Limits and handoff

Ripple remains a foundation/CLI bootstrap. Extraction, resolution, production Git reads, persistent indexing, traversal and incremental replay are later tasks; existing fixtures do not establish end-to-end analyzer correctness. Digests are shape-checked contract values, not recomputed source hashes. Actual captured-input verification and atomic publication remain implementation requirements. Fixture helpers are controlled test utilities, not a sandbox for arbitrary symlink trees or detached process descendants.

No benchmark was run and performance is not measured. No external dependency advisory scan or formal security audit was performed. Review hot-path costs (including source-position lookup and nested scope validation) when real extraction and benchmark harnesses exist. Next implementation task remains M1-T01; this review introduces no new product features or dependencies.
