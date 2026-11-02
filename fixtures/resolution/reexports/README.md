# Re-export and configured binding fixtures

Run `cargo test --locked -p ripple-resolve --test reexports`.

`project/` contains independently authored origin declarations, named/default/type-only aliases, imported forwarding, star barrels, same-origin diamonds, distinct-origin collisions, explicit overrides, anchored/unanchored cycles and unknown star branches. The fixture config maps `@/*` to `./*`. `expected.json` independently names each client import's original file/name/type-only flag or required unresolved reason. Tests verify original transfer membership and actual declaration spans for successful bindings.

The 19 tests add targeted deletion/missing-provider repair, unsupported config/recovery, exact names through unknown surfaces, a second unknown competing branch, explicit duplicate/shadowed/namespace forwarding, anonymous defaults, stable input order, JSON artifact revalidation/tampering and fingerprint signature/body behavior. A 521-file chain exceeds the explicit 512-round closure bound and must fail without partial success. These are bounded fixture results, not a compiler oracle or performance benchmark.

Artifacts persist config/input/dependency/transfer/fingerprint records through serde round trips. They are transport evidence until recomputed/validated against captured facts and config. No project scripts, filesystem lookups or external programs run in the production resolver.
