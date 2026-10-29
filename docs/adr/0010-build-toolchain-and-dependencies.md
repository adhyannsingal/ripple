# ADR 0010: Bootstrap toolchain and dependency distribution

Status: accepted for M0-T01 (2026-10-28).

## Context

The Rust workspace needs reproducible builds on macOS, Linux and Windows before semantic implementation. The bootstrap host had no Rust installation. Compiler and native grammar/SQLite compatibility must be checked rather than assumed.

## Options

- Floating stable Rust and loose third-party ranges.
- A pinned compiler and exact shared direct versions plus committed lockfile.
- System SQLite versus bundled SQLite.

## Decision

Pin Rust 1.99.1 (edition 2024) with minimal profile, rustfmt and Clippy. Declare the same tested minimum in workspace metadata. Use resolver 3 and centrally pinned exact direct versions: clap 4.6.7, serde 1.0.229, serde_json 1.0.154, thiserror 2.0.21, Tree-sitter 0.27.1, TypeScript grammar 0.23.2, rusqlite 0.41.1. Use rusqlite's bundled SQLite feature. Preserve transitive resolution in Cargo.lock. Do not pre-add optional optimization/evaluation crates until their task needs them.

Crates depend only in the plan's allowed direction. CI uses official checkout/setup-python actions at verified release SHAs. Install Rust using the official rustup distribution; the bootstrap install used `--no-modify-path` so shell startup files stay unchanged.

## Reasoning

The official stable manifest and crates.io metadata were checked when selecting versions. An installed compiler, native grammar load tests for both TS/TSX and a bundled SQLite connection test verify this actual combination. A shared lockfile makes resolution reviewable. Bundling SQLite keeps distribution independent of a system DB library; a C toolchain remains a build requirement.

## Consequences

Only the declared compiler version is currently tested; older compatibility is not claimed. Native builds cost more compile time. Updates require rerunning checks. Node remains an evaluation-only future dependency. Packages are `publish = false`; project licensing/public publishing is deferred rather than guessed. Remote CI results must be recorded after a workflow run; local macOS results alone cannot prove Windows/Linux support.

## Revisit condition

Revisit on an upstream security/compatibility issue, needed API change, measured distribution burden, or an intentional supported-toolchain/platform policy change.

## Primary references

- [Rust installation](https://rust-lang.org/tools/install/) and [rustup profiles](https://rust-lang.github.io/rustup/concepts/profiles.html).
- [Official stable manifest](https://static.rust-lang.org/dist/channel-rust-stable.toml) (queried 2026-10-28; manifest date 2026-10-15).
- [Tree-sitter Rust crate](https://crates.io/crates/tree-sitter/0.27.1), [TypeScript grammar](https://crates.io/crates/tree-sitter-typescript/0.23.2), and [rusqlite](https://crates.io/crates/rusqlite/0.41.1).
- [Official checkout v7.0.1](https://github.com/actions/checkout/releases/tag/v7.0.1) and [setup-python v7.1.0](https://github.com/actions/setup-python/releases/tag/v7.1.0).
