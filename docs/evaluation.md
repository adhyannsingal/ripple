# Resolver evaluation

M3-T01 supplies the pinned compiler API adapter and independent comparable-edge policy in [eval/oracle](../eval/oracle/README.md); [ADR 0008](adr/0008-oracle-selection.md) records the choice. The Node harness is evaluation-only. The mini corpus has three hand-checked semantic edges/four occurrences covering aliases, re-export origins, overload implementation mapping and TSX. Separate tests check UTF-16/UTF-8 coordinates including astral/CRLF text, unmatched/ambiguous mapping and deduplication.

The external corpus manifest pins the sourcemap-codec repository URL, immutable SHA, individual raw source hashes and MIT license. It is an explicit four-source projection with fixed evaluation options, not an upstream build or entire-repository compatibility claim. Network fetches are explicit and no corpus scripts execute. Corpus/compiler/options/policy/analyzer versions belong in every result.

M3-T02 will add measured resolved precision/recall, candidate coverage, mapping exclusions and categorized mismatches. No accuracy numbers are claimed yet. Denominators must be independent of Ripple success; raw misses and mapping failures cannot disappear into supported-scope filtering. Dedicated test-safety/selection and performance measurements belong to later milestones.
