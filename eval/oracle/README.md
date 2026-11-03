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

## Measured comparison

Build the trusted analyzer with `cargo build --locked -p ripple-cli`, then run:

```sh
python3 -m unittest discover -s eval/oracle/test -v
python3 eval/oracle/compare_run.py --source eval/corpora/mini --binary target/debug/ripple --output /tmp/mini-comparison
python3 eval/oracle/compare_run.py --source /tmp/sourcemap-codec-projection --manifest eval/corpora/sourcemap-codec.json --binary target/debug/ripple --output /tmp/external-comparison
```

Output directories must be new. The runner checks exact Node version, copies only declared source bytes into a disposable Git root, runs index/status, then exports the already CLI-validated private index in a read transaction. SQLite schema is an internal evaluation coupling, not a public export API. Do not point it at arbitrary indexes. Supply the binary built from this checkout; metadata records its digest, checkout SHA, Rust source tree and analyzer policy versions (a digest alone does not attest build provenance).

Resolved precision = correct distinct pairs / all mapped comparable resolved pairs. Recall = correct / all independently eligible oracle pairs, including missing Ripple definitions. Candidate coverage = oracle pairs with candidate evidence / oracle pairs; candidates never count as resolved true positives. Zero denominators are JSON null. Precision excludes out-of-scope occurrences and unavailable endpoint mappings; their complete counts/records remain visible. Recall never drops oracle edges for mapping failure. Unresolved counts are occurrences across the full analyzed projection, not unique comparable edges. Edge duplication and multiple witnesses do not inflate the metric.

The raw oracle/Ripple exports, sorted comparisons and Markdown report retain tool/corpus/input provenance, mapping losses, out-of-scope predictions and mismatch rules/reasons. Byte-identical reruns are expected on the same pinned environment/binary. Cross-platform binary hashes differ. Routine CI compares only owned fixtures; external fetch/evaluation is an explicit, reproducible command. Results are narrow supported-scope samples, not general TypeScript accuracy.
