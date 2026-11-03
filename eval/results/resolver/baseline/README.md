# First pinned baseline

Tool implementation: e32a230 (full revision, Rust source tree, compiler/options/corpus and binary digest are in each comparison.json). The Rust source tree is identical to the integrated M2 analyzer. Node 24.22.1, TypeScript 6.0.3. No upstream scripts run.

- Mini: 3/3 resolved precision, 3/3 recall; exact independent edges.
- sourcemap-codec four-source projection: 16/16 resolved precision, 16/16 recall; no supported mismatch observed. All excluded scope/mapping/unresolved data are retained.
- Two same-environment external runs produced byte-identical oracle.json, ripple.json, comparison.json and report.md.

Reproduce with the pinned fetch and compare commands in ../../../oracle/README.md (relative link: [harness](../../../oracle/README.md)). Use a new output path. Tool revision/evaluation platform binary digests are provenance fields and may intentionally differ when regenerating at a later commit/platform. Do not interpret this tiny, restricted sample as full-project TypeScript precision/recall or test sufficiency. Performance: not measured.
