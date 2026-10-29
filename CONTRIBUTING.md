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
cargo run --locked -p ripple-cli -- --help
cargo run --locked -p ripple-cli -- --version
```

On Windows use `python` if `python3` is unavailable. On first build, rustup loads the pinned compiler/components and Cargo fetches the committed lockfile dependencies. CI defines the same checks for macOS, Linux and Windows; defining a job is not evidence that it has already passed remotely.

Install the current binary locally with `cargo install --locked --path crates/ripple-cli`. During bootstrap, only help/version are available. Analysis commands are implemented by later tasks and currently return usage errors.

## Dependencies and ownership

The coordinator owns the workspace manifest and `Cargo.lock`. Pin shared third-party versions centrally in `[workspace.dependencies]` and inherit them in member manifests. Commit the lockfile. Dependency changes must be deliberate, explained in an ADR when architectural, and verified on affected checks; never introduce parallel lockfile rewrites. See [the bootstrap dependency decision](docs/adr/0010-build-toolchain-and-dependencies.md).

Crates are unpublished local packages until a release task deliberately establishes publication/license metadata. Do not infer an open-source license from dependencies. Native dependency smoke tests validate grammar/API and SQLite compatibility; they do not claim syntax extraction or storage features exist.

Use your configured Git identity and focused commits. Do not add Codex attribution or rewrite human attribution. Link the real check results and task report in each handoff; preserve partial work and blockers rather than marking them done.
