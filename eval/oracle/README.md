# Pinned resolver oracle

Evaluation requires Node **24.22.1**, npm and Python 3.9+. Production Ripple does not. Run from the repository root:

```sh
npm ci --prefix eval/oracle --ignore-scripts
npm test --prefix eval/oracle
node eval/oracle/adapter.mjs eval/corpora/mini eval/corpora/mini/files.json > /tmp/mini-oracle.json
python3 eval/oracle/fetch.py eval/corpora/sourcemap-codec.json /tmp/sourcemap-codec-projection
```

Use the version in `.nvmrc`. TypeScript **6.0.3** is an exact locked evaluation dependency; the TypeScript 7 native package has a different API. No corpus install, plugin, build or test script is executed. Only exact digest-checked source/license files are fetched. `sourcemap-codec.json` pins upstream URL/SHA/MIT license metadata and documents the four-file source projection. This is a controlled source evaluation, not a full upstream-project/config compatibility measurement. Network fetching is an explicit command, not part of routine CI.

## Comparable-edge policy v1

The oracle parses the declared UTF-8 TS/TSX inventory with fixed ES2022/CommonJS/Node10/JSX-preserve compiler options and no standard library, ambient types or external files. It does not load project tsconfig/plugins/package policy. Type/JSX diagnostics are retained; syntax errors fail the run. Compiler definitions are found using `getSymbolAtLocation`, resolving aliases with `getAliasedSymbol`. Overload signatures map to their unique implementation. No target is chosen by matching names.

Eligibility is defined from compiler AST/definitions before seeing Ripple results: direct non-optional identifier calls to top-level function implementations, and runtime identifier reads referring to top-level functions/classes or supported class methods. Owners are top-level functions/classes/methods or file scope. Nested callbacks/functions, namespace containers, receiver/computed/constructor/optional calls, external/parameter/variable targets and non-runtime identifiers are excluded or absent by syntax policy. Non-identifier call targets are outside this contract. Imports and re-exports are used to identify origins, not scored as semantic call edges. Method dispatch is not inferred. The denominator includes supported edges Ripple misses. Unsupported exclusions and mapping failures remain separate from missed edges.

Oracle symbols use `(relative path, kind, declaration byte start/end)` keys, never database IDs. Occurrence spans and definition spans are UTF-8 byte offsets, half-open. UTF-16 offsets that bisect surrogate pairs are rejected. `mapSpan` requires exact declaration spans for endpoint matching; nearest enclosing spans are used only for ownership. Missing/ambiguous mapping is explicit. Deduplicate semantic `(source,target,kind)` pairs, retaining all distinct occurrence spans. Count eligible occurrences and distinct edges separately. Report every exclusion/diagnostic; do not claim full TypeScript accuracy, test sufficiency or corpus representativeness.

The mini corpus independently expects caller → café (two calls, one edge), reference → café (read), and TSX View → caller. The barrel aliases café and the implementation has an overload; a shadowed parameter is excluded. Non-ASCII/astral text precedes call spans. Tests independently assert these outcomes, ambiguous/unmatched mapping, duplicate handling and syntax failures.
