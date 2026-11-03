# Change classification goldens

`cases.json` supplies **25 independently authored expected outcomes**. `ripple-impact/src/changes_tests.rs` constructs disposable Git commits, obtains real M4-T01 hunks, reuses discovery/extraction and checks pure classification. Corpus scripts never execute.

Coverage: body/signature/both deltas; ordinary comments versus ASI-sensitive comments; symbol/file rename as delete+add; added/deleted declarations; import-only and top-level changes; class headers/member add/delete/field shape; method-body-only edits without parent-class duplication (multiline and same-line); multiple declarations on a line; EOF/no-newline and Unicode/CRLF; missing fingerprints, recovered opposite endpoints, inherited config paths and lockfile invalidation. Original file rows always survive even with no supported symbol.

Separate synthetic tests cover invalid IDs/analyzer tokens/source inventories, hash version mismatch, absent hashes without fabricated changed evidence, ambiguous ownership, zero-width/out-of-range hunks and equal endpoint IDs. Multisource cases reverse input fact order and require identical output. Each symbol result retains base/head facts; coarse hunk owners are not themselves change seeds.

Run `cargo test --locked -p ripple-impact changes_tests`. Performance is not measured. See [ADR 0022](../../docs/adr/0022-change-classification.md) for boundary/uncertainty policy and conservative file-level fallback.
