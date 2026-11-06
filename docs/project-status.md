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

All original M0–M8 gates and their 43 tasks are verified in the [ledger](planning/tasks.json). The owner-approved MIT license, final evidence, dependency notices and 0.4.0 archive handoff are complete; see the [release checklist](release-checklist.md) and [M8 report](task-reports/M8.md). Public publication is separate; this page does not assert a tag exists. No selected test set is sufficient by construction, and index archives have no GC.

## November 6 post-plan follow-up

Ripple is functional as a local developer preview for its documented TS/TSX scope; all six commands and compiling owned-repository behavior have passed local verification. [The practical exercise](examples/small-repo/README.md) independently confirms one known failure is selected with a symbol witness, a related import remains fallback-only, an unrelated test is excluded, stale state is rejected and historical queries preserve user state. The exercise still explicitly reports incomplete analysis (eight unresolved occurrences, including constructor/global/parameter-reference boundaries). This is finite evidence, not complete TypeScript support or guaranteed-safe test skipping.

M9 is a newly authorized post-plan follow-up, not a previously missing original milestone. Archive input bounds and standalone discovery's 256 MiB captured-byte cap are implemented with regressions; integration state and applicable CI are recorded in the ledger. Command-wide cancellation and index quota/retention remain pending in M9-T04. The original 0.4.0 archives retain their recorded M8 source/hashes; current follow-up changes have not been repackaged or given a new final-candidate evidence claim. Public tagging/release remains separate.
