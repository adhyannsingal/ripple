# ADR 0019: Configured module origins, uncertainty and persistent evidence

Status: accepted and implemented in M2-T07.

## Boundary

`reexports::resolve_modules(project, sources, facts, config)` consumes captured, validated core facts and an evaluated EffectiveConfig. It composes configured source lookup with direct exports, named/star re-exports and imported-alias forwarding. The legacy `resolve()` API retains its relative/direct-only behavior. No IO, parser, compiler or Git handles enter production resolution; no CALLS/REFERENCES edges are emitted here. M2-T02 owns supported call resolution, and M2-T05 owns index/CLI composition and live capture verification.

The new artifact token is `ripple-modules-v1`, composing the existing config token/hash and project identity. Existing syntax/hash encodings and legacy resolver token are unchanged. New Deserialize implementations apply only to transport config/lookup records, never runtime EffectiveConfig.

## Fixed points and precedence

Represent a binding by repository file and exported name. Seed explicit local origins only when the existing direct-binding validator identifies a supported declaration. Imported-alias forwarding requires one module-root import binding and exactly one matching import fact; namespaces, shadowing, unsupported locals and incomplete facts remain unresolved. Named re-exports retain both alias names and original provider names. Explicit names override star contributions. Conflicting explicit declarations remain unresolved.

Compute monotone name/origin closure over sorted transfer relations, retaining every distinct original SymbolId and type-only contribution. Stars exclude default; explicit named/default re-exports can expose it. Multiple routes to the same declaration are one origin. Distinct origins remain ambiguous, including through downstream barrels. Runtime exposure wins over a type-only path to the same origin; type-only import/export restrictions still propagate to consumers.

Finish origin discovery before classifying empty cells. Then propagate missing, incomplete, ambiguous and unsupported evidence to a second fixed point. This prevents an empty cyclic named branch from leaving a false unique winner on another star branch. Anchored cycles converge; unanchored cycles stay unresolved. Missing/opaque star providers advertise an unknown name surface. A branch that could supply a missing competing name taints the result; an explicitly known provider name remains usable through another star barrel. Unknown surface flags stay visible even when particular explicit bindings are known. Unknown targets never become guessed symbol IDs.

Keep explicit syntactic type-only flags even where unsupported type declarations have no executable SymbolId. ExportBinding::Symbol carries effective type-only status; ImportedBinding.type_only is the conservative union of the importing syntax and provider exposure. Future call consumers must gate runtime edges on that combined flag.

## Witnesses, persistence and fingerprints

After convergence, reconstruct a finite deterministic shortest transfer witness to the actual declaration span using BFS over binding/type-only states. Preserve original source spans and transfer rules; imported records separately retain their import span. Transfers are module-binding provenance, not impact witness edges. Work/dependency tables are indexed by file/export key, with no SQL query per hop.

ModuleAnalysis serializes project/version/config provenance, all config dependencies/diagnostics, export/import results, candidate origins/reasons, transfers, source lookup probes and external fingerprints. JSON round trips retain this evidence for persistence handoff. Deserialized artifacts are untrusted transport; `validate(project, sources, facts, config)` recomputes and compares the complete analysis before reuse. It does not certify live freshness or revision origin. Actual SQLite/index-cache composition remains with the index/incremental integration tasks; this library writes no files.

External hash v1 is raw SHA-256 over compact ordered JSON containing its domain, module token, project, config hash, file, export names/syntactic flags, origin identities/type-only flags/origin signature hashes, uncertainty and configured lookup records. Full artifact witnesses retain spans; external fingerprints exclude body hashes and witness locations. They detect module-surface changes, not TypeScript semantic API equality. Missing/inferred signatures, side effects and unsupported language behavior still require conservative invalidation. These fingerprints do not authorize early cutoff; M5 must establish its generated cold-equality guards and fallback conditions first.

## Bounds and evidence

Reuse the source/fact/request inventory validation. Limit closure to 100000 cells/transfers, 512 rounds in either phase and five million charged work units, including witness work. Exhaustion is an error with no partial result. These are logical bounds, not total process memory/time quotas. Complete artifacts and witnesses may be large. Performance and compiler-oracle accuracy are not measured; those harnesses do not exist yet.

Nineteen integration tests use an independent multi-file expected-binding fixture and targeted cases for aliases, collisions/diamonds, defaults, type-only exposure, anchored/unanchored cycles, empty-branch and unknown-star taint, deletion/repair, unsupported policy, deterministic ordering, artifact tampering and bounded failure. Every successful binding is checked against original transfers and declaration facts. The unknown-star fixture caught and repaired overly broad taint during implementation.

The [ECMAScript ResolveExport algorithm](https://tc39.es/ecma262/2026/multipage/ecmascript-language-scripts-and-modules.html#sec-resolveexport) provides the named/star/default/ambiguity reference; [TypeScript type-only documentation](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-3-8.html#type-only-imports-and-export) supplies the type-only boundary. Ripple implements the documented subset, with conservative uncertainty and no compiler-equivalence claim.
