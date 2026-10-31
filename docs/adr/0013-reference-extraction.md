# ADR 0013: Syntax references with lexical provenance

Status: accepted (M1-T04).

## Decision and boundary

`ripple_parse::extract` (also `references::extract`) consumes a captured `ParsedSource` and `ProjectKey`, returning validated, deterministically ordered supported `FileFacts`. It composes the existing `symbols::extract` API, which remains symbol-only for callers that need that boundary. No core DTO or dependency changes are needed. No syntax fact contains a resolved target, confidence claim or graph edge. The resolver consumes these records rather than Tree-sitter trees.

## Imports and exports

Capture top-level static imports/exports only; nested module statements are diagnosed without publishing global module facts. Capture unescaped string specifiers without normalizing relative paths, aliases or package names into guessed modules. Named/default/alias imports retain imported and local names, source spans and binding-level `type_only`. Namespace imports retain a Namespace binding plus an unsupported diagnostic. Bare side-effect imports have no bindings. An empty type-only import is diagnosed and omitted so it cannot masquerade as a runtime side-effect import. Import attributes and import-equals forms are diagnosed; unsupported module strings do not produce guessed import facts. Recognizable local import bindings remain lexical evidence even when their module syntax is unsupported.

Local named/default exports and named/star re-exports retain source spans, aliases and type-only markers. Anonymous supported default function/class/arrow targets use local name `default`, consistent with their stable symbol role; named default targets retain their actual local name. Unescaped identifier export names are supported. Namespace re-exports, empty re-exports and arbitrary default expressions are diagnosed rather than fabricated as ordinary local/star exports. Empty re-exports still have module-loading behavior that a future resolver must treat as incomplete evidence. The pinned grammar recovers on `export type *`; do not call it supported. Named type-only re-exports are supported.

## References and ownership

Direct nonoptional identifier invocations produce Call facts, including plain parenthesized identifiers. Receiver, computed, optional, constructor, assertion-wrapped and eval/require calls produce ComputedCall evidence with unsupported-target diagnostics. Dynamic imports, including literal arguments, retain DynamicImport evidence and diagnostics; they are not static Import facts. Callee/index/argument subexpressions retain runtime identifier uses where applicable. Read denotes a syntactic runtime value use, not data-flow or definite-assignment proof. Types, declaration positions, property keys, static import/export clauses, intrinsic lowercase JSX tags and closing tags are excluded. JSX component identifiers retain Read evidence without proving a component call target.

Every reference carries exact source text/span and a lexical scope. Ownership is the smallest enclosing supported symbol, otherwise the file. Nested functions/callbacks add scopes and bindings, without inventing additional symbol targets. Top-level test callbacks remain file-owned. A source-ordered symbol-interval sweep implements ownership; generated tests compare it with the independent existing owner lookup.

## Lexical evidence and uncertainty

Create deterministic file-local ScopeIds with enclosing parents for functions/parameters, function bodies, blocks, classes/static blocks, catch clauses, loops and switch bodies. These IDs are transport-local, not semantic symbol identities. Record import, parameter, local, function and class bindings. Destructuring visits binding positions only, excluding property keys, types and default values. Named expressions bind internally; declarations also bind in their enclosing scope. Ordinary functions/methods add an implicit `arguments` binding anchored to the function span; arrows inherit it. Body `var` declarations hoist into the body environment or module, not into parameter defaults or outside a class static block. Block and parameter shadowing remains visible independently of declaration order; no TDZ execution or flow-sensitive lifetime claim is made.

Ambient/namespace bodies and dynamic `with` bodies are skipped with explicit diagnostics. Escaped names, decorators and opaque calls remain uncertainty boundaries. The future resolver must respect type-only imports, scope parents, declaration spans and unsupported diagnostics; a Call name matching an import is never sufficient proof of a dependency. Recovery/opaque scope diagnostics cannot be discarded to promote a target to RESOLVED.

## Bounds and compatibility

Symbol extraction first applies its existing one-million-node preflight. Supplemental extraction is iterative and bounded to named-tree depth 256, 8192 scopes, 100000 references and **8 MiB of aggregate reference text**. Reference strings are checked before allocation, preventing overlapping computed targets from multiplying captured text without a bound. Exceeding a supplemental budget discards imports/exports/scopes/references together and emits `reference_resource_limit`; existing validated symbols and diagnostics remain. Detailed diagnostics share the existing 1024-entry budget plus explicit truncation/resource markers. Resource-limit reasons survive warning truncation. Failed parsing emits no syntax facts; recovery emits only partial validated facts with diagnostics, omitting malformed imports/exports/calls.

The analyzer compatibility token becomes `ripple-facts-v3-ts0.23.2-runtime0.27.1` because complete FileFacts now include new extraction behavior. Symbol IDs and v1 hash byte encoding do not change. Rebuild incompatible cached analyzer output; do not infer compatibility from the unchanged development package version.

## Evidence and follow-ups

Eight independently specified TS/TSX fixture programs cover default/alias/type-only imports, local/barrel exports, circular module specifiers, shadowing, module-level test callbacks and dynamic boundaries. Behavioral tests cover lexical environments, provenance, round trips, generated ownership, unsupported/recovered cases and budgets. Overlapping-text and warning-cap/resource-marker regressions failed before their repairs. Performance and compiler-oracle accuracy are not measured; M1-T05 wires inspect, M2 resolves supported facts and M3 establishes oracle evidence.
