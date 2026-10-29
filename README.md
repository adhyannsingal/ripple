# Ripple

A planned local Rust CLI that maps TypeScript Git changes to symbols and explains potentially affected code and test files through dependency paths.

This repository currently contains the build specification, execution plan and a tested planning helper. The analyzer has not been implemented yet. The product is named Ripple; the repository and planned CLI use `ripple`.

- [Build specification](ripple_buildspec.md): authoritative product scope and constraints.
- [Implementation plan](docs/implementation-plan.md): 38 dependency-linked tasks across nine milestones, including acceptance checks and handoffs.
- [Agent instructions](AGENTS.md): how to start, work, validate and report each task.
- [Execution protocol](docs/planning/execution-protocol.md): asynchronous coordination, implementation defaults, recovery and integration rules.
- [Task ledger](docs/planning/tasks.json): current task state, dependencies, ownership and validation evidence.
- [Completion report template](docs/task-reports/TEMPLATE.md): required short summary after every task.

To begin, have the coordinator read the agent instructions and assign `M0-T01`. All implementation tasks are pending. The coordinator can be an agent; routine tasks and milestone checks do not require owner intervention.

Check task readiness with `python3 scripts/plan.py validate` and `python3 scripts/plan.py ready`. The helper validates planning state; it does not start agents.
