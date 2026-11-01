# Current module and direct-binding resolution

M2-T01 supplies a library layer, not a new CLI command. `inspect` still reports syntax only and has `resolution_available: false`. Call edges, persistent indexing and impact queries remain upcoming.

Use `ripple_resolve::modules::ModuleResolver` for pure captured-inventory lookup, or `ripple_resolve::resolve(project, sources, facts)` for validated project-wide direct export/import evidence. All source files require one fact record; facts must come from those captured bytes. The production crate uses core records and serde only.

| Supported layer | Behavior |
|---|---|
| Relative paths | Normalize within root; reject escapes, external/nonrelative specifiers and unsupported path forms. |
| Source lookup | Extensionless `.ts`, `.tsx`, `.d.ts` before directory indexes; explicit source extensions exact; `.js` substitutes TS source candidates. |
| Dependency evidence | Ordered missing candidates plus the winner, or all failed candidates, with importer/span provenance. |
| Direct bindings | Supported top-level functions/classes, const-bound functions, named/default aliases and supported overload groups. |
| Type-only imports/exports | Flags propagate conservatively; no runtime call edges are emitted. |
| Uncertainty | Missing/opaque/ambiguous/unsupported targets remain unresolved with reasons. Re-exports and imported-alias forwarding are deferred. |

`Found` describes an inventory match under Ripple's explicit policy, not full compiler/runtime resolution. Config/module modes, path aliases, package exports/entry fields, directory package manifests and arbitrary extensions remain unsupported at this stage. `.jsx`, `.mjs`, `.cjs`, JSON/query imports and unknown suffixes are not guessed. `configuration_evaluated` is false in this stage. Even `incomplete_resolution: false` certifies neither config/runtime policy nor TypeScript completeness; the two indicators have separate meanings. The future integrated resolver must keep configuration/package uncertainty visible before graph confidence is assigned. See [ADR 0015](adr/0015-relative-resolution.md) for the exact contract and [TypeScript's reference](https://www.typescriptlang.org/docs/handbook/modules/reference) for the broader mode-dependent behavior.

Run the independent fixtures and behavioral checks:

```sh
cargo test --locked -p ripple-resolve
```

No compiler-oracle accuracy or performance results exist yet. The oracle is M3; persistent storage, traversal, effective config, re-exports, calls and cold commands retain their individual M2 acceptance gates.
