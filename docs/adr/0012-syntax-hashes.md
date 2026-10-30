# ADR 0012: Supported declaration extraction and structural syntax hashes

Status: accepted (M1-T03).

## Decision

`ripple-parse::symbols::extract` consumes an immutable ParsedSource and ProjectKey and returns validated, sorted **symbol-only** FileFacts. Imports, ExportFact records, references and lexical scopes are still empty until M1-T04. Syntax extraction never emits resolved graph edges or proves TypeScript validity.

Support top-level named function declarations (including async/generators), classes, class methods and const identifier bindings directly to arrow/function expressions. Plain parentheses/comments around initializers/default expressions are supported. Named/anonymous default function/class/arrow exports and local named/default export clauses supply syntactic exposure metadata; re-exports do not mark unrelated local declarations exported. Type-only exposure affects signature metadata but not runtime `exported`. Public methods inherit their class's exposure; private/protected/# methods are not marked exported. This is syntactic visibility, not a resolver guarantee.

Keep the existing project/path/qualified-name/kind identities. Anonymous default declarations use `default` plus disambiguator `default-export`. Static/get/set method roles use stable disambiguators (`static`, `get`, `set`, or combinations), never offsets. Simple quoted method names are individual qualified-name components. Computed/numeric/escaped names are diagnosed rather than guessed. Contiguous overload signatures preceding exactly one implementation form one symbol; hash all headers, including the implementation header conservatively. Duplicate implementation groups and ambient-only targets are omitted with diagnostics. Methods in different classes stay distinct.

Function/method signature spans cover headers (including an overload group); body spans identify the implementation body/expression. A class signature span is its header, while its signature hash also includes member shape. `owner_for_span` picks the smallest enclosing supported symbol, so nested callbacks/functions belong to their enclosing function/method and module-level callbacks remain file-owned.

## Hash format v1

Use [sha2 0.11.1](https://docs.rs/sha2/0.11.1/sha2/) SHA-256 with lowercase hex digests and SYNTAX_HASH_VERSION=1. Each field is encoded as an unsigned **u64 big-endian byte length followed by its bytes**. Numeric field payloads are big-endian. The prefix fields are:

1. `ripple.syntax`, u32 version, domain (`signature`, `body`, `class_shape`), language (`ts`/`tsx`).
2. u64 metadata count and metadata strings, then u64 root count. Signature metadata contains sorted export-role strings followed by the static/get/set role string; body/class-shape metadata is empty.
3. Each root: `tree`, structural node frames, `/tree`.

Node frames contain `node`, parent field name (empty for each root), node kind, named/token flag (one byte), then `children` and child frames or `leaf`, restricted-line-break flag (one byte) and exact UTF-8 leaf text; terminate with `/node`. No source positions, absolute paths, revisions, node addresses or storage IDs enter the bytes. Node IDs/ranges are only used to select omitted bodies.

Ordinary comments and explicit semicolon tokens are omitted; empty-statement nodes remain observable. Other punctuation/structure and meaningful strings, regexes, templates and TSX text are retained. This is conservative syntax normalization, not equivalence of every quote style, parentheses, trailing comma or type spelling.

The pinned grammar does not always represent restricted line breaks inside comments. Add a leaf boundary flag for LF/CR/U+2028/U+2029 after return/throw/yield/break/continue/async within the same construct and before postfix ++/-- within an update expression. This preserves ASI-sensitive changes while ignoring harmless multiline comments. The return-comment regression failed before this encoding; the independent matrix covers it and related cases.

Exclude implementation bodies from function/method signatures. Class hashes exclude only bodies represented by supported method symbols. Unsupported/computed/duplicate member bodies, fields and static initialization stay in the class fingerprint so edits cannot disappear; diagnostics explain the fallback. A supported method body edit changes its method hash, not the class shape.

## Uncertainty and compatibility

Recovered declarations with error nodes are omitted. A failed parse produces no symbols. Recovered files, CommonJS/namespace exports, other unmodeled export forms/spellings and directive-like compiler comments withhold complete signature hashes; clean body evidence may remain. Decorators, ambient/type/namespace targets, non-const bindings, computed names and unsupported class initialization remain explicit diagnostics. Traversal is bounded to 1000000 nodes before declaration collection; at most 1024 detailed combined diagnostics are retained plus explicit truncation markers. Traversal uses cursors rather than recursive AST walking.

These hashes do **not** prove compiler API equality: inferred returns, imported/aliased types and runtime/compiler transformations require conservative downstream invalidation. M5 must not enable early cutoff solely from signature equality. Raw source/config input hashing is a separate producer responsibility. Use EXTRACTOR_VERSION (`ripple-symbols-v2-ts0.23.2-runtime0.27.1`) in analyzer compatibility; grammar, extraction or encoding changes require an intentional compatibility decision rather than reusing the package's development version blindly.

M1-T07 advances the extractor token from v1 to v2 because assertion/non-null wrappers around function/class literal variable initializers now produce explicit unsupported-binding diagnostics. These remain unsupported symbol targets. If directly exported or exposed through a local export clause, their unmodeled export shape withholds complete signature hashes for the file. The hash byte format remains v1 and its independent digest oracle is unchanged. Scalar assertions and factory calls are not classified as literal callable bindings.

## Evidence and follow-ups

14 independently authored declaration fixtures, 24 hash-change pairs, a hand-authored CST/SHA-256 encoding oracle, identity/ownership/round-trip tests and malformed/unsupported cases define this boundary. Performance and resolver/oracle accuracy are not measured here. M1-T04 extends the facts; M4/M5 consume comparisons under their own uncertainty and correctness gates.
