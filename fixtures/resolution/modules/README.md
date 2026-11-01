# Relative lookup and direct binding fixtures

`lookup.json` independently declares a captured inventory and expected path/rule/failure outcomes. It covers priority collisions, source/runtime suffixes, directory indexes, root traversal, external and missing specifiers.

`bindings.json` lists the TS/TSX programs in `project/` and hand-authored import-alias outcomes. Some programs deliberately contain unsupported/recovered/ambiguous syntax. Supported symbols, missing/opaque providers, type-only bindings, default roles, overload groups and deferred barrels have separate expected outcomes; expectations are not analyzer snapshots. No compiler-validity or runtime-mode agreement is implied.

`cargo test --locked -p ripple-resolve` also checks negative lookup records, cold repair after adding a captured file, explicit path behavior, inventory/project/span validation, deterministic input order and transport round trips. These are resolution-library tests, not proof of incremental persistence or complete call graphs.
