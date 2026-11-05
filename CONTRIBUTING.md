# Contributing to Ripple

Read [AGENTS.md](AGENTS.md), the [build spec](ripple_buildspec.md), and your [task card](docs/implementation-plan.md) before changes. Claim tasks through the coordinator and preserve syntax/semantics/impact separation. Each task needs a short report under `docs/task-reports/`.

## Development environment

Install [Rust through rustup](https://rust-lang.org/tools/install/) and Git. The workspace pins Rust **1.99.1**, edition 2024, with rustfmt and Clippy in `rust-toolchain.toml`. This is the currently tested minimum; earlier compilers are not promised. If `cargo` is not on PATH after installation, load the rustup environment for your shell (`. "$HOME/.cargo/env"` for sh/zsh) or use its full path. No shell startup files need to be changed.

Use Python **3.9+** for the stdlib planning helper; CI exercises Python 3.14. No Python or Node runtime is required by the Rust binary. Native grammar and bundled SQLite compilation needs a C toolchain: Xcode Command Line Tools on macOS, a C compiler/linker on Linux, or Visual Studio C++ Build Tools with the Windows SDK on Windows. Bundled SQLite removes the need for a separately installed SQLite library/server.

## Local checks

```bash
python3 scripts/plan.py validate
python3 -m unittest discover -s scripts/tests -v
cargo fmt --all -- --check
cargo build --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python3 scripts/m7-smoke.py --binary target/debug/ripple
cargo run --locked -p ripple-cli -- --help
cargo run --locked -p ripple-cli -- --version
cargo run --locked -p ripple-cli -- inspect src/caller.ts --root fixtures/inspect/project
cargo run --locked -p ripple-cli -- inspect src/view.tsx --root fixtures/inspect/project --format json
```

On Windows use `python` if `python3` is unavailable. On first build, rustup loads the pinned compiler/components and Cargo fetches the committed lockfile dependencies. CI defines the same checks for macOS, Linux and Windows; defining a job is not evidence that it has already passed remotely.

Install the current binary locally with `cargo install --locked --path crates/ripple-cli`. Implemented commands are `index` (incremental by default, `--full` for cold), `inspect`, `status`, `impact`, `diff` and `test`, plus help/version. See [CLI usage](docs/cli.md) and [incremental behavior](docs/incremental.md). CI installs a fresh release binary and exercises all implemented commands.

For dependency and credential checks, run `cargo audit` with a current RustSec database, `npm audit --prefix eval/oracle`, and `gitleaks git --redact=100 --log-opts=--all .` after installing those optional audit tools. These supplement the checks above; report tool/database versions and actual results. They are not currently CI gates. Reuse a single `CARGO_TARGET_DIR` across sequential task worktrees to avoid duplicating build caches; never share it between concurrent Cargo mutations.

## Dependencies and ownership

The coordinator owns the workspace manifest and `Cargo.lock`. Pin shared third-party versions centrally in `[workspace.dependencies]` and inherit them in member manifests. Commit the lockfile. Dependency changes must be deliberate, explained in an ADR when architectural, and verified on affected checks; never introduce parallel lockfile rewrites. See [the bootstrap dependency decision](docs/adr/0010-build-toolchain-and-dependencies.md).

The owner selected the project MIT license (see LICENSE). Crates remain unpublished local packages; binary candidate preparation is separate from public tag/package publication. Upstream dependency/corpus notices retain their own licensing. Native dependency smoke tests validate grammar/API and SQLite compatibility; they do not claim syntax extraction or storage features exist.

Use your configured Git identity and focused commits. Do not add Codex attribution or rewrite human attribution. Link the real check results and task report in each handoff; preserve partial work and blockers rather than marking them done.

## Evaluation and release handoff

The product CLI needs no Node. Evaluation uses pinned Node 24.22.1 / TypeScript 6.0.3 from `eval/oracle/package-lock.json`; install with `npm ci --prefix eval/oracle --ignore-scripts`. See [mutation methodology](eval/mutation/README.md), [replay](eval/commit-replay/README.md), [workflow measurements](bench/results/workflows/README.md) and [oracle](eval/oracle/README.md) for actual reproduction commands and bounds. Corpus scripts are never executed by routine indexing/replay. The owned mutation suite executes only its reviewed fixture tests.

`python3 docs/examples/quickstart.py --binary /path/to/ripple` exercises the six commands in a disposable project. M8-T04 prepares checksummed archives and fresh-runner smoke evidence. Public publication is a separate authorized action; keep `publish = false` until that scope is deliberately changed. SQLite is bundled; distribution/runtime dependencies are audited per supported target.
