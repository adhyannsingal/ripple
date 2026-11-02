# ADR 0021: Verified cold publication and capability-scoped CLI

Status: implemented in M2-T05; integration/platform gate recorded separately.

## Pipeline and boundaries

`ripple index ROOT` cold-builds a single Git-root TS/TSX project. The CLI supplies `ripple_git::worktree::observe` as an explicit provenance provider to `ripple-index::build`. Git subprocesses stay in ripple-git; the production index crate still depends only on core/parse/resolve plus storage dependencies. The parser/extractor and resolver operate on captured bytes. Impact consumes validated loaded graph snapshots without storage/Git access at each hop.

Capture repository-supported source and ignore inputs, main tsconfig and a finite local inheritance chain, package manifests in visited directories and recognized root lockfiles: package-lock.json, npm-shrinkwrap.json, yarn.lock, pnpm-lock.yaml, bun.lock and bun.lockb. Follow config probes incrementally so an exact winner does not unnecessarily read its fallback. Explicit auxiliary paths use anchored Worktree reads, reject symlinks/nonregular parents and do not enter mandatory excluded directories. Cached bounded directory observations keep capture coherent without repeated full scans. Total accepted capture is 256 MiB, with 16 MiB per input and 100000 listed entries; config/parse/resolution have their own tighter limits. This is not a total-process memory quota.

An absent main config uses an explicit implicit-default config with a missing input dependency, never fabricated filesystem bytes. Package JSON is parsed strictly with duplicate-key rejection. Ordinary metadata/scripts/dependency declarations are retained but never executed. Unmodeled package entry points/conditions/types/workspaces or module mode produce explicit policy diagnostics and prevent successful configured matches. Package bytes and the package-policy version participate in the effective fingerprint. Conditional source-inventory matching is not current Node/TypeScript runtime equivalence.

Parse/extract, resolve modules and lexical calls/references, then recapture and compare source/config/ignore/package/lock membership/digests, missing auxiliary probes, discovery diagnostics, effective config and HEAD/dirty provenance. Retry at most twice (three total attempts). Continued edits/read failures preserve the previous complete generation. Compatible repeated index operations always cold-build a new generation; no incremental reuse/cutoff is claimed. Canonical cold equality excludes operational generation/time/origin as specified by core contracts.

Publish the validated snapshot and captured bytes atomically through Store. `created_at` is a `unix:<seconds>` wall-clock string. The analyzer identity composes cold-builder, syntax, relative/module/config/package and graph policy versions. Metadata summary and per-generation `modules-N.json` are derived post-commit artifacts. Module evidence is written via a synced temporary file/rename; failure reports pending cache/summary after a committed generation, not rollback. SQLite graph/facts/input bytes remain authoritative. A cache must be recomputed/validated before later reuse; current queries recompute module completeness and never trust that sidecar for freshness or graph loading.

## Provenance and security

Read Git HEAD and dirty status with argument-array subprocesses, no shell, redirected stdio, a 10-second command deadline and 8 MiB stdout/stderr caps. Clear inherited GIT_* redirection/config/trace variables case-insensitively; use isolated system/global configuration, disable optional index locking/refresh, filesystem-monitor commands and untracked cache. Refuse configured repository clean/process filters before status. Reject indexed submodules and nested Git projects. No checkout, stash, stage update, network operation or project script is requested. Dirty status follows this declared repository-local policy, excluding .ripple and unsupported submodule/filter behavior; it does not promise equivalence to arbitrary user-global filters/EOL/ignore settings. Test fixtures verify the user's Git index and tracked bytes are unchanged.

The Git installation and private local repository metadata/filesystem are trusted. Active hostile replacement of directories/configuration, hard links/network filesystems and malicious Git binaries are outside the containment guarantee. Capability-relative reads, observed symlink checks and SQLite NOFOLLOW/defensive settings are guards, not an OS-atomic source snapshot or authenticated cache. See [Git command controls](https://git-scm.com/docs/git) and [status porcelain/optional refresh](https://git-scm.com/docs/git-status).

## Queries and output

`status`/`impact` load one compatible generation, then compare two fresh captured views against stored input/config/origin provenance. They reject stale/changing/incompatible state with refresh/repair instructions and never silently rebuild. Missing status does not create an index. A post-verification edit can make a just-published generation stale; subsequent queries detect it. No stat-only shortcut is used.

Impact selection accepts a unique qualified name, path::qualified.name or exact semantic key. Ambiguous short names fail with qualified choices. Query results retain both evidence tracks, weakest-hop confidence, original witness spans/rules, test partitions and explicit depth/display completeness. Display limiting never reduces complete machine evidence or potentially-related test sets. Human text escapes nonprintable controls; JSON preserves exact typed values. Incomplete analysis remains visible even when a report exits successfully. Neither set is a sufficient test selection.

## Evidence and deferred work

Unit/CLI fixtures exercise canonical cold rebuilds, reopening, source add/delete/edit/config/ignore/HEAD races, bounded retries, post-verification staleness, inherited configs/binary locks, cache failure after commit, unsupported package policy, schema/name errors, environment spoofing, symlink boundaries and disabled filesystem monitors. `scripts/m2-smoke.py` runs all implemented commands from a clean installed binary in a disposable root with spaces/Unicode; CI does this on Linux/macOS/Windows. Security and final results belong in the milestone report after actual verification.

This is a v0.1 development candidate, not a public release. Git diff/test commands, oracle metrics, incremental indexing, archive GC, benchmarks and final related-test safety evaluation remain later milestones. Performance and power-loss fault injection are not measured.
