# M1 handoff review — 2026-11-01

Reviewed the M1 gate and its producers before M2 consumes FileFacts, starting from `ff5254554a562a00de3de9859a76edb3ce913892`. Scope includes discovery/capture policy, parser recovery and coordinates, supported symbol identities/ownership, structural hashes and ASI, imports/exports/scopes/references, inspect reports and acceptance fixtures. Earlier reviews remain evidence, not a substitute for this handoff check.

| Boundary | Review and acceptance evidence |
|---|---|
| Discovery | Explicit root, anchored reads, bounded entries/files, repository ignores, symlink/special-entry handling and test classification; scanner fixtures/platform tests. No atomic-snapshot or independent aggregate-memory guarantee. |
| Parsing | Correct TS/TSX grammar, UTF-8/CRLF spans, clean/recovered/failed states and bounded recovery diagnostics; parser/span tests. Syntax is not compiler proof. |
| Symbols/hashes | Stable structured identities, overload/role rules, supported owners, separate class/method fingerprints; 14 declaration programs, 24 hash pairs and independent digest oracle. No early cutoff justified by equality alone. |
| Lexical facts | Import/export/type-only evidence, shadowing scopes, callback/file ownership and opaque constructs; independent references fixtures and generated owner checks. |
| CLI | Versioned deterministic reports, diagnostic routing, explicit incompleteness, path policy, Unicode escaping and no Git/index writes; independent TS/TSX and human goldens. |

## Finding repaired

**Medium correctness/resource risk:** the primary symbol node-budget marker could be suppressed after 1024 earlier warnings. Complete extraction uses that marker to stop its supplemental pass. A cleanly parsed production-size reproduction with 1100 directive comments and 500050 empty statements hit the primary cutoff yet published one import, one export, three scopes and one reference with no symbols. Its diagnostics lacked `extraction_resource_limit`.

Primary resource markers now bypass detailed-warning truncation. Complete extraction therefore returns no supplemental facts after the primary node cutoff. A small independent recipe drives a private reduced node budget for a portable regression; it failed before the repair. Production retains its one-million-node limit. The installed CLI separately replays the original production-budget reproduction. Analyzer compatibility advances to `ripple-facts-v4-ts0.23.2-runtime0.27.1`; IDs and v1 hash encoding remain unchanged.

## Results and handoff

Baseline workspace checks passed (115 Rust tests). The repair passes 116 Rust tests, strict Clippy, formatting/build and the focused cutoff regression. All 18 planning tests, planning validation, installed production-budget smoke and [Linux/macOS/Windows CI](https://github.com/adhyannsingal/ripple/actions/runs/41341146389) passed. RustSec checked 122 packages with zero known vulnerabilities/warnings. The task ledger records integration before M2 begins. No other actionable defect was established in this pass. No performance or compiler-oracle measurements were made; known-advisory checks and code inspection are not an exhaustive security audit.

M2 may consume captured, validated facts after integration. Relative module lookup must preserve both successful and missing candidates, reject paths outside the root, and avoid guessed external-package targets. Direct export resolution must distinguish supported symbols, aliases, type-only exposure, ambiguity and unsupported/recovered providers. Re-export fixed points and config aliases remain their own cards; resolved imports alone must never become proof of symbol dependencies.
