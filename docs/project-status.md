# Current capabilities

Ripple implements all six local CLI commands: index, inspect, status, impact, diff and test. Syntax/semantics/impact boundaries, stable IDs, explicit uncertainty, verified atomic incremental state and both-side witnesses are preserved. See [quickstart](../README.md), [architecture](architecture.md), [CLI](cli.md) and [limitations](limitations.md).

| Evidence | Actual scope |
|---|---|
| Compiler agreement | Owned 3-edge and pinned four-source 16-edge projections; no general TypeScript accuracy claim |
| Incremental correctness | 248 default generated comparisons, 1,024 extended comparisons, targeted race/fault tests and pinned history replay |
| Test impact | Per-file head/absent/unindexed partitions; symbol/fallback tracks, ratios and complete machine selection within requested depth |
| Controlled faults | Five killed owned mutations: symbol 3/5, combined/import 4/5, all-tests 5/5; invalid/timeout/survivor exclusions and known dynamic/global-state misses |
| Performance | Small warm-cache library and verified-CLI workloads; phase/resource/profile evidence; unsafe metadata shortcut rejected |
| Installation | Six-command smoke on macOS/Linux/Windows; four checksummed 0.4.0 candidates; three CI fresh-runner smokes plus local Apple Silicon smoke |

All M0–M8 gates and all 43 tasks are verified in the [ledger](planning/tasks.json). The owner-approved MIT license, final evidence, dependency notices and 0.4.0 archive handoff are complete; see the [release checklist](release-checklist.md) and [M8 report](task-reports/M8.md). Public publication is separate; this page does not assert a tag exists. No selected test set is sufficient by construction, and index archives have no GC.
