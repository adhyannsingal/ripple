# Ripple architecture

Implemented at M0-T02: a buildable workspace and backend-independent source/identity/syntax contracts. Parsing/extraction, resolution, storage, Git comparisons and impact analysis remain future implementation tasks; do not infer feature support from the types below.

## Boundaries

| Crate | Responsibility | Internal dependencies |
|---|---|---|
| ripple-core | Shared source, identity, syntax, graph and result contracts | none |
| ripple-parse | Capture-facing scanner, TS/TSX parsing and syntax facts | core |
| ripple-resolve | Supported module/export/call resolution and uncertainty | core |
| ripple-index | Cold/incremental orchestration and atomic SQLite generations | core, parse, resolve |
| ripple-git | Immutable refs, tree/blob sources and structured changes | core |
| ripple-impact | Graph queries, changed-symbol comparison and witnesses | core |
| ripple-cli | Compose providers/engines and render outputs | all as needed |

Core currently uses only serde/thiserror. It has no Tree-sitter, SQLite, filesystem or Git dependencies. Syntax facts expose lexical references and bindings, never resolved edges. M0-T04 defines graph/evidence contracts; M0-T05 defines generation/revision contracts. Implementations live in their owning crates rather than leaking into core.

## Source and identity

`RepoPath` normalizes slash separators, redundant separators and `.` components while rejecting absolute/drive/UNC paths, parent traversal and control characters. Preserve case and Unicode spelling; do not derive identity from machine-specific absolute paths. Filesystem collision/symlink checks belong to scanning. Host filenames containing literal backslashes are outside this portable path contract and must be diagnosed by a scanner rather than accidentally conflated.

`SourceSnapshot` owns immutable captured UTF-8 text keyed by normalized paths in sorted order. Providers capture once through `SourceProvider`; the index layer will verify the original inputs before publication and bound retries. Core captures no filesystem freshness guarantee. Non-UTF-8 inputs are rejected/diagnosed at provider boundaries; core `SourceFile` accepts valid Rust strings only.

`SourceSpan` uses half-open UTF-8 byte offsets and one-based display line/byte-column coordinates. LF advances lines; CRLF counts as one newline with both bytes retained. EOF/empty spans are representable; interior UTF-8 offsets are rejected. Validate deserialized facts against their captured source, not just structurally against serialized coordinates. Bare CR line endings are not normalized into LF silently.

`SymbolId` has project key, path, kind, structured qualified-name components and a disambiguator. Use persisted `default` project key for initial single-project support. Line/byte spans and revisions are absent from identity. `semantic_key()` is versioned and byte-length-framed; dotted display names are not unique identifiers. A method literally named `a.b` cannot collide with two qualified-name components. Disambiguators describe stable declaration roles/signatures, never byte positions or line numbers.

## Syntax facts and planned declaration support

`FileFacts` holds symbols, imports/exports, lexical scopes, bindings, references and diagnostics. Validate paths, coordinates, owner membership, lexical parent containment/cycles, hash shape and incomplete-parse diagnostics before downstream use. Recovered syntax may emit validated partial facts with explicit diagnostics; failed parsing emits diagnostics without syntax facts. Validation does not prove that extraction or resolution is correct.

The M1 extraction target includes named top-level function declarations, named classes/methods, named/default exports and top-level const bindings to arrow/function expressions, represented as FUNCTION nodes. Support anonymous default declarations using stable default-export roles. Treat overload signatures with their implementation as one logical symbol and hash the public signature group. Ambient-only declarations and unsupported anonymous/computed cases are diagnosed instead of fabricated as executable targets. No additional symbol kinds are introduced for callbacks initially.

References inside nested callbacks retain lexical scope and belong to their closest supported enclosing symbol; top-level test callbacks belong to the file. Record parameter/local/import bindings to let resolution detect shadowing. Module-level direct calls retain file ownership. Namespace/computed/dynamic constructs retain syntax evidence plus unsupported diagnostics when precise resolution is unavailable.

No semantic hashes are computed at M0-T02. `VersionedHash` is a versioned 256-bit lowercase hex representation; M1-T03 chooses/documents the algorithm and structure-aware hash encoding. Signature/body fields are optional until extraction, and their presence/meaning is not inferred from type construction.

## Validation and execution

Contract tests use manually authored facts, not parser-generated expected output. Every task has its own report and integrated commit in the ledger. See [the execution protocol](planning/execution-protocol.md) for claims, integration, uncertainty, cold-equality gates and version-specific completion requirements.
