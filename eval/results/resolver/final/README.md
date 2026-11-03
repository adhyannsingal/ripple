# Final M3 comparison

Regenerated at the source revision in comparison.json (02c9cba), after adding the composition regression. No production resolver behavior changed because the measured baseline exposed no supported mismatch. All raw oracle/Ripple semantic artifacts are byte-identical to baseline, and all metrics, mismatch categories, exclusions and candidate/unresolved counts are equal. Metadata intentionally records the newer analyzer checkout/Rust source tree.

- Mini: precision 3/3; recall 3/3; zero false/missed comparable edges.
- External pinned source projection: precision 16/16; recall 16/16; zero false/missed comparable edges.
- External limits: 567 unresolved occurrences, 524 oracle excluded occurrences, six out-of-scope predictions and one unmatched constructor definition. Zero eligible predicted edges lost to mapping. These results do not measure general TypeScript accuracy or test sufficiency.

Reproduce with the pinned fetch/compare commands in [harness README](../../../oracle/README.md), supplying a freshly built trusted binary and new output directory. Performance is not measured. Corpus SHA/license/content hashes, binary digest, Rust tree, compiler/options/Node and tool-policy provenance are retained in JSON.
