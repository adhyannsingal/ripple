# Ripple architecture

Implemented through M0-T05: a buildable workspace and backend-independent source/identity/syntax, graph/evidence and index/revision/canonical data contracts. M1 now supplies deterministic source discovery, bounded TS/TSX parsing, supported symbol extraction and structural signature/body hashes. Import/export/reference and lexical-scope extraction are now implemented; resolution, storage, Git comparisons and impact analysis remain future implementation tasks; do not infer feature support from the types below.

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

Core currently uses only serde/thiserror. It has no Tree-sitter, SQLite, filesystem or Git dependencies. Syntax facts expose lexical references and bindings, never resolved edges. M0-T04 defines the implemented graph/evidence contracts; M0-T05 defines the implemented generation/revision contracts. Implementations live in their owning crates rather than leaking into core.

## Source and identity

`RepoPath` normalizes slash separators, redundant separators and `.` components while rejecting absolute/drive/UNC paths, parent traversal and control characters. Preserve case and Unicode spelling; do not derive identity from machine-specific absolute paths. Scanning rejects portable case collisions and skips encountered symlinks. Worktree IO is anchored to an open directory handle, with capability-relative paths and regular-file checks; see [ADR 0011](adr/0011-source-discovery.md). Host filenames containing literal backslashes are outside this portable path contract and must be diagnosed by a scanner rather than accidentally conflated.

`SourceSnapshot` owns immutable captured UTF-8 text keyed by normalized paths in sorted order. Providers capture once through `SourceProvider`; the index layer will verify the original inputs before publication and bound retries. Core captures no filesystem freshness guarantee. Non-UTF-8 inputs are rejected/diagnosed at provider boundaries; core `SourceFile` accepts valid Rust strings only.

`SourceSpan` uses half-open UTF-8 byte offsets and one-based display line/byte-column coordinates. LF advances lines; CRLF counts as one newline with both bytes retained. EOF/empty spans are representable; interior UTF-8 offsets are rejected. Validate deserialized facts against their captured source, not just structurally against serialized coordinates. Bare CR line endings are not normalized into LF silently.

`SymbolId` has project key, path, kind, structured qualified-name components and a disambiguator. Use persisted `default` project key for initial single-project support. Line/byte spans and revisions are absent from identity. `semantic_key()` is versioned and byte-length-framed; dotted display names are not unique identifiers. A method literally named `a.b` cannot collide with two qualified-name components. Disambiguators describe stable declaration roles/signatures, never byte positions or line numbers.

## Syntax facts and supported declaration subset

`FileFacts` holds symbols, imports/exports, lexical scopes, bindings, references and diagnostics. Validate paths, coordinates, owner membership, lexical parent containment/cycles, hash shape and incomplete-parse diagnostics before downstream use. Recovered syntax may emit validated partial facts with explicit diagnostics; failed parsing emits diagnostics without syntax facts. Validation does not prove that extraction or resolution is correct.

`ripple-parse::symbols::extract` now supports named top-level function declarations, named classes/methods, named/default exports and top-level const bindings to arrow/function expressions, represented as FUNCTION nodes. Anonymous default declarations use stable default-export roles. Contiguous overload signatures preceding one implementation form one logical symbol; fingerprints retain all headers conservatively. Static/get/set roles distinguish methods with the same local name. Ambient-only declarations and unsupported anonymous/computed cases are diagnosed instead of fabricated as executable targets. No additional symbol kinds are introduced for callbacks initially.

References inside nested callbacks retain lexical scope and belong to their closest supported enclosing symbol; top-level test callbacks belong to the file. Record parameter/local/import bindings to let resolution detect shadowing. Module-level direct calls retain file ownership. Namespace/computed/dynamic constructs retain syntax evidence plus unsupported diagnostics when precise resolution is unavailable.

M0 contracts compute no hashes themselves. M1-T03 produces SHA-256 syntax fingerprints with versioned domains, length-framed node kinds/fields/leaf content and ASI-sensitive line-break evidence, as specified in [ADR 0012](adr/0012-syntax-hashes.md). Supported method bodies are omitted from class shape, while unrepresented class members retain full syntax conservatively. Hashes and semantic IDs ignore source line shifts. Signature hashes may be absent for recovered/unmodeled export input; their presence does not prove TypeScript compiler API equality or justify early cutoff. Raw source/config digest production remains separate.

`ripple_parse::extract` now returns supported symbol/import/export/reference/scope facts; `symbols::extract` remains the symbol-only API. References retain exact source spans, lexical parents and smallest enclosing symbol/file ownership. Binding-level type-only imports, shadowing and module-level callbacks stay explicit; no resolved targets or graph edges are emitted. Supplemental extraction has depth/scope/count/text budgets, with all supplemental facts withheld on a resource limit. See [ADR 0013](adr/0013-reference-extraction.md) for supported syntax, opaque boundaries, compatibility and limits. Unsupported names, bindings, ambient/type-only declarations, decorators and class initialization have diagnostics and documented limits.

## Graph, generations and revision binding

See [graph-model.md](graph-model.md) for typed endpoints and independent symbol-backed/module-fallback witnesses. Graph snapshots are sorted semantic records; the impact crate will build adjacency/BFS later. Every stored witness must validate against the graph for its actual revision.

`SnapshotData` is a serializable transport record, not an automatically trusted query index. `validate(captured_sources)` checks manifest membership/byte lengths, syntax facts, project identity, graph symbol/file membership, coordinates/provenance and hash-format shape. It returns a private, immutable `IndexSnapshot`; decoding persisted data must pass through that source validation again. Hash computation/verification is a producer responsibility and not accomplished by checking 64 hex characters. Config/ignore/package/lockfile bytes are fingerprinted as manifest inputs, but only supported UTF-8 source files enter SourceSnapshot/FileFacts.

SnapshotOrigin distinguishes a possibly dirty worktree with optional HEAD (including unborn repositories) from an immutable commit. RevisionGraphs binds Base/Head labels to matching full commit IDs and compatible project/formats, so deleted symbols remain explainable only in their base graph. It never treats a clean worktree as a committed-tree index. The real Git provider/diff mapper and old/head traversal remain future tasks.

`IndexVersions` separates SQLite schema, hash encoding and analyzer compatibility identifiers. Callers must compare against supported identifiers from the running analyzer with `ensure_compatible`; positive version numbers alone are not compatibility proof. Incompatible development builds must not share an unchanged package-version token. A complete generation has creation metadata/counter; SQLite implements actual transactions/locking/recovery later.

`check_observation` compares supplied input/HEAD observations to the captured manifest. It performs no filesystem access and does not create a freshness certificate. The index implementation must actually recapture/re-hash membership and inputs before publication, with at most three attempts, and preserve the previous generation on continued changes/failure. Files can change after verification; queries still need compatibility/freshness checks.

`CanonicalState` v1 preserves input hashes, effective-config semantics, facts, typed edges, confidence, rule/provenance, parse status and unknown/diagnostic records while normalizing order. It excludes generation numbers, times and actual checkout origin. Thus equal semantic state at different revisions can still fail freshness/revision checks. This comparison artifact cannot be loaded as an unchecked runtime index, and unsupported comparison format versions are rejected. SQLite row IDs never enter these contracts. M5/M6 extend fingerprints and test real incremental/cold updates, not just projection examples.

## Validation and execution

Contract tests use manually authored facts, not parser-generated expected output. Every task has its own report and integrated commit in the ledger. See [the execution protocol](planning/execution-protocol.md) for claims, integration, uncertainty, cold-equality gates and version-specific completion requirements.
