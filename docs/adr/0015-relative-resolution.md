# ADR 0015: Captured relative lookup and direct bindings

Status: accepted (M2-T01).

## Boundary

`ripple_resolve::resolve(project, sources, files)` consumes captured core source/fact records, checks path/span/project consistency and complete fact inventory, and returns deterministic module/direct-binding evidence. Production dependencies are core and serde; Tree-sitter/parse and JSON are test-only harness dependencies. It performs no filesystem, Git, package-manager, compiler or project-script calls. Core SymbolId gains a read-only declaration-role getter; its serialized identity encoding is unchanged.

The caller must supply facts extracted from those captured bytes with a compatible analyzer. Structural validation is not a content-freshness or compiler-validity certificate. Deserialized ModuleResolution is a transport artifact, not a trusted cached index. Persistent generation/freshness handling remains M2-T03/T05 and later incremental tasks.

## Explicit source-inventory policy v1

Use case-sensitive normalized inventory paths, rejecting duplicate/case-colliding inventories and unsupported source extensions. Normalize relative dot segments against the importing file's directory; reject traversal above the root, unknown importers, absolute/drive/control/backslash/query/hash forms and specifiers over 4096 bytes. Nonrelative packages/path aliases remain unresolved, never globally name-matched.

For extensionless leaves without an unrecognized suffix, try `.ts`, `.tsx`, `.d.ts`, then `/index.ts`, `/index.tsx`, `/index.d.ts`. A trailing slash or final `.`/`..` requests directory candidates only. Explicit `.ts`/`.tsx` paths are exact. Explicit `.js` substitutes `.ts`, `.tsx`, `.d.ts`; unsupported `.jsx`/`.mjs`/`.cjs`/JSON/other suffixes are diagnosed. Stop at the first inventory match, recording each preceding missing candidate and the winner. Missing lookups retain all candidates. Adding a missing higher-priority file or deleting the winner can therefore trigger a later lookup repair.

[TypeScript's module reference](https://www.typescriptlang.org/docs/handbook/modules/reference#file-extension-substitution) documents `.js` substitution and that extensionless/directory behavior depends on runtime/compiler mode. This stage deliberately implements the stated Ripple source policy, not all Node/TypeScript modes. `Found` means a matching captured source path under that policy. It is not compiler-oracle agreement or a RESOLVED graph edge. `configuration_evaluated` is explicitly false until a later stage verifies supported config/module/package policy; consumers must not promote conditional inventory matches directly into graph confidence. Package entry fields/conditions, directory package manifests, module modes, config aliases and inheritance are not interpreted here; integration must preserve unsupported configuration/package boundaries before emitting graph confidence. No filesystem or package fallback is silently attempted.

## Direct export and import evidence

Group explicit Local export records by exported name. Link only one supported top-level function/class identity with represented body evidence and unambiguous module-local binding provenance. Const-bound functions follow their normal symbol identity. Anonymous defaults require the stable `default-export` role. Consecutive overload exports may share a represented logical symbol; unrelated duplicate aliases/collisions remain unresolved. Private/internal non-exported declarations are not looked up by globally matching names.

Type-only exposure remains flagged; an imported binding is type-only if either import or export is type-only. Type declarations without executable symbols remain explicit unresolved type records. This layer never turns them into runtime calls. Recovered, resource-limited, truncated or otherwise opaque providers/importers cannot prove a direct target. Missing lexical evidence, unsupported locals and ambiguous bindings remain unresolved. Signature hashes are not semantic proof: a known direct binding can remain identifiable despite a withheld global signature fingerprint; source diagnostics and incompleteness remain visible.

Named/default import aliases retain original names, local names, spans, lookup result and optional symbol identity. Namespace imports stay unresolved. Named/star exports and local forwarding through imports retain deferred records; M2-T07 owns re-export fixed points. Every static import/re-export lookup has a dependency record with source provenance, including successful side-effect imports and failed lookups.

## Determinism, bounds and compatibility

Output arrays use deterministic typed ordering; input file order does not select a target. Limit captured inventory to 100000 files and import/export requests to 100000. Overflow/invalid inventory fails explicitly rather than publishing a truncated success. Diagnostic reasons are structured and retained. `incomplete_resolution` reports unresolved/deferred/source-warning evidence; false still does not certify configuration or all TypeScript semantics; `configuration_evaluated` remains independently false. Module/binding records contain no CALLS edges or test sufficiency claims.

The resolver compatibility token is `ripple-relative-v1`, separate from M1's `ripple-facts-v4-ts0.23.2-runtime0.27.1`. Later config/re-export/call consumers must intentionally evolve compatibility and compose it into index analyzer metadata. No database schema is added by this task.

## Evidence and next consumers

Independent inventory and multi-file binding fixtures cover precedence, missing candidates, TSX/default/alias/type-only exports, overloads, ambiguous locals/exports, opaque providers and deferred barrels. Tests validate cold repair after a captured provider is added, input/project validation, deterministic input-order behavior and serialization round trips. Compiler-oracle and benchmark harnesses are not yet implemented; accuracy and performance are not measured. M2-T03/T04 are independent ready storage/traversal work; M2-T06/T07 then extend config and export evidence before call resolution and cold CLI integration.
