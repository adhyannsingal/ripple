# ADR 0008: Pinned compiler API and independent comparable edges

Status: implemented by M3-T01.

Use the TypeScript 6.0.3 classic compiler API, locked npm integrity and Node 24.22.1 for evaluation only. TypeScript 7's package exposes a different native API; intentionally pin the classic API rather than silently changing semantics. No Node dependency enters the Rust CLI. See [compiler API documentation](https://github.com/microsoft/TypeScript/wiki/Using-the-Compiler-API).

AST/definition eligibility precedes Ripple analysis and includes every supported oracle edge, even when Ripple has no corresponding symbol/edge. Use exact UTF-8 declaration spans and nearest supported owners; convert compiler UTF-16 explicitly, group overload implementations, deduplicate pairs and retain occurrence provenance. Mapping exclusions, unsupported syntax and actual resolver misses are separate outputs. Receiver ambiguity is excluded independently; Ripple success never filters the denominator.

The fixed source-projection policy, options, eligible owner/target rules and limits are versioned in [oracle README](../../eval/oracle/README.md). Mini fixtures supply hand-checked aliases/re-exports/overloads/TSX and byte-coordinate tests. The external source projection has immutable URL/SHA/content digests/license metadata; upstream config/package/tests are deliberately excluded, disclosed and never executed. No claim of full upstream project compatibility follows.

Later policy/compiler/corpus changes must create a new comparable baseline. Precision/recall use deduplicated semantic edges, report raw numerators/denominators and undefined empty cases. Candidate coverage, unresolved counts and mapping loss are separate from resolved-edge accuracy. Performance and compiler accuracy outside this narrow subset remain unmeasured.
