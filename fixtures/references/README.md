# Import, export and lexical reference fixtures

These are syntax-only programs with independently authored projections in `expected.json`; unresolved module/type names are intentional. They are not compiler-oracle output. A cycle means both source files retain their relative specifiers, not that a resolver fixed point has run.

`cargo test --locked -p ripple-parse --test references` checks the projections, source provenance, ownership, deterministic extraction and round trips. Separate behavioral tests check scope parent chains, parameter/block/var/catch/loop/destructuring shadowing, default parameter environments, recovery and explicit unsupported/resource boundaries. Binding inspection in tests establishes lexical evidence only; production semantic resolution remains in ripple-resolve.
