# Ripple

Ripple is a local Rust CLI for explainable TypeScript change impact. It maps Git changes to supported symbols, follows reverse dependencies, and reports potentially related code and test files with base/head witness paths, confidence and explicit uncertainty.

The six commands are `index`, `inspect`, `status`, `impact`, `diff` and `test`. Indexing persists atomic SQLite generations and reuses unchanged analysis behind content/version/equality guards. Diff/test analyze immutable endpoints without checkout or index publication. Test selection is a conservative union of symbol-backed and file-import fallback evidence; it never executes tests or guarantees a sufficient set.

## Install and try

Install Rust with a C toolchain and Git supporting `--no-lazy-fetch`; [setup and supported hosts](CONTRIBUTING.md). SQLite is bundled; the CLI needs no Node, database server, account or project scripts.

```sh
cargo install --locked --path crates/ripple-cli --root /path/to/install
python3 docs/examples/quickstart.py --binary /path/to/install/bin/ripple
```

Windows uses `python` and `ripple.exe`. The demo creates a disposable Git project, exercises every command and checks one symbol-backed test plus one fallback-only addition. In a supported TypeScript Git project:

```sh
ripple index .
ripple inspect lib.ts
ripple status
ripple impact lib.ts::f --format json
ripple diff BASE..HEAD --format json
ripple test BASE..HEAD --format json
ripple index . --full
```

Use existing explicit refs for BASE/HEAD and a real supported symbol/path. [CLI reference](docs/cli.md) describes flags, output and failure semantics.

## Evidence and current limits

[Evaluation](docs/evaluation.md) links narrow compiler-oracle agreement, incremental/cold equality, pinned history replay and controlled mutation results. In the deliberately small mutation corpus, symbol-backed selection caught 3/5 failing files and combined caught 4/5, equal to the independent static file-import baseline. Dynamic imports/global state expose misses. These are finite observations, not general test safety or accuracy estimates.

[Performance](docs/benchmarks.md) separates verified CLI cost from cached graph traversal and records raw samples/hardware/state. The profile rejected a timestamp-only freshness shortcut after a same-size restored-mtime edit defeated it. No unmeasured optimization gain is claimed.

Scope: one Git root, TS/TSX and a [narrow documented config/resolution subset](docs/resolution.md). Modern package modes, project references, receiver/dynamic dispatch and many constructs remain diagnosed. Index archives have no GC. [Limitations](docs/limitations.md) and JSON completeness fields must be read before using selections. Prior work is acknowledged in [prior art](docs/prior-art.md).

[Architecture](docs/architecture.md), [thesis](docs/thesis.md), [graph model](docs/graph-model.md), [incremental policy](docs/incremental.md), [ADRs](docs/adr/0001-rust-core.md), [task ledger](docs/planning/tasks.json) and [agent protocol](AGENTS.md) provide the contributor handoff. The ledger records verified progress; it is not a background scheduler.

Licensed under [MIT](LICENSE). Upstream corpus notices remain separate. Versioned candidate packaging and final release checks are the remaining M8-T04 handoff; no public release is implied by this source checkout.
