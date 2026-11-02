# ADR 0018: Captured effective configuration and local aliases

Status: accepted and implemented in M2-T06.

## Boundary and compatibility

`CapturedConfigs` owns immutable caller-supplied bytes. `evaluate(root)` constructs an `EffectiveConfig` with effective base URL/path mappings, defining-config origins, sorted present/missing dependencies, structured diagnostics and a versioned SHA-256 fingerprint. Runtime config cannot bypass evaluation through deserialization. `ConfiguredResolver` borrows it and a validated source inventory; no filesystem, compiler, Git, package manager or project code is executed. M2-T05 owns discovery/capture and live manifest verification. The existing `resolve()` direct-binding API remains the M2-T01 relative layer; M2-T07 composes configured lookup into re-export/binding resolution.

The policy token is `ripple-config-v1-legacy-baseurl`, independent of `ripple-relative-v1` and syntax/hash versions. Downstream analyzer metadata must compose these tokens. `supported()` means no detected unsupported configuration in this subset; it is not a compiler-validity certificate. `Found` is a conditional captured-source match, never a RESOLVED graph edge by itself.

## Parsing and inheritance

Accept UTF-8 JSON with line/block comments, trailing commas and optional BOM. Preserve quoted strings and reject malformed text, duplicate decoded keys, non-object roots and invalid compilerOptions. Comment preprocessing preserves line breaks. JSON objects/arrays are capped at 4096 entries and serde_json's nesting bound remains enabled.

Follow one relative repository-local `extends` string. Probe an exact captured path, then append `.json` if missing and not already suffixed. Record every probe, including missing higher-priority files. Reject cycles, package/array/absolute/outside-root extends, macros and unsupported forms. No directory/package fallback occurs. Limit inheritance to 32 configurations, inventory to 128 files, 1 MiB/file and 16 MiB total, and paths to 4096 bytes. Duplicate/case-colliding config inventories fail explicitly. Reports are capped at 128 details plus a truncation marker; any diagnostic prevents successful configured lookup.

Merge compilerOptions by property; the child replaces the entire paths map. Normalize baseUrl relative to the file declaring it. Retain paths declaration origins; interpret targets against the final effective baseUrl when present, otherwise against their declaring config directory. A child baseUrl override therefore also changes inherited path target roots. In the absence of baseUrl, targets must start with `./` or `../`. Bound paths to 256 mappings and 16 targets each. Root is represented by an empty normalized directory string.

Support absent or explicitly node/node10 moduleResolution and the documented CommonJS/ES module names, case-insensitively, only under Ripple's existing source-inventory policy. Other modes, project references, files/include/exclude selection and unknown config/resolution options produce diagnostics. A small explicit set of checking/emit options is retained in raw fingerprints without interpreting or validating TypeScript's type system. Unsupported effective compiler options can be overridden by a supported child value; unsupported structural fields anywhere in the chain remain diagnostic.

## Matching and provenance

Relative requests bypass aliases. For bare specifiers, exact paths keys win, otherwise choose the longest wildcard prefix. Support one wildcard in patterns/targets, including suffix matching. Equal-priority wildcard ambiguity is unresolved rather than guessing declaration order. Try the chosen mapping's targets in array order, retaining missing source probes; do not try a less-specific mapping after failure. If all supported target lookups are missing, try baseUrl next. Unmapped packages remain unresolved; no package fallback occurs. Unsupported extensions/path forms fail explicitly rather than trying alternate interpretations. Preserve original importer/specifier, config hash, route/declaring-config evidence and ordered attempts in each configured lookup.

Mapped probes reuse the existing TS/TSX/.d.ts/JS-substitution/directory source policy. Substitution is normalized within the repository before probing; absolute paths, controls, backslashes, URI/query/hash forms and root escapes cannot identify a target.

The effective fingerprint is raw SHA-256 of compact JSON encoding of `(policy token, root, baseUrl, sorted paths, sorted config dependencies, sorted diagnostics)`, hash version 1. Each dependency includes raw-byte SHA-256/length or explicit missing state. Formatting/comments and overridden inputs therefore still invalidate the fingerprint. Unused captured configs do not. No timestamps, machine paths or insertion order enter it. Present dependencies plus `CapturedConfigs::get` supply original bytes for a later persistent input manifest.

## References and evidence

The [TypeScript module reference](https://www.typescriptlang.org/docs/handbook/modules/reference#paths) describes wildcard prefixes and ordered target fallback; [extends documentation](https://www.typescriptlang.org/tsconfig/extends.html) describes declaration-relative inheritance. Ripple deliberately retains the requested legacy baseUrl lookup behavior. [TypeScript 6.0 documentation](https://devblogs.microsoft.com/typescript/announcing-typescript-6-0/#deprecated---baseurl) deprecates this behavior; current compiler/runtime equivalence is not claimed. M3 must pin and disclose its oracle policy/version instead of silently comparing different semantics.

Independent fixtures cover inherited origins/overrides, exact/wildcard/fallback probes, comments, missing files, cycles, unsupported forms and bounds. A separate Python SHA-256 recipe supplies a known effective fingerprint. No compiler-oracle accuracy or performance measurement exists yet.
