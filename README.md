# Ripple

**Understand the ripple effects of your TypeScript changes.**

Ripple is a local command-line tool that helps you answer: **“If I change this function, what else could be affected—and why?”** It connects supported TypeScript and TSX symbols, compares Git revisions, and traces potentially affected code and test files back to the changes that led to them.

Use it to explore dependencies, review a branch, or decide where to focus your testing. Reports include the dependency paths behind each result and make uncertainty visible.

**Status:** v0.4.0 developer preview. All six commands are implemented and tested. A public GitHub Release has not been published yet; build from source for the latest fixes. See [current capabilities](docs/project-status.md) and the [release checklist](docs/release-checklist.md) for verified candidate details.

## What Ripple helps you do

- **Explore a symbol:** find supported callers and downstream dependencies without manually following imports.
- **Review a change:** compare two commits and explain impact using both the old and new code, including removed relationships.
- **Find potentially related tests:** distinguish evidence tied to a particular symbol from broader file-import relationships.
- **Reuse local analysis:** refresh an on-disk index while reusing unchanged analysis where the correctness checks allow it.
- **Inspect the evidence:** read human-friendly output or request structured JSON for scripts and developer tools.

## A concrete example

Suppose a checkout project has this dependency path:

```text
pricing.ts::total → checkout.ts::checkout → checkout.test.ts
```

The arrows show downstream impact: `checkout` calls `total`, and the test calls `checkout`. Another test, `shipping.test.ts`, imports a different function from `pricing.ts`; `unrelated.test.ts` uses a separate module.

When `total` changes, Ripple reports:

| Test file | Evidence |
|---|---|
| `checkout.test.ts` | Symbol-backed: a supported path connects the changed function to this test. |
| `shipping.test.ts` | Fallback-only: an import connects it to the changed file, without proving a dependency on `total`. |
| `unrelated.test.ts` | Not selected in this example. |

In our [reproducible small-project exercise](docs/examples/small-repo/README.md), all three tests passed initially. After a deliberate arithmetic regression, an independently run full suite failed only `checkout.test.ts`, which Ripple had selected with symbol evidence. The report still exposed unsupported constructs and incomplete analysis.

> Ripple reports potentially related test **files**. It does not run tests, and a selected set is never a guarantee that other tests can safely be skipped. Dynamic behavior and unsupported constructs can hide dependencies.

## Get started

### Build from source

You need Git supporting `--no-lazy-fetch`, the [pinned Rust toolchain](rust-toolchain.toml), and a C build toolchain. See [platform setup](CONTRIBUTING.md) for details.

```sh
git clone https://github.com/adhyannsingal/ripple.git
cd ripple
cargo build --release --locked -p ripple-cli
./target/release/ripple --help
```

On Windows, use `target\release\ripple.exe`. To install the command into Cargo's executable directory, run this from the checkout:

```sh
cargo install --locked --path crates/ripple-cli
ripple --version
```

The product runs locally with Git and bundled SQLite. It needs no Node.js runtime, database server, account, or cloud credentials. Node.js and TypeScript are used only by optional evaluation tooling.

### Try the disposable demo

After the release build above:

```sh
python3 docs/examples/quickstart.py --binary target/release/ripple
```

On Windows, use `python` and `target\release\ripple.exe`. Python is needed for this demo, not for the Ripple executable. The demo creates and removes its own Git project, exercises all six commands, and checks the symbol-backed and fallback-only test sets.

### Use it in your project

From a supported TypeScript Git repository root, add `.ripple/` to `.gitignore` before indexing:

```sh
# Capture the project and build or refresh its local index.
ripple index .

# Verify that the saved index still matches the current inputs.
ripple status

# Inspect supported syntax facts in a file.
ripple inspect src/pricing.ts

# Explain downstream impact from a specific symbol.
ripple impact src/pricing.ts::total

# Compare two existing committed revisions.
ripple diff BASE..HEAD

# Report potentially related test files with structured evidence.
ripple test BASE..HEAD --format json
```

Replace the example path, symbol, and `BASE` with ones that exist in your project. `BASE..HEAD` means an explicit two-endpoint comparison; it does not include uncommitted work. `diff` and `test` need no existing index and preserve the working tree and Git index.

If inputs change, run `ripple index .` again; stale `status` and `impact` queries fail with refresh guidance. Use `ripple index . --full` for a cold rebuild.

## Command reference

| Command | What it does |
|---|---|
| `ripple index .` | Build or incrementally refresh the persistent local index. |
| `ripple inspect FILE.ts` | Show supported syntax facts, symbols, imports, and diagnostics. |
| `ripple status` | Check index freshness and show provenance and analysis counts. |
| `ripple impact FILE.ts::symbol` | Explain downstream impact from a supported symbol. |
| `ripple diff A..B` | Classify committed changes and explain impact across both revisions. |
| `ripple test A..B` | Report potentially related test files, evidence, and selection ratios. |

`index` takes a positional root; the other commands accept `--root PATH`. `--format json` is available on every command. Impact, diff, and test also support `--depth` and `--limit`.

If a depth limit truncates traversal, downstream results can be omitted and the report marks that incompleteness. The display limit shortens human output while JSON retains the complete result within the requested depth. See the [full CLI guide](docs/cli.md) for flags and exit codes.

For automation, inspect `incomplete_analysis` and the detailed uncertainty fields as well as the exit code: a successfully produced report can still be incomplete. `test` JSON exposes separate symbol-backed, fallback-only, and combined test-file sets with witnesses.

## How it works

1. **Capture inputs:** read supported source, configuration, and ignore inputs within the project boundary.
2. **Build a graph:** extract syntax facts and resolve supported relationships, retaining confidence and unresolved boundaries.
3. **Locate changes:** compare Git endpoints and map changes to symbols or explicit file-level fallback.
4. **Explain impact:** follow reverse dependencies using both endpoint states and retain the paths behind the results.
5. **Keep local state consistent:** store complete SQLite generations atomically, verify freshness, and guard incremental reuse with cold-state equality tests.

[Architecture](docs/architecture.md) explains the design; [incremental indexing](docs/incremental.md) describes reuse, invalidation, and failure behavior.

## Supported scope and current limits

Ripple targets a **single-project Git root containing `.ts` and `.tsx` files**, with a documented subset of TypeScript configuration and resolution rules. It supports selected functions, classes/methods, imports, exports, and direct-call relationships. The default test-file suffixes are `.test.ts`, `.test.tsx`, `.spec.ts`, and `.spec.tsx`.

Coverage is intentionally bounded. Full TypeScript semantics, general receiver/dynamic dispatch, modern package-resolution modes, project references, and full monorepo behavior are not implemented comprehensively. Unknown relationships remain visible; finding no related tests does not prove there are none. Read [resolution support](docs/resolution.md) and [limitations](docs/limitations.md) before applying results to your project.

Index data stays under `.ripple/` in the analyzed repository. Its size depends on project size and indexing history; retained generations currently have no automatic garbage collection. Command-wide cancellation and index quota/retention controls remain open hardening work. Prebuilt candidate archives are roughly 4–5 MB compressed and 13–16 MB unpacked, excluding per-project indexes; building from source can create much larger Rust caches. The archived candidates predate the latest hardening changes—check their recorded source before using them.

## Verification and evidence

The latest recorded hardening/practical-validation batch passed **277 Rust tests, 37 Python tests, and 6 Node tests**, alongside Linux, macOS, and Windows CI. These are bounded checks, not a blanket correctness or security guarantee.

The evaluation includes compiler-oracle comparisons in declared supported projections, generated incremental-versus-cold comparisons, pinned Git-history replay, controlled runtime faults, and workflow benchmarks. The small mutation campaign caught 3 of 5 killed mutations with symbol-backed selection and 4 of 5 with the combined selection; known dynamic-import and global-state misses remain documented. Combined selection equalled file-import selection in that corpus.

- [Evaluation results and methodology](docs/evaluation.md)
- [Performance measurements and their limits](docs/benchmarks.md)
- [Practical project evidence and reproduction](docs/examples/small-repo/README.md)
- [Security review and remaining hardening work](docs/reviews/full-security-2026-11-05.md)

## Development and further reading

Start with [CONTRIBUTING.md](CONTRIBUTING.md) for setup and checks. [AGENTS.md](AGENTS.md), the [implementation plan](docs/implementation-plan.md), and the [task ledger](docs/planning/tasks.json) describe the development workflow and verified progress.

For the reasoning behind the project, read the [product thesis](docs/thesis.md), [graph and evidence model](docs/graph-model.md), [prior art](docs/prior-art.md), and [architecture decisions](docs/adr/0001-rust-core.md).

## License

Ripple is licensed under the [MIT License](LICENSE). Third-party dependencies and evaluation corpora retain their own licenses and notices.
