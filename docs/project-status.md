# What Ripple does today

Ripple is a local Rust CLI in development for explainable TypeScript change impact. M0 source/graph/index contracts, M1 source understanding and the M2 cold persistent graph pipeline are implemented. M3 adds a pinned evaluation-only compiler oracle, comparison metrics and regression evidence. M4-T01 adds an immutable Git endpoint/tree/hunk library adapter. M4-T02 adds pure base/head symbol change classification with explicit file-level fallback and uncertainty. User-facing Git diff, incremental updates and the dedicated test command remain later tasks.

| Area | Implemented behavior |
|---|---|
| Capture | Supported TS/TSX, repository ignores, finite local config inheritance, package policy and recognized root lockfiles; anchored reads, explicit exclusions/symlink rules and content/path verification. |
| Syntax | Bounded Tree-sitter parsing, source spans, stable supported symbols, signature/body fingerprints, import/export/scope/reference facts and explicit unsupported diagnostics. |
| Semantics | Configured source lookup, aliases, named/star/default re-export fixed points and supported lexical local/imported calls/reads; shadowing, type-only and ambiguity stay explicit. |
| Storage | Immutable typed SQLite generations, captured bytes/facts, atomic publication, integrity/version checks, concurrent reader/writer behavior and derived-summary recovery. |
| Git library | Pinned A..B commits, scanner-compatible raw trees, add/delete/rename/text hunks and explicit binary/mode diagnostics; no worktree/index mutation. CLI diff is still pending. |
| Change classification | Captured base/head facts and Git ranges yield added/deleted/body/signature/unchanged or uncertain symbol records; class/method boundaries, renames and global inputs stay explicit. CLI diff and graph union remain pending. |
| Queries | Deterministic reverse witnesses with confidence/provenance, independent symbol-backed/module-fallback tracks, potentially-related test partitions, depth/display limits and hubs. |
| CLI | index, inspect, status and impact in human/versioned JSON. Persistent queries verify freshness and reject stale/incompatible state; ambiguous symbols need qualified choices. |
| Verification | Fixture/adversarial tests, installed capability smoke, pinned TypeScript oracle/metric tests and Linux/macOS/Windows CI; see milestone reports for actual gate outcomes. |

From a supported Git project root, `ripple index .` builds a cold generation; `ripple status` verifies it and `ripple impact path/to/file.ts::symbol` explains potentially affected code/tests. `inspect` remains a syntax-only debugging view. Use `--format json` for typed complete evidence. No Node runtime, database server, hosted service or project-script execution is required.

The reproducible repository demo is `python3 scripts/m2-smoke.py --binary target/debug/ripple`, after a locked workspace build; the installed form takes `--install-root`. It uses a disposable Git root with spaces/Unicode, checks a two-hop semantic witness and distinguishes one exact test from one conservative fallback addition. It also verifies stale-content rejection and unchanged Git-index bytes.

Index builds always cold-recompute. Worktree verification reads/hashes contents and is not an OS-atomic snapshot. Supported package/module policy is narrow and diagnosed; external Git filters, submodules and nested Git projects are unsupported. Test sets are potentially related, never sufficient. The oracle measures only the documented 3-edge mini and 16-edge external source projection. General accuracy, latency, incremental-equality and mutation-safety are not established.

See [M2 gate](task-reports/M2.md), [security/verification review](reviews/M2-security-verification-2026-11-02.md), [CLI usage](cli.md), [resolution policy](resolution.md), [storage](storage.md), [limitations](limitations.md) and the [task ledger](planning/tasks.json). See [M3 evaluation/gate](task-reports/M3.md). [M4-T01](task-reports/M4-T01.md) is integrated and verified. [M4-T02](task-reports/M4-T02.md) is integrated and verified. The next task is M4-T03, combining base/head impact.
