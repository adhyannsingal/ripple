# What Ripple does today

Ripple is being built as a local Rust tool that explains which TypeScript code and tests may be affected by Git changes, with dependency paths showing why. The current implementation provides the source-analysis foundation. End-to-end impact analysis is upcoming.

## Implemented and tested

| Area | Current behavior |
|---|---|
| Workspace and coordination | Seven Rust crates, pinned dependencies/toolchain, CI on macOS/Linux/Windows, dependency-linked task cards and resumable task reports. The planning helper validates state and lists ready tasks; it does not launch agents. |
| Repository discovery | Captures supported `.ts`/`.tsx` source and root configuration/ignore bytes, applies repository ignores and mandatory exclusions, identifies test filename suffixes and sorts portable paths deterministically. Anchored directory reads, resource bounds and explicit diagnostics guard unsupported paths and symlinks. |
| Parsing | Tree-sitter TS/TSX syntax trees, UTF-8 byte/line/column spans, clean/recovered/failed states and bounded error reporting. Syntax is not proof of TypeScript semantics. |
| Symbols | Supported top-level functions, const-bound function/arrow expressions, classes and methods; overload groups; syntactic export metadata; stable semantic identities and enclosing-symbol ownership. Unsupported forms remain explicit diagnostics. |
| Fingerprints | Separate structural signature/body SHA-256 hashes. Ordinary comments/spacing are stable while ASI and meaningful literal/TSX changes remain observable. Supported method body edits do not change the class shape fingerprint. Unsupported export shapes withhold complete signatures. |
| Shared contracts | Validated source, syntax, graph/evidence, revision, index and canonical-state data structures. These are boundaries for later implementations; a graph contract does not mean graph traversal or persistent indexing is available. |

For example, changing `function f() { return 1; }` to `function f() { return 2; }` preserves its identity and syntactic signature hash but changes its body hash. Adding blank lines preserves identity and hashes. These library-level facts will feed future change/impact analysis; they do not establish semantic API equality or safe test skipping.

## Available CLI

```sh
cargo run --locked -p ripple-cli -- --help
cargo run --locked -p ripple-cli -- --version
```

The binary reports `ripple 0.1.0-dev`. Help and version are the only current commands. It cannot yet inspect/index a repository, analyze a Git diff, traverse affected symbols or select related tests. No Node runtime or hosted service is required by the binary.

## Progress and next steps

M0 contracts/bootstrap are complete. M1 discovery, parsing and supported symbol/hash extraction are implemented; M1 remains in progress until references/imports/exports and the inspect CLI are integrated and verified. The current review adds explicit uncertainty for asserted callable/class literal bindings without expanding supported syntax.

Next is **M1-T04**, extracting imports, exports, references and lexical ownership. **M1-T05** then exposes inspect output. Later milestones add semantic resolution and SQLite generations, compiler-oracle evaluation, Git base/head impact analysis, incremental/replay correctness, related-test evidence and measured release benchmarks.

No resolver accuracy, latency, incremental-equality or test-sufficiency results are claimed. See [limitations](limitations.md), the [task ledger](planning/tasks.json), [implementation plan](implementation-plan.md) and [latest extraction review](reviews/extraction-recheck-2026-10-30.md) for the exact boundaries and evidence.
