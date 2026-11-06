# Compiling small Git repository exercise

This owned disposable checkout-calculation project exercises every command and independently compiles/runs all three test files. Its baseline is green; one arithmetic regression must fail checkout.test.ts. Ripple should select that file with a symbol witness, retain shipping.test.ts as module fallback and omit unrelated.test.ts. It checks stale-state rejection, refresh, cold canonical equality and historical query preservation of staged/unstaged/index/database state.

From a source checkout, build the CLI and incremental helper in release mode, then run:

```bash
cargo build --release --locked -p ripple-cli
cargo build --release --locked -p ripple-index --example incremental_evaluate
python3 scripts/small-repo-smoke.py --binary target/release/ripple --helper target/release/examples/incremental_evaluate --node /path/to/node --typescript eval/oracle/node_modules/typescript --output /tmp/ripple-small-repo-new
```

Use the oracle's pinned Node/TypeScript installation. The harness executes only owned tests, with bounded subprocesses and a fresh private Git template/config. It never modifies a user repository. Ripple itself runs no tests. report.json retains inputs, hashes, independent test exits, selection witnesses and uncertainty; a finite happy-path fixture is not proof of full TypeScript semantics or safe omission of other tests. No external repository scripts run.

## Recorded November 6 exercise

[Raw practical report](report.json) pins source, executable hash, exact owned inputs, compiler/runtime versions, all independent exits and Ripple witnesses. The baseline passed all three tests. After changing multiplication to addition, only checkout.test.ts failed; symbol selection caught it, shipping.test.ts remained fallback-only and unrelated.test.ts was omitted. Stale-query rejection, full/incremental canonical equality and user Git/index/database preservation all passed. No selected set is claimed sufficient. The first trial required TypeScript 6's explicit --ignoreConfig for a file-list compile; diagnostics now retain both compiler output streams.

[Before](workflow-before.json) and [after](workflow-after.json) retain a five-sample warm workflow smoke comparison (one warm-up each). The before CLI is the original source-16eaf56 candidate; the after CLI includes the discovery budget. Both use the same newly built phase helper, so helper-phase differences are repeat noise, not a before/after scanner comparison. Source checkout metadata in the raw runner describes the harness checkout; each CLI's SHA256 distinguishes the executable actually measured. These are small sequential runs with uncontrolled desktop load, not a statistical performance guarantee or refreshed 20-sample final-release gate. An earlier exploratory before run overlapped compilation and is excluded here.

Cold-index p50: 364.24 → 370.32 ms; verified impact p50: 318.13 → 319.06 ms. No speedup or significant regression claim follows from five samples.

The exercise still reports `incomplete_analysis: true` and eight unresolved occurrences. Reasons retained in the raw report include unsupported constructor targets (the owned tests use `new Error`), missing supported lexical bindings and parameter-shadowed targets. The expected changed-function/test witness is present despite these unknowns. A successful command/fixture check does not mean the entire small program has complete semantic coverage.
