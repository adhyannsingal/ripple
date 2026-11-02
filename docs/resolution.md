# Current module, config and direct-binding resolution

These are library layers. `inspect` remains syntax-only with `resolution_available: false`; index/impact/status commands are not yet implemented. SQLite storage and reverse traversal now exist independently; see [storage](storage.md) and [queries](graph-model.md#query-library-m2-t04).

`ModuleResolver` and `resolve(project, sources, facts)` provide the M2-T01 relative lookup/direct-export/import-binding layer. It requires one validated fact record per captured source, produced from those bytes under a compatible analyzer. It retains type-only flags, opaque/recovered-provider uncertainty and ordered successful/missing lookup dependencies. Named/default aliases work; re-exports and imported-alias forwarding remain deferred in this legacy API. The configured M2-T07 API below expands that layer. `configuration_evaluated` remains false in that existing API. The production resolver now uses core, serde, serde_json and SHA-256, with no parser or Node runtime dependency.

## Effective config and alias API (M2-T06)

```rust,ignore
let captured = CapturedConfigs::new(config_inputs)?;
let config = captured.evaluate(&root_config_path)?;
let resolver = ConfiguredResolver::new(&config, source_inventory)?;
let lookup = resolver.lookup(&importer, specifier);
```

Import these types from `ripple_resolve::config` and `ripple_resolve::paths`. All configs/source paths are captured inputs supplied by the caller; these APIs perform no discovery or IO. Supply the finite relevant inheritance inventory before expecting a supported result. Missing inputs remain visible in `config.inputs()` and produce diagnostics. `CapturedConfigs::get` retrieves present bytes for manifest construction; freshness/retry is M2-T05 work.

| Supported policy | Behavior |
|---|---|
| JSONC | UTF-8/BOM, line/block comments and trailing commas; quoted strings preserved; duplicate decoded keys/malformed text rejected. |
| Inheritance | One relative local extends string; exact captured file then `.json`; cycles/depth/escapes explicit; child compiler properties override and paths replaces the entire parent map. |
| Origins | baseUrl is relative to its declaring config; inherited paths keep declaration origin when baseUrl is absent and use final effective baseUrl when present. |
| Alias precedence | Exact key, otherwise longest wildcard prefix; one wildcard/suffix supported; equal-priority wildcard matches are explicitly ambiguous. |
| Fallback | Chosen targets in array order, then legacy baseUrl for missing supported lookups; no less-specific alias or external-package fallback. |
| Source lookup | `.ts`, `.tsx`, `.d.ts` before directory indexes, exact source extensions and `.js` substitution, under Ripple's source policy. Relative requests are unaffected by aliases. |
| Evidence | Original request, config hash, routes/declaration origins, every missing source probe before the winner; present and missing config probes separately retained. |

`EffectiveConfig::supported()` reports absence of diagnosed unsupported configuration in this subset, not TypeScript compiler validity. Any config diagnostic prevents a configured lookup from returning Found, including for relative requests. Absent/opaque imports and unsupported extensions remain explicit. Found means an inventory match, not runtime validity or a RESOLVED graph edge. Configured lookup now feeds M2-T07 `reexports::resolve_modules`; the legacy `resolve()` entry point remains relative/direct-only.

Limits: 128 config inputs, 1 MiB/file, 16 MiB total, 32 inheritance layers, 4096-byte paths/specifiers, 256 path mappings, 16 targets each and 4096 entries per JSON object/array; nesting remains bounded by serde_json. Diagnostics retain 128 details plus a marker. Source inventory retains the existing 100000-file bound. These bound accepted inputs, not total process memory.

The effective fingerprint includes the config policy token, selected normalized settings/origins, every consumed raw-byte digest/length, missing probes and diagnostics. Raw comments, parent edits and exact-versus-suffixed config changes invalidate it; unused captured configs and input insertion order do not. See the independent hash fixture and [ADR 0018](adr/0018-effective-config.md).

Supported moduleResolution is absent or node/node10 under the source-inventory policy; documented CommonJS/ES module names are recognized case-insensitively. Checking/emit options such as strict/target/lib/jsx are retained in raw fingerprints but do not become type-system proof. Project references, files/include/exclude selection, package/array extends, configDir macros, rootDirs/moduleSuffixes, modern unsupported modes and unknown resolution options are diagnosed. A supported child option can override an unsupported parent compiler value; structural unsupported fields remain visible.

This task intentionally implements the build spec's legacy baseUrl lookup policy. The [TypeScript 6.0 release documentation](https://devblogs.microsoft.com/typescript/announcing-typescript-6-0/#deprecated---baseurl) changes that behavior. No latest-compiler equivalence is claimed; the later oracle must pin a declared compatible policy/version. See [module reference](https://www.typescriptlang.org/docs/handbook/modules/reference#paths) and [inheritance reference](https://www.typescriptlang.org/tsconfig/extends.html).

Run `cargo test --locked -p ripple-resolve`: 21 config tests, 10 legacy module tests and 19 re-export tests cover independent lookups/digests/bindings, unsupported forms, boundaries, persistence round trips and cold repair. The M3 compiler-oracle and benchmark harnesses do not exist yet; accuracy and performance are not measured.

## Configured bindings and re-exports (M2-T07)

Use `ripple_resolve::reexports::resolve_modules(project, sources, facts, config)` to produce ModuleAnalysis. It validates complete captured fact inventory, follows configured imports and named/star exports, and forwards unique module-root imported aliases to their original supported declaration. Explicit export names override stars; stars exclude default. Duplicate explicit declarations, distinct-origin collisions, namespace forwarding, unsupported type declarations, opaque sources and missing providers remain unresolved. Same-origin diamonds are unique. Cycles terminate; an empty cyclic branch remains uncertainty even beside a known branch.

Every successful export retains the original SymbolId, effective type-only flag, actual declaration span and a deterministic transfer witness. ImportedBinding.type_only combines importing syntax and provider flags; call consumers must use that combined flag. Explicit unsupported type-only exports retain `syntactic_type_only` even without a symbol. Transfer records preserve exporter/provider names and source provenance. Unknown-star surface flags remain visible; they are not guessed named targets. A provider's explicit known names can remain usable even while other names are unknown.

ModuleAnalysis includes project/config provenance, present/missing config inputs, import/re-export lookup dependencies, transfers and external fingerprints. It supports serde JSON round trips for persistence handoff. Deserialize produces transport evidence, not a trusted analysis: call `analysis.validate(project, sources, facts, config)` before reuse, with appropriate caller-side artifact size limits. Validation recomputes against captured inputs; live freshness and revision binding remain index/Git obligations. This library writes no SQL/files; index/incremental integration owns cache composition.

External fingerprints include supported signature/exposure/origin/config/lookup/uncertainty evidence. They exclude body hashes and witness locations, so some body-only edits preserve a fingerprint. That is not semantic API equality or permission to skip re-resolution; inferred types, side effects and unsupported forms require conservative fallback and M5's equality gates. No early-cutoff optimization is implemented.

Closure is bounded by 100000 cells/transfers, 512 rounds per phase and five million charged work units. Exhaustion fails explicitly without returning partial unique results. Full witnesses can consume substantial memory; no latency or total-memory guarantee is measured. See [ADR 0019](adr/0019-reexport-fixed-point.md), [fixtures](../fixtures/resolution/reexports/README.md) and the [task report](task-reports/M2-T07.md). Call resolution and CLI integration remain separate tasks.
