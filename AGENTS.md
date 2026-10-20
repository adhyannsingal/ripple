# Ripple agent instructions

Use **Ripple** for the product, `ripple` for the CLI, `ripple-` prefixes for crates, and `.ripple/` for local index state.

## Read before work

1. Read `ripple_buildspec.md`, the authoritative product scope.
2. Read `docs/implementation-plan.md` and `docs/planning/execution-protocol.md`.
3. Read your task card, relevant ADRs, and existing fixtures/tests in the owned subsystem.
4. Inspect `docs/planning/tasks.json` and current Git state. This repository initially contains planning documents only; do not assume planned code or checks already exist.

The build spec controls scope. The plan supplies implementation defaults and scheduling; it does not silently supersede the spec. Record intentional architectural changes in an ADR and update affected cards/contracts. README claims must describe implemented behavior.

## Start and continue autonomously

The coordinator selects tasks whose dependencies are **integrated and verified**, using milestone order and then task ID. Different agents may work on independent ready tasks in isolated branches/worktrees. Do not create extra agents unless the invoking user or execution environment authorizes delegation. A solo agent follows the same dependency order.

Before coding, state briefly: problem, expected behavior, non-goals, affected invariant, and acceptance test. Claim the task through the coordinator before editing. Do not edit another agent's files or the shared task ledger concurrently. The coordinator owns integration and ledger updates.

Use the defaults in the execution protocol for routine decisions. Continue after each completed task to the next ready task when your assignment authorizes continuing. If blocked, record the blocker and work on an independent ready task within your authorization. Do not invent test results, credentials, metrics, approvals, or completion.

## Engineering rules

- Preserve the syntax → semantics → impact boundaries and crate dependency direction in the plan.
- Preserve semantic IDs, deterministic ordering, explicit uncertainty, witness provenance, and atomic persistent updates.
- Add fixtures for each new extraction/resolution behavior. Tree-sitter syntax is not TypeScript semantic proof.
- Keep imports, calls, and test reachability explainable; never claim a selected test set is sufficient.
- Use both base and head state for diff analysis, including deleted symbols and removed relationships.
- On incremental behavior changes, compare canonical incremental and cold state. On resolver changes, update oracle evidence once the harness exists. On hot-path changes, record a benchmark comparison once the harness exists.
- Keep unsupported syntax and unresolvable constructs visible in `docs/limitations.md`.
- Do not add frontend, authentication, hosted services, LLM features, or languages outside the build spec.
- Follow repository checks as they become available. No code task is complete until its acceptance checks and applicable CI pass, or it is explicitly recorded as blocked.

## Finish every task with a short report

Create or update `docs/task-reports/<TASK-ID>.md` using `docs/task-reports/TEMPLATE.md`. Include what changed and why, actual checks/results, fixtures, invariants, performance impact, limitations, and follow-ups. Use “not measured” where appropriate. Include the commit SHA in the integration ledger to avoid a self-referential commit hash in the report.

Send a concise completion message:

```text
<TASK-ID> — <done / blocked / review>
Changed: <one sentence>
Validation: <checks and results, or why blocked>
Limits/follow-ups: <none or brief list>
Report: docs/task-reports/<TASK-ID>.md
Next: <next ready task or integration dependency>
```

Code completion is not integration. The coordinator marks a task `done` only after integration, review, and validation on the shared integration branch. Human approval is not a routine milestone gate. Ask only for genuinely unavailable input or actions outside the authorized scope; continue independent work while waiting.
