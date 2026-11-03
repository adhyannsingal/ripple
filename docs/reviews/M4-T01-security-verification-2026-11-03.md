# Verification and security review — 2026-11-03

Reviewed integration revision: `91cb542633aeefa13990678a6adf65d27a21b8a4`.

Status: passed within the documented scope; all verification/platform checks passed. No new blocking code/security finding established in the reviewed scope. No implementation files changed during this review.

## Actual verification

- `cargo fmt --all -- --check`, `cargo build --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: **231 local Rust tests passed**, including eight Git adapter tests and existing capture/storage/graph/output regressions.
- Pinned Node oracle tests: **6 passed**; Python metric tests: **4 passed**; Python planning tests: **18 passed**. `python3 scripts/plan.py validate` and `ready` passed; next implementation task remains M4-T02.
- `python3 scripts/m2-smoke.py --binary target/debug/ripple`: passed; persistent two-track witness, stale detection and unchanged Git-index evidence retained.
- `git diff --check`: passed. `cargo tree -p ripple-git --edges normal --depth 1`: production Git adapter depends only on core.
- Latest source CI: [workspace](https://github.com/adhyannsingal/ripple/actions/runs/41706375624), [oracle](https://github.com/adhyannsingal/ripple/actions/runs/41706376168). Linux/macOS Intel/Windows and oracle passed all required steps, including clean install and CLI smoke.

## Security checks and review

| Area | Result/evidence |
|---|---|
| Rust dependencies | Fresh RustSec audit: **122 locked dependencies, zero known advisories and no warnings**. Advisory database commit `6c5fedf8ff03c485ed674d8f941e0180f3f72d70`. |
| Evaluation dependency | `npm audit --prefix eval/oracle --json`: **zero known vulnerabilities**. Exact TypeScript dependency/integrity is locked; corpus scripts are not installed or executed. |
| Credential patterns | **308 tracked working files and 672 distinct blobs reachable from all local Git refs** scanned (10,778,116 historical blob bytes). No AWS access-key, GitHub/OpenAI token or private-key pattern matches. No credential values were printed. No tracked symlink was skipped. |
| Git input/process boundaries | Reviewed argument-array invocation, ref revalidation/`--end-of-options`, actual commit-type validation, immutable endpoint binding, GIT_* isolation, disabled replacement/lazy fetching/pager/fsmonitor and external diff/textconv behavior. Invalid/malicious refs, direct tree IDs, missing objects, external drivers and read-only preservation have passing regressions. |
| Paths and resource bounds | Portable UTF-8/NUL records, strict malformed-record rejection, regular-blob-only reads and symlink/submodule policy reviewed. Per-command output, per-blob/tree/read bounds and range overflow checks exist; oversized/malformed/Unicode/CRLF cases pass. |
| Capture/persistence/output | Reviewed anchored source reads and symlink checks, captured-input verification/retries, parameterized data SQL, foreign keys/defensive SQLite/NOFOLLOW, generation publication and derived-artifact trust. Existing source/config race, store corruption/fault and terminal-control regressions pass. |
| Scope/evidence | Binary/non-UTF-8/nonregular changes retain explicit file-level uncertainty; unsupported dispatch never becomes a guessed precise edge. Configured diff order/color is isolated. Human output escapes terminal controls; machine JSON retains typed evidence. No test-set sufficiency claim. |

Applied the security-review and code-review checklists to this local CLI. Hosted authentication, browser/session and payment controls do not apply to the implemented product. The previous Windows-only test cleanup lint exemption remains confined to a disposable Windows fixture; Unix permissions are not weakened.

## Limits

The review assumes a trusted Git binary and private repository metadata/filesystem, as documented in ADR 0009/0021. Active hostile replacement/races, hard links/network filesystems, malicious binaries/descendants, OS-atomic capture and hard total-process memory/end-to-end deadlines are not established guarantees. Performance and power-loss injection are not measured. This is targeted code/fixture/advisory verification, not exhaustive penetration testing or a system-wide software audit.

Credential checks use the named patterns, not an exhaustive secret detector: untracked files (including the preserved `graphify-out/`), unreachable objects, remote-only refs and secrets outside these patterns were not scanned. Absence of matches is not proof that no secrets exist. Advisory results are a dated known-vulnerability snapshot, not a guarantee against undisclosed issues.

Checkpoint: preserve implemented source and existing task completion evidence. Review completed and evidence recorded in the ledger; next implementation remains M4-T02. No blocking follow-up identified within the supported scope.
