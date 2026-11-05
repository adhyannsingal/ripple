# M6 incremental release measurements

Source/helper/manifest hashes, raw samples, analyzer versions, hardware and exact command are in [incremental.json](incremental.json). Reproduce from the repository root:

```sh
cargo build --release --locked -p ripple-index --example incremental_evaluate
python3 bench/scripts/incremental.py --helper target/release/examples/incremental_evaluate --output /tmp/ripple-bench-new --samples 20
```

Windows uses the `.exe` helper and `python`. The pinned external workload needs network access to raw.githubusercontent.com. No corpus scripts run.

20 retained samples and one discarded warm-up for each of five cases on two workloads: **200 measured comparisons + 10 warm-ups, zero canonical mismatches**. Setup comparisons also verify equality. Every sample has a fresh application tree; updates begin with exactly one prior generation. Kernel/file caches are warm, not evicted; desktop background load is uncontrolled. Update is measured before cold in fixed order, which can bias caches. Release library timing includes capture/reverification/analysis/publication and excludes process startup, canonical reload/comparison and setup/network. Helper-wall timings include both builds, startup and comparison. No user Git subprocess cost is included. Percentiles use nearest rank, not confidence bounds. No CI latency assertion.

Hardware: {"platform": "macOS-26.5-arm64-arm-64bit-Mach-O", "machine": "arm64", "processor": "arm", "cpu": "Apple M4", "memory_bytes": 17179869184}. Toolchain: rustc 1.99.1 (58cf8f9c3 2026-10-13).

| Workload | Case | Update p50/p95 ms | Independent cold p50/p95 ms | Parsed/reused | Module/reference files resolved |
|---|---|---:|---:|---:|---:|
| sourcemap-projection-4 | cold | 50.02 / 51.27 | 46.11 / 47.15 | 4/0 | 4/4 |
| sourcemap-projection-4 | no_op | 46.91 / 48.39 | 48.48 / 52.54 | 0/4 | 0/4 |
| sourcemap-projection-4 | body | 49.48 / 52.95 | 48.95 / 51.80 | 1/3 | 4/4 |
| sourcemap-projection-4 | export | 49.76 / 51.76 | 48.95 / 52.91 | 1/3 | 4/4 |
| sourcemap-projection-4 | config | 53.35 / 59.90 | 49.66 / 52.34 | 4/0 | 4/4 |
| supported-chain-65 | cold | 30.77 / 33.18 | 28.09 / 30.37 | 65/0 | 65/65 |
| supported-chain-65 | no_op | 26.25 / 27.58 | 28.60 / 34.10 | 0/65 | 0/0 |
| supported-chain-65 | body | 27.04 / 28.11 | 28.35 / 29.75 | 1/64 | 0/1 |
| supported-chain-65 | export | 27.53 / 40.39 | 28.77 / 33.34 | 1/64 | 65/2 |
| supported-chain-65 | config | 28.04 / 30.32 | 27.88 / 30.36 | 65/0 | 65/65 |

The source projection is four TS files plus original config/package/license at the last upstream replay SHA. It retains incomplete analysis and 651 unresolved references (see replay evidence). The deterministic supported chain has 64 functions in a call chain plus one test file; no unknowns at baseline/body/no-op. Export renaming deliberately creates a missing binding. It is a synthetic small workload, not a representative large repository.

No-op parses zero files; body updates parse one and reuse 3/64 files respectively. The supported body case skips module resolution and resolves one reference file. Config changes deliberately fall back to parsing all sources. These counters verify reuse; they do not imply large latency gains. Small-workload p50 improvements are modest or absent, and some update tails are slower than cold. Full verification, prior-state loading and full-generation publication still occur; timings alone do not attribute costs to phases. No speculative tuning was added. Larger repositories, cache eviction, long archive growth, concurrent load, real Git provenance and query/startup costs remain unmeasured. M8 supplies broader performance/safety evaluation.
