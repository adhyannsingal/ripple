# Verified workflow measurements

Reproduce release builds and run from the repository root:

```sh
cargo build --release --locked -p ripple-index --example workflow_benchmark
cargo build --release --locked -p ripple-cli
python3 bench/scripts/workflows.py --binary target/release/ripple --helper target/release/examples/workflow_benchmark --output /tmp/ripple-workflows-new --samples 20
```

[Raw samples and metadata](workflows.json) pin workload/source/binary/helper hashes, compiler, CPU/RAM/OS, counts, RSS and index size. 20 retained timing samples, one discarded warm-up, warm OS cache, fresh Git/index roots per sample. Fixed ordering/background load are uncontrolled. Percentiles are nearest rank; p99 of 20 samples is the maximum, not a stable tail estimate. The script currently records BSD-time RSS on macOS; other-host RSS is unmeasured. No CI latency assertion.

| Observation | p50 ms | p95 ms | p99 ms |
|---|---:|---:|---:|
| cold | 422.862 | 430.317 | 430.514 |
| no_op | 417.307 | 427.798 | 427.934 |
| body | 417.731 | 433.617 | 438.711 |
| verified_impact | 363.667 | 373.530 | 381.580 |
| startup_version | 5.773 | 6.594 | 10.038 |
| adjacency_ms | 0.090 | 0.107 | 0.108 |
| capture_verification_ms | 300.458 | 308.376 | 308.972 |
| module_policy_ms | 1.281 | 1.552 | 1.553 |
| production_verified_load_ms | 305.276 | 319.306 | 338.016 |
| pure_traversal_ms | 1.587 | 2.037 | 2.082 |
| storage_load_ms | 3.324 | 3.616 | 3.655 |

Workload: 65 supported one-line TS files (65 LOC), deterministic call chain/test; 153.71 files/s and 153.71 LOC/s from CLI cold p50. These synthetic LOC numbers are not representative project throughput. Maximum observed impact RSS: 77955072 bytes (21 observations including warm-up). Three-generation index bytes: 1813819–1813819, including SQLite/sidecars. No long-term archive/GC measurement.

CLI observations include startup, Git provenance, verification and rendering. `--version` is a startup proxy and is not subtracted. The phase helper compiles production capture source verbatim and compares query equality with production load_current; it measures storage, capture/verification, policy resolution, adjacency and pure traversal separately. Stage observations are not assumed perfectly additive. Cached storage/traversal is not a verified-worktree query. Initial harness alias-root rejection was repaired before successful evidence; no product fix was needed.

## Profile decision: reject metadata-only freshness shortcut

Capture/verification (~300 ms p50) dominates verified loading (~305 ms), rather than traversal (~1.6 ms). Replacing verified capture with a size/mtime cache could avoid meaningful work, but the controlled counterexample changes a function body without changing size or mtime: metadata stays equal and production status correctly rejects stale state. Such a shortcut violates captured-content freshness and cannot ship. No before/after speedup is claimed because the unsafe candidate was rejected before performance comparison; production algorithms remain unchanged. This is the card's permitted evidence-based rejected optimization outcome.

Larger/real production workloads, cold kernel caches, CPU stacks, peak cold-index RSS, long archive growth and concurrent load remain unmeasured. M6's separate external/synthetic incremental library samples exclude real Git provenance and must not be confused with these CLI numbers. Final candidate evidence is rerun in M8-T04.
