# Independent Ripple fixtures

`manifest.json` is the sorted, versioned catalog of small TS/TSX projects. Each case keeps executable input under `project/` and manually reviewed assertions in sibling `expected.json`. These assertions specify selected extraction/resolution/impact behavior; they are not a claim that analyzer commands are already implemented or full snapshots of their future output.

Initial cases cover a named-import call, parameter-shadowed import, and unrelated exports with separate symbol-backed versus module-fallback test sets. TSX is included in the last case. No npm install or test runner is required to index these source fixtures; mutation/evaluation runner projects arrive in later tasks.

## Add or review a case

1. Add the smallest project files that isolate the behavior. Use UTF-8 and intentional newline conventions; annotate special byte/CRLF fixtures in the case notes.
2. Write expected symbols/edges/evidence by inspection. Include negative expectations for unsupported or falsely resolved behavior. Never generate expectations by copying the analyzer's actual output.
3. Add a unique manifest entry with portable project/expectation paths and a sorted complete source-file list. Version incompatible expectation changes explicitly.
4. Run `cargo test --locked -p ripple-cli --test infrastructure` and the affected subsystem fixtures once they exist. Review source and expectation changes together; there is no blind golden-update command.

Shared read-only loaders live in `support/mod.rs`. Integration tests can include them with a relative `#[path]` attribute; they require serde/serde_json as dev dependencies. Temporary Git/CLI helpers in `crates/ripple-cli/tests/support/` use argument arrays, timeouts, disposable directories, fixed fixture identities/dates and isolated Git configuration. They never checkout or stash the user's repository. Fixed authors here are test data, not project commit attribution.

Future crates should inherit needed dev dependencies centrally instead of introducing a production fixture-helper crate. Run full workspace checks after helper changes. The current process runner kills/reaps its direct child on timeout; it is intended for these small hook-free fixtures, not arbitrary commands spawning detached process trees.
