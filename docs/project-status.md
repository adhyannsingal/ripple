# What Ripple does today

Ripple is being built as a local Rust tool that explains which TypeScript code and tests may be affected by Git changes, with dependency paths showing why. The current implementation provides the source-analysis foundation. End-to-end impact analysis is upcoming.

## Implemented and tested

| Area | Current behavior |
|---|---|
| Workspace and coordination | Seven Rust crates, pinned dependencies/toolchain, CI on macOS/Linux/Windows, dependency-linked task cards and resumable task reports. The planning helper validates state and lists ready tasks; it does not launch agents. |
| Repository discovery | Captures supported `.ts`/`.tsx` source and root configuration/ignore bytes, applies repository ignores and mandatory exclusions, identifies test filename suffixes and sorts portable paths deterministically. Anchored directory reads, resource bounds and explicit diagnostics guard unsupported paths and symlinks. |
| Parsing | Tree-sitter TS/TSX syntax trees, UTF-8 byte/line/column spans, clean/recovered/failed states and bounded error reporting. Syntax is not proof of TypeScript semantics. |
| Symbols | Supported top-level functions, const-bound function/arrow expressions, classes and methods; overload groups; syntactic export metadata; stable semantic identities and enclosing-symbol ownership. Unsupported forms remain explicit diagnostics. |
| Imports, exports and references | Unescaped static module specifiers, named/default aliases, type-only bindings and basic re-exports; direct calls, runtime uses, lexical scopes/shadowing and symbol/file ownership. Dynamic/receiver/unsupported forms retain diagnostics; targets remain unresolved. |
| Fingerprints | Separate structural signature/body SHA-256 hashes. Ordinary comments/spacing are stable while ASI and meaningful literal/TSX changes remain observable. Supported method body edits do not change the class shape fingerprint. Unsupported export shapes withhold complete signatures. |
| Inspect CLI | Captures a selected root and displays one discovered TS/TSX file as deterministic human or versioned JSON facts, with source spans, ownership and explicit diagnostics/incompleteness. No index or Git state is written. |
| Shared contracts | Validated source, syntax, graph/evidence, revision, index and canonical-state data structures. These are boundaries for later implementations; a graph contract does not mean graph traversal or persistent indexing is available. |

For example, changing `function f() { return 1; }` to `function f() { return 2; }` preserves its identity and syntactic signature hash but changes its body hash. Adding blank lines preserves identity and hashes. These library-level facts will feed future change/impact analysis; they do not establish semantic API equality or safe test skipping.

## Available CLI

```sh
cargo run --locked -p ripple-cli -- --help
cargo run --locked -p ripple-cli -- --version
cargo run --locked -p ripple-cli -- inspect src/caller.ts --root fixtures/inspect/project
cargo run --locked -p ripple-cli -- inspect src/view.tsx --root fixtures/inspect/project --format json
```

The binary reports `ripple 0.1.0-dev`. `inspect` now exposes source facts without resolving dependency targets. It cannot yet build a persistent index, analyze a Git diff, traverse affected symbols or select related tests. See [CLI usage](cli.md). No Node runtime or hosted service is required by the binary.

## Progress and next steps

M0 contracts/bootstrap and the M1 source-understanding gate are complete. M1 discovery, parsing, supported symbols/hashes, import/export/reference extraction and the inspect CLI are integrated and verified. See the [M1 gate report](task-reports/M1.md) for evidence. Extraction keeps opaque syntax, type-only imports, lexical shadowing and uncertainty explicit.

Next is **M2-T01**, resolving relative modules and direct export bindings. Later milestones add semantic resolution and SQLite generations, compiler-oracle evaluation, Git base/head impact analysis, incremental/replay correctness, related-test evidence and measured release benchmarks.

No resolver accuracy, latency, incremental-equality or test-sufficiency results are claimed. See [limitations](limitations.md), the [task ledger](planning/tasks.json), [implementation plan](implementation-plan.md), the [reference extraction report](task-reports/M1-T04.md) and [extraction review](reviews/extraction-recheck-2026-10-30.md) for the exact boundaries and evidence.
