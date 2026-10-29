# Plan amendments — 2026-10-21

Status: done (planning amendments validated; implementation authorized on 2026-10-28)

The eight findings from the plan review are addressed across the authoritative build spec, implementation plan, execution protocol, agent instructions and task ledger.

| Finding | Change |
|---|---|
| Hashes could hide behavior changes | Require syntax structure and meaningful content; add automatic-semicolon-insertion and newline-comment regression cases. |
| File fallback could look symbol-precise | Separate symbol-backed and module-fallback witnesses/counts; keep conservative-union test selection and evaluate both tracks independently. |
| Incremental properties arrived after optimization | Add M5-T05 as a prerequisite of early cutoff; M6 expands the existing harness. |
| Diff needed tests before detection existed | Move classification to M1-T01 and basic dual-track reachability to M2-T04; M7 finalizes revision-aware selection and its CLI. |
| Input could change during indexing | Capture bytes and membership, verify before publication, bound retries, preserve the prior generation on failure, and test concurrent changes. |
| Some task cards were oversized | Split source, graph and index contracts; split relative modules, config aliases and re-export resolution. There are now 38 cards across the same nine milestones. |
| Coordination depended entirely on prose | Add a read-only stdlib validator/readiness CLI, schema-v2 checkpoints and concrete path claims; include failure-path regression tests. |
| Completion ordering contradicted itself | Replay owns its repairs before benchmarking; release gates are version-specific and implemented commands gain install smoke before final packaging. |

Additional consistency fixes: typed-node SQLite schema, generation-consistent reads, distinct freshness/traversal benchmark costs, report checkpoints, dependency-safe task splitting/reopening, configured-author commit instructions, and local metadata/Python-cache ignores.

Validation: `python3 scripts/plan.py validate` passes for 38 tasks/nine milestones; `ready --json` lists only M0-T01. The 15 stdlib tests pass, including dependency readiness, cycles, ownership conflicts, evidence/reports, milestone order, malformed state and CLI behavior. Local Markdown links and `git diff --check` pass.

Rust checks are not applicable yet: the Rust workspace has not been implemented. No analyzer accuracy/performance claims were made. The planning helper validates recorded structure; coordinator review must still verify actual checks and Git ancestry. All implementation tasks remain pending.

Handoff: commit this reviewed planning baseline, then begin M0-T01 under the implementation authorization. Public pushing is not part of this local implementation step.
