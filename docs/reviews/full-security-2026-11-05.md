# Whole-repository security verification — 2026-11-05

Reviewed source: `6a8df464cc8fe3f6618d99f6c7a55680a2462289` (Ripple 0.4.0). This audit ran after M8 completion. No product, dependency or release artifact bytes changed. The existing untracked `graphify-out/`, ignored build caches and private environment/configuration were not audited or modified. No deployment/publication was performed.

## Result

No critical/high-severity issue was identified in the reviewed scope. Automated checks passed. Two low-severity defense-in-depth/resource findings remain below; this is not a penetration-test certification or proof of absence of vulnerabilities.

## Fresh checks actually run

| Check | Actual result |
|---|---|
| cargo-audit 0.22.2, complete Cargo.lock graph | 122 packages, 1409 advisories; zero vulnerabilities or informational warnings |
| RustSec database | `26fd2d43f7280a1fb925a3743ec463017afe7119`, updated 2026-11-05T09:50:49+01:00 |
| npm audit, eval/oracle lockfile | Zero vulnerabilities across all severity levels; this is the repository's only npm lockfile |
| Gitleaks 8.30.1, all Git refs/history | 246 commits / 8.97 MB; zero findings, redacted output |
| Gitleaks, tracked working files copied into disposable directory | 8.62 MB; zero findings; ignored/untracked directories excluded |
| cargo test --workspace --locked | 275 passed, zero failed |
| cargo clippy --workspace --all-targets --locked -- -D warnings | Passed |
| cargo fmt --all -- --check; git diff --check | Passed |
| Python tooling/release, replay, mutation metrics, oracle metrics | 22 + 4 + 3 + 4 = 33 passed |
| npm test --prefix eval/oracle | 6 passed |
| scripts/release.py verify, all four recorded artifact directories | Archive/member hashes, inventory, path and resource rules passed; cross-platform binaries were not executed in this audit |
| Workflow policy inspection | All external actions pinned to full commit SHAs, contents read-only, checkout credentials not persisted, no pull_request_target |
| scripts/plan.py validate / ready | 43 tasks / 9 milestones consistent; no ready tasks |

Raw local logs are `/tmp/ripple-whole-security-{rustsec,npm,gitleaks,working-files}.*`, `/tmp/ripple-whole-security-{tests,clippy,python,node}.log`. These temporary logs are not durable deliverables; this report retains their results. Audit tools consulted current advisory services; no dependencies were upgraded.

## Security boundary review

Reviewed all seven crate boundaries, with targeted inspection of CLI output/error handling, repository paths/scanning, config inheritance, Git adapters, persistent storage/publication, release packaging and evaluation subprocess/network handling. Product operations use no hosted service, authentication surface or project test execution. No authored unsafe Rust block or runtime network client was identified.

- Git uses argument arrays, end-of-options on refs, sanitized Git environment, disabled filesystem monitoring, no lazy fetch and bounded direct-child/output handling. Existing regressions cover malicious refs, external diff/textconv/filter/fsmonitor scripts, environment redirection and preservation of dirty user state.
- Scanner uses anchored directory handles and regular-file validation, with source symlinks excluded, no-follow/nonblocking Unix opens and file/entry/depth limits. Config inheritance has a depth bound. Parser byte/time/recovery limits remain explicit. Existing symlink, root replacement, invalid-byte and parser-budget tests passed on this macOS host; Linux-only FIFO coverage was not executed locally.
- SQL values are bound parameters; interpolated table/column names inspected are internal fixed lists. SQLite opens no-follow with defensive/trusted-schema settings; state publication uses transactions, captured-input verification and integrity seals. Corruption, incompatible schema, rollback, concurrent-writer and stale-generation regressions passed. Hash seals provide integrity, not adversarial authenticity.
- Human output escapes untrusted snippets/control characters, while machine output retains structured original evidence. Direction-marker/control regressions passed.
- Evaluation fetches pinned hash-checked source inputs and uses disposable directories. Runtime mutation tests are owned fixtures; fetched project scripts are not executed. Tool binaries, manifests, PATH and private destination directories remain trusted inputs.
- Release verification checks duplicate/case-colliding names, traversal, symlinks, declared expansion limits and hashes. `--smoke` explicitly executes the candidate; hashes from the same untrusted supplier do not authenticate it. Actions are scoped to build/test/artifact upload, not public publication.

## Findings and follow-ups

**SEC-01 — Low: compressed input and manifest are read before bounding their size.** In [release.py](../../scripts/release.py), `verify` uses `manifest.json.read_text()` and `archive.read_bytes()` before inspecting ZIP expansion limits. A deliberately huge local candidate/manifest can exhaust verifier memory, despite later 128 MiB expanded/64 MiB member limits. This is a code-inspection finding; no OOM was induced. It requires an operator to supply that package and affects developer release tooling, not normal Ripple CLI analysis. Follow-up: cap manifest/ZIP input sizes on an opened regular handle, stream the archive hash, then retain expansion checks; add oversized-input rejection tests. Until then use the recorded, trusted four-candidate manifest and avoid treating the verifier as a general hostile-file sandbox.

**SEC-02 — Low in the documented local-use model: aggregate resource consumption lacks a complete process/storage cap.** Source capture can accumulate many individually valid files before downstream aggregate checks; Git invocations have individual deadlines rather than a total analysis deadline, and immutable local index generations have no GC/quota. A very large/adversarial repository or repeated indexing can exhaust memory, CPU time or disk. Existing per-file/entry/parser/stored-generation limits reduce exposure but do not provide a global bound. This is already documented scope, not a newly reproduced exploit. Follow-up: enforce aggregate capture budget before retaining bytes, design command-wide cancellation and explicit retention/quota behavior. No user cache or worktree was deleted during this audit.

These findings do not demonstrate remote code execution, SQL injection or data exfiltration. Shared hostile filesystem mutation, malicious trusted executables/Git private metadata, unsigned-artifact authenticity and native dependency unknown vulnerabilities remain outside the assurance provided here. Archives are unsigned/not notarized. No new fuzzing/sanitizer campaign, Linux/Windows rerun, hosted-runner isolation proof or external penetration test was performed in this audit; prior platform gates remain separately recorded in the release checklist. Dependency advisories and secret signatures detect known patterns only.

Checkpoint: audit complete; runtime source and candidate archives unchanged. The two hardening follow-ups require implementation and focused regression checks before being claimed resolved.

## November 6 follow-up

M9-T01 bounds verifier manifests to 1 MiB and compressed archives to 160 MiB, rejects nonregular/symlink input paths, hashes in bounded chunks and uses the same opened archive handle for ZIP verification. Oversize, growing stream, symlink/directory and expansion regressions pass; all four original archives verify unchanged. This addresses SEC-01 for the documented private-directory/trusted-executable model; shared hostile concurrent in-place writes remain outside it. SEC-02 remains tracked separately: indexing already has a 256 MiB capture wrapper, but standalone discovery needs equivalent protection; command-wide cancellation and index quotas/retention remain future work.
