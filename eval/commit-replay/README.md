# Pinned incremental history replay

Build the evaluation-only helper and run the manifest-driven harness from the repository root:

```sh
cargo build --locked -p ripple-index --example incremental_evaluate
python3 eval/commit-replay/run.py --helper target/debug/examples/incremental_evaluate --output /tmp/ripple-replay-new
```

On Windows use the `.exe` helper and `python`. Output must be a new directory. Python 3.9+ and network access to raw.githubusercontent.com are needed. The manifest pins five ancestor-ordered commits from the MIT-licensed jridgewell/sourcemap-codec repository: a finite **source projection**, not a complete repository or general compatibility benchmark. Each snapshot contains unmodified src TS plus original package.json, tsconfig.json and LICENSE, verified by size/SHA256. Tests, build config, assets and lockfiles are excluded. The original export-map/config diagnostics remain visible; incomplete static analysis is not erased. The transition moving VLQ into its own file exercises real addition. Three explicit curated add/config/delete edits supplement upstream history, and are never represented as upstream commits. No upstream scripts run; no user tree is checked out, indexed or modified.

The runner materializes immutable bytes in bounded, automatically removed temporary trees. One tree carries the incremental generations across all eight steps; each independent cold tree is new. The Rust helper compares full `CanonicalAnalysis` (snapshot plus normalized module/config/discovery evidence), verifies a second DB load, and fails on any mismatch. The digest is a compact summary of already compared data, not a substitute for equality. Evidence includes counters, global fallback reasons, diagnostics and timings. `Worktree { head: None, dirty: true }` is an explicit synthetic materialization origin; immutable upstream identities are recorded separately in the evidence, not falsely claimed as a checked-out HEAD.

The entire campaign runs twice with fresh storage and requires exact semantic/counter/diagnostic equality after excluding only timing fields. Output records analyzer/schema/hash versions, manifest/helper hashes and source commit. Build the helper from that source commit when reproducing evidence; do not reuse a stale helper. Timings include capture/reverification, parsing/resolution and atomic publication; canonical reload/comparison and process startup are excluded from the library measurements and included in helper-wall time. These are debug replay observations, not benchmark claims. M6-T03 supplies repeated release measurements.
