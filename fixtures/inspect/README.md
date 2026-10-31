# Inspect CLI fixtures

`project/` has independent TS/TSX syntax examples and an empty file. Missing imported modules are intentional: inspection does not resolve dependencies. `expected.json` hand-specifies names, aliases, export roles, reference kinds and owners; tests separately validate envelope fields, hashes/spans and scope presence.

`empty-human.txt` is a complete independently authored human golden. `caller-human.txt` hand-specifies every populated human field, source coordinate, scope and semantic key. Its signature/body placeholders are filled from the corresponding JSON fields to cross-check rendering; hash semantics are independently covered by the parser's digest oracle and hash-change fixtures. Neither file is analyzer-generated output. Golden line endings are normalized to LF in comparisons; the populated report is also checked with CRLF source.

Run `cargo test --locked -p ripple-cli --test inspect`. Temporary-project tests cover errors, uncertainty, deterministic relocation/repetition, symlink policy, terminal escaping and read-only operation. Demo:

```sh
cargo run --locked -p ripple-cli -- inspect src/caller.ts --root fixtures/inspect/project
cargo run --locked -p ripple-cli -- inspect src/view.tsx --root fixtures/inspect/project --format json
```
