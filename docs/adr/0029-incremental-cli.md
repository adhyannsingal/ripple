# ADR 0029 — Default verified refresh and explicit full rebuild

Status: accepted for M5-T04

Make `index` call the verified incremental library by default; `--full` calls the same cold builder used as the correctness baseline. Keep the format-v1 index envelope and existing fields, with additive `update` counters/reasons. Distinguish accepted-attempt parser/module/reference work from total parse work across rejected captures, actual source deltas from unavailable history, and invalidation candidates from resolved files. Human output escapes data through the existing terminal renderer.

Every successful refresh still publishes one complete immutable generation. No-op means canonical semantic idempotence/zero parses where supported, not zero writes or reclaimed history. Failed capture/parse/write operations preserve the prior committed generation; verified status/impact require current inputs and never silently refresh. Unknown contexts and unsupported constructs remain visible. `--full` does not silently delete/migrate unsupported on-disk formats; incompatible schema repair remains explicit archive/rebuild.

Extend the three-platform clean-install gate to `scripts/m5-smoke.py`, which first runs the prior v0.2 capabilities and then verifies actual no-op/body/global/forced/failure/staleness counters and Git-index preservation. CLI tests compare default refreshed state to forced cold state, test malformed input recovery and ensure unsupported formats fail without success output. Underlying transaction/race/independent-cold/generated tests remain mandatory.

Keep the workspace version `0.2.0-dev`. M5 completes incremental implementation, while M6 owns expanded properties, real-history replay and the measured v0.3 candidate gate. There is no public release/tag or performance/test-safety claim here. Algorithm/format caveats are documented in `docs/incremental.md`.
