# Compiling small Git repository exercise

This owned disposable checkout-calculation project exercises every command and independently compiles/runs all three test files. Its baseline is green; one arithmetic regression must fail checkout.test.ts. Ripple should select that file with a symbol witness, retain shipping.test.ts as module fallback and omit unrelated.test.ts. It checks stale-state rejection, refresh, cold canonical equality and historical query preservation of staged/unstaged/index/database state.

From a source checkout, build the CLI and incremental helper in release mode, then run:

```bash
cargo build --release --locked -p ripple-cli
cargo build --release --locked -p ripple-index --example incremental_evaluate
python3 scripts/small-repo-smoke.py --binary target/release/ripple --helper target/release/examples/incremental_evaluate --node /path/to/node --typescript eval/oracle/node_modules/typescript --output /tmp/ripple-small-repo-new
```

Use the oracle's pinned Node/TypeScript installation. The harness executes only owned tests, with bounded subprocesses and a fresh private Git template/config. It never modifies a user repository. Ripple itself runs no tests. report.json retains inputs, hashes, independent test exits, selection witnesses and uncertainty; a finite happy-path fixture is not proof of full TypeScript semantics or safe omission of other tests. No external repository scripts run.
