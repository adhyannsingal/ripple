# Ripple

A local Rust CLI in development that maps TypeScript Git changes to symbols and explains potentially affected code and test files through dependency paths.

M0 (contracts and bootstrap) is complete and reviewed: the workspace, CLI bootstrap, source/syntax/graph/evidence and index/revision/canonical contracts are implemented and tested. Run `cargo run --locked -p ripple-cli -- --help` or try `inspect` below. See [CONTRIBUTING.md](CONTRIBUTING.md) for setup and checks. The product is named Ripple; the repository and CLI use `ripple`.

M1 source understanding and the M2 cold-index pipeline are implemented. Ripple captures TS/TSX and supported configuration inputs, resolves supported lexical/imported calls and re-export chains, stores atomic SQLite generations, verifies freshness and explains reverse paths with separate symbol-backed/module-fallback evidence. Unsupported syntax/configuration and incomplete analysis remain explicit. M4 adds immutable `diff A..B` with base/head witnesses, explicit change/uncertainty records and test-presence partitions. M5 adds verified incremental parse/reference reuse, explicit rebuild reasons and `index --full`. M6 adds expanded generated equality checks, pinned history replay and repeated release update measurements. The v0.3.0-dev candidate keeps incomplete analysis explicit; dedicated test selection and mutation-safety evaluation remain M7–M8 work.

```sh
cargo install --path crates/ripple-cli --locked --root /path/to/install
# From a supported Git project root, using the installed binary:
ripple index .
# Repeat after edits to refresh; force a cold rebuild when needed:
ripple index . --full
ripple status
ripple impact src/auth.ts::authenticate --format json
ripple inspect src/auth.ts
ripple diff main..HEAD --format json
```

See [CLI usage](docs/cli.md) for paths, output and exit codes, and [incremental indexing](docs/incremental.md) for reuse proofs, counters and limits.

- [Current capabilities](docs/project-status.md): what works now, current CLI usage and the next implementation steps.
- [Build specification](ripple_buildspec.md): authoritative product scope and constraints.
- [Implementation plan](docs/implementation-plan.md): 43 dependency-linked tasks across nine milestones, including acceptance checks and handoffs.
- [Agent instructions](AGENTS.md): how to start, work, validate and report each task.
- [Execution protocol](docs/planning/execution-protocol.md): asynchronous coordination, implementation defaults, recovery and integration rules.
- [Task ledger](docs/planning/tasks.json): current task state, dependencies, ownership and validation evidence.
- [Completion report template](docs/task-reports/TEMPLATE.md): required short summary after every task.

Have the coordinator read the agent instructions and assign the next ready task from `python3 scripts/plan.py ready`. The task ledger records verified completion and the next ready task. The coordinator can be an agent; routine tasks and milestone checks do not require owner intervention.

Check task readiness with `python3 scripts/plan.py validate` and `python3 scripts/plan.py ready`. The helper validates planning state; it does not start agents.
