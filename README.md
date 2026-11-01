# Ripple

A local Rust CLI in development that maps TypeScript Git changes to symbols and explains potentially affected code and test files through dependency paths.

M0 (contracts and bootstrap) is complete and reviewed: the workspace, CLI bootstrap, source/syntax/graph/evidence and index/revision/canonical contracts are implemented and tested. Run `cargo run --locked -p ripple-cli -- --help` or try `inspect` below. See [CONTRIBUTING.md](CONTRIBUTING.md) for setup and checks. The product is named Ripple; the repository and CLI use `ripple`.

M1 source discovery, TS/TSX parsing, supported symbol extraction, structural signature/body hashes and import/export/reference libraries are implemented. Lexical scopes retain shadowing and call ownership without resolving dependency targets. `inspect` exposes these facts as human output or versioned JSON, with explicit diagnostics and incomplete-analysis status. M2 now has a relative source-lookup/direct-binding library with explicit uncertainty. Config/re-export/call resolution, persistent indexing and Git impact commands remain upcoming.

```sh
cargo run --locked -p ripple-cli -- inspect src/caller.ts --root fixtures/inspect/project
cargo run --locked -p ripple-cli -- inspect src/view.tsx --root fixtures/inspect/project --format json
```

See [CLI usage](docs/cli.md) for paths, output and exit codes.

- [Current capabilities](docs/project-status.md): what works now, current CLI usage and the next implementation steps.
- [Build specification](ripple_buildspec.md): authoritative product scope and constraints.
- [Implementation plan](docs/implementation-plan.md): 43 dependency-linked tasks across nine milestones, including acceptance checks and handoffs.
- [Agent instructions](AGENTS.md): how to start, work, validate and report each task.
- [Execution protocol](docs/planning/execution-protocol.md): asynchronous coordination, implementation defaults, recovery and integration rules.
- [Task ledger](docs/planning/tasks.json): current task state, dependencies, ownership and validation evidence.
- [Completion report template](docs/task-reports/TEMPLATE.md): required short summary after every task.

Have the coordinator read the agent instructions and assign the next ready task from `python3 scripts/plan.py ready`. The task ledger records verified completion and the next ready task. The coordinator can be an agent; routine tasks and milestone checks do not require owner intervention.

Check task readiness with `python3 scripts/plan.py validate` and `python3 scripts/plan.py ready`. The helper validates planning state; it does not start agents.
