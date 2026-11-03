# Resolver evaluation

M3-T01 supplies the pinned compiler API adapter and independent comparable-edge policy in [eval/oracle](../eval/oracle/README.md); [ADR 0008](adr/0008-oracle-selection.md) records the choice. The Node harness is evaluation-only. The mini corpus has three hand-checked semantic edges/four occurrences covering aliases, re-export origins, overload implementation mapping and TSX. Separate tests check UTF-16/UTF-8 coordinates including astral/CRLF text, unmatched/ambiguous mapping and deduplication.

The external corpus manifest pins the sourcemap-codec repository URL, immutable SHA, individual raw source hashes and MIT license. It is an explicit four-source projection with fixed evaluation options, not an upstream build or entire-repository compatibility claim. Network fetches are explicit and no corpus scripts execute. Corpus/compiler/options/policy/analyzer versions belong in every result.

M3-T02 will add measured resolved precision/recall, candidate coverage, mapping exclusions and categorized mismatches. No accuracy numbers are claimed yet. Denominators must be independent of Ripple success; raw misses and mapping failures cannot disappear into supported-scope filtering. Dedicated test-safety/selection and performance measurements belong to later milestones.

## First measured baseline

M3-T02 adds `compare_run.py` and metric tests: synthetic true/false/missed pairs, candidate evidence, duplicate predictions, ambiguous mappings and undefined zero-denominator cases. An unmatched expected endpoint stays a recall miss. Precision is scoped to independently eligible occurrences with mapped predicted endpoints; mapping losses/out-of-scope predictions are separately retained. See the reproduction commands and exact rules in the [harness README](../eval/oracle/README.md).

The owned mini corpus measures 3/3 precision and 3/3 recall. The four-source sourcemap-codec projection measures 16/16 precision and 16/16 recall on eligible edges, no comparable candidate edges, 567 unresolved occurrences, one unmatched constructor definition and six out-of-scope constructor-related class reads. The oracle excludes 524 runtime-identifier occurrences. These exclusions are visible, independent of Ripple success and not included in the eligible-edge denominator. No supported missed/false-resolved edge was observed on this sample; broader accuracy is not established.

M3-T03 must use the no-measured-gap branch: strengthen a supported regression without expanding resolver scope, then regenerate the same-corpus comparison and report unchanged counts if confirmed. Performance, selection safety and incremental correctness remain unmeasured here.
