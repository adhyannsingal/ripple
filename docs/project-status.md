# What Ripple does today

Ripple is a local Rust CLI in development for explainable TypeScript change impact. M0 source/graph/index contracts, M1 source understanding and the M2 cold persistent graph pipeline are implemented. Git endpoint comparison, incremental updates and the dedicated test command remain later milestones.

| Area | Implemented behavior |
|---|---|
| Capture | Supported TS/TSX, repository ignores, finite local config inheritance, package policy and recognized root lockfiles; anchored reads, explicit exclusions/symlink rules and content/path verification. |
| Syntax | Bounded Tree-sitter parsing, source spans, stable supported symbols, signature/body fingerprints, import/export/scope/reference facts and explicit unsupported diagnostics. |
| Semantics | Configured source lookup, aliases, named/star/default re-export fixed points and supported lexical local/imported calls/reads; shadowing, type-only and ambiguity stay explicit. |
| Storage | Immutable typed SQLite generations, captured bytes/facts, atomic publication, integrity/version checks, concurrent reader/writer behavior and derived-summary recovery. |
| Queries | Deterministic reverse witnesses with confidence/provenance, independent symbol-backed/module-fallback tracks, potentially-related test partitions, depth/display limits and hubs. |
| CLI | index, inspect, status and impact in human/versioned JSON. Persistent queries verify freshness and reject stale/incompatible state; ambiguous symbols need qualified choices. |
| Verification | Fixture/adversarial tests, installed capability smoke and Linux/macOS/Windows CI; see milestone reports for actual gate outcomes. |

From a supported Git project root, `ripple index .` builds a cold generation; `ripple status` verifies it and `ripple impact path/to/file.ts::symbol` explains potentially affected code/tests. `inspect` remains a syntax-only debugging view. Use `--format json` for typed complete evidence. No Node runtime, database server, hosted service or project-script execution is required.

The reproducible repository demo is `python3 scripts/m2-smoke.py --binary target/debug/ripple`, after a locked workspace build; the installed form takes `--install-root`. It uses a disposable Git root with spaces/Unicode, checks a two-hop semantic witness and distinguishes one exact test from one conservative fallback addition. It also verifies stale-content rejection and unchanged Git-index bytes.

Index builds always cold-recompute. Worktree verification reads/hashes contents and is not an OS-atomic snapshot. Supported package/module policy is narrow and diagnosed; external Git filters, submodules and nested Git projects are unsupported. Test sets are potentially related, never sufficient. No compiler-oracle accuracy, latency, incremental-equality or mutation-safety measurements are claimed.

See [M2 gate](task-reports/M2.md), [security/verification review](reviews/M2-security-verification-2026-11-02.md), [CLI usage](cli.md), [resolution policy](resolution.md), [storage](storage.md), [limitations](limitations.md) and the [task ledger](planning/tasks.json). The next scheduled work after M2 is M3-T01, the pinned compiler oracle; independent Git endpoint work is also dependency-ready.
