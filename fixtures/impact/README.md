# Reverse-query fixtures

`support.rs` builds small independently authored, core-validated graphs with semantic identities, original edge provenance, candidate confidence and explicit opaque boundaries. These are graph-contract fixtures with synthetic one-byte spans, not extraction/resolver output. `crates/ripple-impact/src/traversal_tests.rs` supplies independent expected paths and sets, including the same two-export/two-test scenario documented in `fixtures/unrelated-exports/`.

Run `cargo test --locked -p ripple-impact`. Hand-authored cases cover chains, diamonds, cycles/self calls, full-path/confidence ties, multiple seeds/revision labels, exact file-owned test calls, import/re-export fallback, changed test seeds, hubs and depth/display limits. Every accepted result's witness is checked against the source graph. Absent/opaque seeds fail before output and graph-wide unresolved records remain visible.

The deterministic generated test uses seed `0x81e293ab`, 256 graphs of five function symbols and all five seeds, with calls/references, resolved/candidate edges and cycles. An independent recursive enumeration of every simple semantic path computes the minimum `(distance, weakest confidence, full edge sequence)` for each target. This is bounded correctness evidence, not a performance benchmark or TypeScript semantic oracle.
