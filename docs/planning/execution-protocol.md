# Asynchronous execution protocol

## Authority and operating model

`ripple_buildspec.md` is authoritative. `../implementation-plan.md` contains task cards and milestone gates. `tasks.json` is the machine-readable execution ledger. `../../AGENTS.md` is the instruction entry point. Task reports persist handoffs across sessions. This is a scheduling protocol; it does not install a background scheduler or start agents.

Use one coordinator/integrator and up to three implementation workers when delegation is authorized. Start with two workers until contracts stabilize. The coordinator can be an agent; the owner need not approve each task. A single agent can occupy both roles sequentially. Do not assume agents continue running after their host session ends.

The coordinator owns the integration branch, `tasks.json`, shared manifest/lockfile changes, and cross-crate contract edits. Workers own assigned paths on task branches/worktrees. Shared files have a single writer at a time. Package additions go through the coordinator; avoid independently regenerating `Cargo.lock` on every branch.

## State transitions and claims

States: `pending → in_progress → review → done`; `in_progress/review → blocked`; `blocked → pending` after the blocker is resolved. `done` means integrated and verified, not merely committed on a worker branch. A regression may reopen a task with a reason and a new report entry. Archive the prior completion evidence, clear obsolete completion fields, recursively return completed dependents to pending revalidation, and reset affected passed gates. Preserve code/commits and report history; dependent work must not remain falsely complete.

1. Coordinator runs `python3 scripts/plan.py validate` and `python3 scripts/plan.py ready`, then checks Git status, integration ancestry, live workers and ownership. Select the lowest milestone/task ID that is ready, or another ready card to fill an independent lane. A milestone target week is a planning estimate, not a completion claim or a runtime deadline.
2. Coordinator records `owner`, `branch`, `base_commit`, `status=in_progress`, `started_at`, a current `heartbeat_at`, concrete `claimed_paths`, `checkpoint` and `next_action` before dispatch. Also update the top-level coordinator session/checkpoint. Timestamps use ISO 8601 with a timezone; Git commits use full SHAs. Claim writes are serialized through this single writer; a shared JSON file is not an atomic distributed queue.
3. Create an isolated worktree from the integration branch, for example `git worktree add ../ripple-M1-T02 -b agent/M1-T02 <integration-branch>`. Preserve existing dirty work. Workers receive task ID, base commit, owned paths, dependencies, and the report requirement.
4. Worker validates prerequisites and states the five preflight items from `AGENTS.md`. If a contract is missing, request a coordinator decision with a concrete default; do not improvise incompatible APIs across branches.
5. Worker implements, runs the card checks, reviews the diff, commits focused changes and the task report, then sends the short summary. Report failures honestly. `review` work cannot satisfy dependent tasks.
6. Coordinator reviews acceptance evidence and semantic/architectural invariants, integrates one task at a time, resolves conflicts, and runs applicable integration checks. Then record `done`, `completed_at`, `integrated_commit`, `validation`, and report path. Dependencies become eligible only now.
7. At a milestone boundary, run its gate on integrated state. Record `gate_status`, `gate_evidence`, and `completed_at` on the milestone. Announce a short milestone summary and continue automatically within the assigned scope.

Planning validation is read-only and provides no locking or proof that recorded checks ran. Run it before/after every ledger mutation; the single coordinator serializes and persists ledger edits on the integration branch. A `done` record needs a real report, integrated SHA and structured validation entries (`command`, `outcome` of `passed`/`failed`/`not_run`). Validation entries default to required; use `required: false` only for explicitly optional checks. All recorded required checks must pass before `done`. For a blocked task, `blocker` records `cause`, `attempted`, and `next_action` (plus the failed command and dependent IDs when relevant). Release claims at successful integration. Report and SHA checks do not substitute for reviewing actual output/ancestry.

Before dependent work starts, confirm that its integration base contains all prerequisite commits. Rebase/update task branches after shared contract changes; rerun affected checks. Prefer narrow follow-up cards over expanding unrelated work.

## Recovery, blockers, and context handoff

- Update a heartbeat at task start, substantial progress, and handoff (target at least every 30 minutes during active execution). Persist a checkpoint before a session stops. A heartbeat older than two hours is a recovery signal, never proof that a worker has stopped.
- Before reclaiming, inspect the host's live-agent status, branch commits, task report, and worktree changes. Contact/stop the original worker where available; only reclaim after confirming it is inactive. Preserve unfinished work on its branch. Record the reassignment and restart from the last verified checkpoint.
- After two failed attempts at the same issue, reduce it to a fixture/reproducer, record evidence, and hand it to the coordinator. The coordinator may repair it, add a narrowly scoped prerequisite task, or select other ready work. Never mark a blocked task done to unlock dependents.
- Blocker records include cause, exact failed command or unavailable input, attempted fixes, next action, and dependent IDs. Escalate to the owner only for unavailable credentials/resources, incompatible product requirements, irreversible actions outside authorization, or an inability to progress after independent ready work is exhausted.
- A blocked task retains claimed paths until the coordinator explicitly releases them. Release claims before assigning the same files elsewhere, while keeping the unfinished branch intact. Narrow repair tasks may be inserted as prerequisites without marking their failing parent complete; record the new dependencies in both plan and ledger.
- Each dispatch/resume includes: task ID, integration base SHA, owned files, relevant contracts/ADRs, checks already run, report path, current blocker, next concrete action. Use repository records rather than relying on chat history.

## Default decisions: proceed without routine owner questions

These are implementation defaults aligned with the amended build spec. M0-T02 establishes syntax/identity contracts, M0-T04 graph/evidence contracts, and M0-T05 index/revision contracts. Encode and test only the boundary needed by each consumer before that consumer starts. Record changes in an ADR; update the spec if scope changes.

| Gap in spec | Default / acceptance consequence |
|---|---|
| Project naming | Product: Ripple. Repository and CLI: `ripple`. Crates use `ripple-` prefixes; local index state lives in `.ripple/`. Keep `ripple_buildspec.md` as the authoritative spec filename. |
| Git backend | Start with the installed Git executable behind `ripple-git`, invoking argument arrays without a shell and parsing machine-readable/NUL-delimited output. Isolate the adapter so it can be replaced by `gix`/`git2`. Record an ADR; installation documents the Git prerequisite. |
| Oracle | Use the TypeScript compiler API in a pinned Node evaluation harness. This is an evaluation dependency, not a production CLI dependency. Resolve current compatible versions during implementation and commit lockfiles. |
| Version/dependencies | Bootstrap chooses a stable Rust toolchain/MSRV, compatible Tree-sitter grammar/API versions, SQLite distribution strategy, and maintained crates, then pins them in toolchain/lockfiles and records ADRs. Do not invent exact version numbers in this plan. |
| One project | A single repository root and main `tsconfig.json`. Implement documented `baseUrl`/`paths` behavior and finite local config inheritance; unsupported project references, package conditions, external modules, or config forms produce visible diagnostics. Hash the effective config and its inputs. |
| Source coordinates | UTF-8 byte offsets, half-open byte spans, 1-based display lines/columns (columns count bytes and are documented). Convert Tree-sitter and Git coordinates at boundaries. Test Unicode, CRLF, EOF, insertions, and zero-length deletion hunks. |
| IDs / overloads | Repository-local stable project key plus normalized relative path, qualified name, kind, and deterministic disambiguator. Do not include absolute machine path, line number, DB row ID, or current revision. Group overload signatures with their implementation when supported; otherwise diagnose ambiguity. |
| Hash semantics | Use versioned syntax-structure-aware hashes: node kinds/structure plus meaningful lexical content, with separate signature/body domains. Never use only whitespace-stripped token sequences. Preserve automatic-semicolon-insertion effects, including line breaks in comments, and meaningful TSX/string/template content. Ordinary trivia edits are unchanged only if parsed structure and semantic content are unchanged. Error nodes remain explicit and prevent unsupported certainty. Signature external facts include return types, modifiers, generics, parameters and export shape. |
| Class changes | Own class header/member shape changes; map method body edits to the smallest enclosing method rather than double-reporting the class. Add/delete class members changes the class external fingerprint. Define unsupported field/initializer impacts explicitly. |
| Graph schema mismatch | The suggested symbol-only edge columns cannot represent file/test/boundary nodes. Use a common typed node table and edge `source_node_id`/`target_node_id`, with symbol/file detail tables. Persist unresolved records with source node and provenance. ADR required before migration v1. |
| Graph direction / evidence | `CALLS/REFERENCES`: dependent symbol or concrete module-level reference owner → target symbol; `IMPORTS/REEXPORTS`: consumer file → provider file; `DEFINES`: file → symbol. Compute two tracks: symbol-backed traversal uses semantic references/calls; module_fallback starts from defining/changed files and follows reverse file imports/re-exports. Never unconditionally expand containment within the symbol-backed track. A fallback hop permanently labels that witness as fallback, even when every import edge is RESOLVED. Preserve shortest deterministic witnesses in each track separately. |
| Ambiguous methods | No global name matching promoted to `RESOLVED`. Emit candidates only when bounded lexical/import/type evidence justifies targets; otherwise emit unresolved with a reason. Same-file scope and shadowing must be checked before import resolution. |
| Diff revisions | `A..B` compares those explicit tree endpoints; `A...B` is unsupported initially with an actionable error. Resolve refs to immutable SHAs once. Do not checkout, stash, or modify the user's working tree. Read trees/blobs and parse hunks through the adapter. |
| Worktree capture | Capture file membership and source/config/ignore bytes once and analyze those bytes. Before publication, re-scan/re-hash all relevant inputs and compare with the captured manifest, including HEAD provenance. Retry at most twice (three total capture attempts); persistent concurrent edits fail explicitly and preserve the prior generation. This verifies the captured view, not an OS-atomic filesystem snapshot. Edits after verification can make it stale; later queries verify freshness. Include supported untracked files and exclude ignored files. Historical revisions use immutable blobs, without modifying worktree-index metadata. |
| Deleted/changed relationships | Traverse each changed symbol in the base graph and head graph where it exists; union by semantic IDs and keep revision labels on witnesses. This preserves deleted callers and removed import/call edges. Report base-only test paths separately when the file no longer exists at head. |
| Rename handling | Use Git rename information as a file-change hint. Semantic path-based identities may become delete+add; no heuristic symbol rename guarantee in v0.x. A hunk outside a supported symbol becomes an explicit file-level change, never silently omitted. |
| Path uncertainty / selection | Path confidence is the weakest semantic hop; `evidence_kind` independently distinguishes symbol_backed from module_fallback relevance. Default potential-related test selection is the conservative union. Human/JSON output shows symbol-backed, fallback-only and union sets/counts; an item with both retains both witnesses. Report unresolved boundaries, parse errors and truncation; no set is claimed sufficient. A narrower evidence view is not a safe test-skipping guarantee. |
| Query limits | Proposed defaults: unlimited traversal depth, at most 100 displayed results, configurable `--depth` and `--limit`. Display limiting must not silently reduce traversal/test selection. Report visited/displayed counts, depth-boundary truncation, and hub summaries; expose complete machine output on request. |
| Freshness and query cost | `status`/worktree queries verify content/path/config/analyzer/schema compatibility and reject stale/incompatible state with refresh instructions. Initial correctness-first verification may read/hash the repository; do not claim end-to-end graph-only latency. Benchmarks separate process startup, input verification, graph loading and traversal. Historical SHA snapshots are immutable. No watcher/stat-only fast path or unverified mode until explicitly designed/measured; queries do not silently refresh. |
| Atomicity / metadata | SQLite is authoritative. Publish one complete generation atomically and read graph generations under a read transaction. Metadata JSON is derived and written by temp-file/rename after DB commit; recover disagreement from DB. Failed capture/build/write leaves the previous generation available and diagnosable (possibly stale, never presented as current). |
| Incremental cutoff | Cache syntax facts and external fingerprints separately. Replace changed-file facts atomically and re-resolve inbound users of changed/deleted/renamed symbols, imports, exports and re-exports until a fixed point. Include candidate/unresolved dependencies and previously missing module lookups so adding a file can repair old unresolved references. If safe invalidation cannot be proven, re-resolve all files while still reusing unchanged parses; optimize only behind equality tests. |
| Global invalidators | Effective tsconfig inputs, package manifest, recognized lockfiles, scanner/analyzer/hash/schema settings invalidate globally. Full rebuild is allowed and reported. Lockfile names and ignore policy are explicit in config docs. |
| JSON / errors | Add `--format json` for commands as a test/agent interface alongside human output. Version the envelope, sort arrays deterministically, keep stdout data-only, send diagnostics to stderr. Document usage/failure/incomplete-analysis exit semantics. Incomplete static analysis must remain visible even with a successful query exit. |
| Evaluation thresholds | No invented accuracy or latency guarantees. Require exact supported-fixture expectations, no false `RESOLVED` fixtures, zero canonical-equality mismatches, reproducible measured reports, and explanations of regressions. Gate resolver changes on no unexplained supported-scope precision regression; expand scope only from evidence. |
| Baselines | Measure symbol-backed and conservative-union selection separately against the same all-tests/file-import baselines and mutations. Disclose if fallback makes union selection equal to file-level reachability. Compare incremental with cold. Pin analyzer/corpus versions and regenerate affected final-candidate evidence after code changes; report denominators, empty cases, sample sizes and misses without hiding them. |

## Validation and integration policy

After workspace bootstrap, standard Rust checks are `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace --locked`. Bootstrap must make these real commands and document any platform-specific requirements. Workers run focused checks while developing; coordinator runs the workspace checks after integration. Oracle, replay, mutation, benchmark, and install commands are added by their owning tasks and documented under `eval/`, `bench/`, and `CONTRIBUTING.md`.

CI uses deterministic fixtures on every change. Network-dependent external corpora and long mutation/performance runs use explicit jobs or scheduled/manual runs with pinned inputs; they must produce evidence before their release milestone passes. Use temporary repositories/directories, bounded timeouts, subprocess cleanup, and independent copies for mutations. Do not run arbitrary fetched repository scripts as part of routine indexing tests.

No blanket shared-runner timing thresholds. Record hardware and repeat counts; compare equivalent states. Missing evidence is a blocker or “not measured,” not a green check. A milestone can progress internally with ready tasks, but its exit gate cannot pass with unfinished required cards.

## Coordinator persistence and planning tooling

The schema-v2 ledger is the durable checkpoint. `coordinator` records owner/session, integration branch, last checkpoint and next action; every claimed task records a base SHA, heartbeat, concrete file/subtree claims and resumable next action. Update checkpoints before a session ends and at handoff. A new coordinator inspects live-worker state and Git/worktrees before resuming; it does not automatically reclaim tasks or fabricate liveness from timestamps.

`owned_paths` in cards are descriptive areas and may contain patterns. `claimed_paths` are concrete repository-relative files or directory prefixes ending in `/`; globs, braces, parent traversal and `.git/` claims are invalid. Only the coordinator writes the shared ledger. Overlapping file/subtree claims among retained tasks are rejected. Assign non-overlapping subdirectories or serialize shared files; a read-only readiness listing is not an atomic claim operation.

Commands (Python 3.9+ stdlib only):

```bash
python3 scripts/plan.py validate
python3 scripts/plan.py ready
python3 scripts/plan.py ready --json
python3 -m unittest discover -s scripts/tests -v
```

Validation catches inconsistent IDs/titles/dependencies, cycles, missing required state/evidence/reports, overlapping claims and premature milestone completion. It does not evaluate code quality, run acceptance commands, verify Git ancestry, enforce live leases, send messages or launch workers. Coordinator review supplies those checks. M0-T01 adds this existing helper to CI alongside the new Rust checks.

## Version gates

A candidate is capability-scoped: v0.1=M0–M2, v0.2=M0–M4, v0.3=M0–M6, v0.4=M0–M8. Earlier candidates require only implemented-command install/smoke checks and their relevant evidence; they do not require the unfinished semester features. Final v0.4 readiness checks all required evidence on the integrated candidate. Public publication/tagging remains a separate authorized action. See the detailed version table in the implementation plan.

## Boundaries of autonomy

Implement, test, document, commit locally, and integrate within the user-authorized assignment. Do not publish packages/releases, push protected branches, spend money, contact others, change repository visibility, or execute destructive operations solely because they appear in a future task card. Prepare release artifacts and draft text first. If publication is separately authorized, proceed within that authorization; otherwise record a `release-ready` handoff and ask only at that final external step. This does not block other implementation milestones.

## Dispatch prompt

```text
Work on <TASK-ID> from docs/implementation-plan.md.
Read AGENTS.md and docs/planning/execution-protocol.md first.
Integration base: <SHA>. Branch/worktree: <path>. Owner: <agent>.
Dependencies verified: <IDs/commits>. Owned paths: <paths>.
Implement only this card and necessary tests/docs. Coordinate shared contract edits.
Use documented defaults; ask only on a real blocker and continue independent authorized work.
Run the acceptance checks and applicable repository checks.
Write docs/task-reports/<TASK-ID>.md and send the six-line completion summary.
Do not claim done until the coordinator integrates and verifies the result.
<Optional: after integration, continue with the next coordinator-assigned ready task.>
```
