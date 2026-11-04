# M5 selective-resolution oracle baseline

These reports were regenerated using implementation commit `6e98f1d` after the graph builder was factored for guarded reuse (`ripple-graph-v2-reuse`). Each comparison records the full analyzer commit, Rust source tree, binary hash, pinned Node 24.22.1/TypeScript 6.0.3 options and input hashes. The final integration SHA/checks are in the task ledger. The oracle validates the cold supported projection; incremental equivalence is separately guarded by 120 generated cold comparisons and the targeted invalidation matrix.

The [mini report](mini/report.md) retains 3/3 precision/recall; the [pinned external source projection](sourcemap-codec/report.md) retains 16/16 with no supported false positives/negatives or unmapped eligible edges. External context still contains 567 unresolved occurrences, 524 oracle exclusions, one unsupported constructor definition and six out-of-scope reads. This is not full-repository compatibility, general accuracy, performance or test-safety evidence. No resolver inference scope was expanded.

```sh
cargo build --locked -p ripple-cli
npm ci --prefix eval/oracle --ignore-scripts
python3 eval/oracle/compare_run.py --source eval/corpora/mini --binary target/debug/ripple --output /tmp/ripple-mini-m5
python3 eval/oracle/fetch.py eval/corpora/sourcemap-codec.json /tmp/ripple-m5-source
python3 eval/oracle/compare_run.py --source /tmp/ripple-m5-source --manifest eval/corpora/sourcemap-codec.json --binary target/debug/ripple --output /tmp/ripple-external-m5
```

Use pinned Node on PATH and new output/source directories. Acquisition verifies the existing licensed manifest and executes no upstream scripts. See the [evaluation scope](../../../../docs/evaluation.md) and [manifest](../../../corpora/sourcemap-codec.json). Raw `oracle.json`, `ripple.json` and `comparison.json` preserve denominators and uncertainty.
