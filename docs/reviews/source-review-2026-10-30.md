# Repository code and security review — 2026-10-30

Baseline: ccd37cb10bfade1b8c833f590ef8815a943d4e0d. Reviewed the current seven-crate workspace, planning scripts/ledger, fixtures, CI configuration, dependency setup and architecture/limitations. The earlier foundation review remains historical evidence; this pass revisited the contracts and focused regression work on newly implemented scanning and parsing. It adds no extraction, resolution, persistence or query features.

## Findings and repairs

| Priority | Finding | Repair and verification |
|---|---|---|
| Medium | A Worktree stored its canonical root only as a pathname. Renaming the root and replacing that pathname with a symlink redirected subsequent discovery/reads outside the selected tree. | Hold an open root capability and use directory-relative operations. The deterministic root-replacement regression read `outside` before the fix; it now retains `inside`. Validate opened files as regular; Unix reads also request NONBLOCK/NOFOLLOW. A Linux-only FIFO test checks bounded rejection at the final open. |
| Medium | The root configuration ignore exception also applied to directories named package.json or tsconfig.json, allowing ignored source subtrees into capture. | Restrict the exception to regular files. A memory-provider regression failed before the fix and passes afterward. Existing real configuration files remain captured. |
| Low | The 100000-entry bound was checked after a complete directory Vec had already been allocated. | Give SourceTree listings their remaining entry budget and abort worktree iteration at that limit. Test zero/over/exact budgets and require an error rather than silent truncation. Future tree providers must honor this contract too. |
| Low | Source discovery reused ADR 0010, which already identifies toolchain/dependency policy. Some architecture/README text also omitted implemented libraries. | Renumber discovery to ADR 0011, repair links and update current implementation claims. |
| State | Parser work remained blocked after its platform CI recovered. | Confirmed both prior runs passed; preserved blocker history, completed M1-T02 and released its claims before claiming this review. |

Parser implementation required no production repair in this pass. Added coverage for a nonzero progress-callback cancellation budget and the explicit 1024-diagnostic truncation record. These checks assert cancellation/report behavior, not a performance timing threshold.

## Review coverage

- Core path/identity/span, syntax validation, graph endpoints, provenance, confidence/evidence partitioning, immutable revision binding, manifest/generation and canonical equality contracts. Source validation and recorded equality remain distinct from live freshness or cryptographic hash verification.
- Scanner ignore precedence, source/test classification, captured bytes, portable path collisions, symlinks, root authority and resource limits; parser grammar selection, UTF-8, coordinates, missing/error nodes and cancellation/recovery.
- CLI help/version/exit behavior, fixture loader and controlled Git/process helpers, standard regression suites, Rust dependency direction, manifest/lockfile pins and development toolchain.
- Python coordination validation, task dependencies/claims/reports, current plan/spec scope and documentation links; CI has read-only repository permissions, pinned actions, disabled persisted checkout credentials and a platform matrix.
- Manual security review of filesystem input boundaries, opened-object validation, source decoding, deserialization boundaries and controlled subprocess use. No project scripts are executed by discovery/parsing. The SQLite implementation is still only a fixed-query bootstrap test; hosted/authentication/API attack surfaces do not exist here.

No additional actionable defect was established in the reviewed unchanged foundation. Planned but unimplemented features were not classified as bugs.

## Dependency and secret-pattern checks

Ran cargo-audit 0.22.2 (installed as a temporary review tool) against the final Cargo.lock using the fetched RustSec database: **114 packages, zero known vulnerabilities, zero warnings**. Database: 1395 advisories, commit `2295466c4e9df26647fd67d8fa1c705ac2fc7f81`, updated `2026-10-30T12:12:39+01:00`. Command: `cargo audit --json` (the temporary binary was invoked directly). Lockfile SHA-256: `36062bc140b7bf256eefb2d6279496cfae9b36f066b260e36e9809114747e8f8`.

A tracked-file pattern check for private-key headers, AWS access-key IDs and GitHub token forms found no matches. This limited pattern check is not proof that secrets are absent. RustSec results reflect that advisory snapshot; they do not exhaustively assess bundled native components, license obligations or undisclosed vulnerabilities.

## Validation and limits

On macOS with Rust 1.99.1: locked build, formatting, strict all-target Clippy, 62 Rust tests, 18 Python tests, ledger validation and CLI help/version pass. Final integration/platform CI evidence belongs in the task ledger. Linux CI additionally exercises the FIFO test; platform-specific test counts differ.

Performance is not measured. Capability-relative IO adds directory-handle/metadata operations; compare costs once the benchmark harness exists. The initial explicitly authorized root open uses ambient authority. Capture is not an OS-atomic snapshot; hard links, mounts, concurrent content writes and total memory across many individually bounded files remain limitations. M2-T05 must verify freshly observed membership/content before publication with bounded retries. This is a code/security review with regression and advisory checks, not a formal audit or penetration test.

Next implementation task: M1-T03 symbol extraction and structure-aware semantic hashes. The broader M1 milestone is still incomplete.
