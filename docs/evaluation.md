# Evaluation evidence and limits

Resolver agreement is measured only within [the documented oracle projections](../eval/results/resolver/final/README.md); historical test-impact examples are [explicitly incomplete](examples/m7-test/README.md). These are different scopes and do not establish full TypeScript correctness.

## Controlled fault-catching evaluation

The [owned mutation harness](../eval/mutation/README.md) uses pinned Node/TypeScript, five individually executed test files and eight deterministic compiling/invalid/timeout controls. Two clean campaigns reproduced semantic results. [Raw results](../eval/results/test-safety/mutations.json) record analyzer source/binary/fixture hashes, fixed base/head commits, actual compile/runtime results, exclusions and per-policy misses.

| Selection | Selected failing / all failing | Selected / possible files (same killed cases) |
|---|---:|---:|
| Symbol-backed | 3/5 | 3/25 |
| Conservative combined | 4/5 | 6/25 |
| Independent static file-import | 4/5 | 6/25 |
| All-tests | 5/5 | 25/25 |

Micro recall includes only five compiling, completed, killed mutations. One survivor has undefined recall; one compile-invalid and one timeout mutant are excluded, never credited as caught faults. All baseline suites passed. Three supported direct failures were selected. The dynamic-import failure is missed by both static policies; the global-state side-effect failure is missed by symbol-only selection but caught through file-import fallback. Miss categories and exact paths remain in raw rows. Combined equals the file-import baseline in this corpus, so no precision advantage is claimed.

This is a small deliberately constructed fixture campaign, not an unbiased safety estimate, coverage analysis or a safe-skip guarantee. No fetched project tests ran: external runtime evaluation is deferred until commands/dependencies are reviewed; the M7 external static demo remains separate. Reproduction commands, aggregate/empty-case policy and resource bounds are in the harness README. Final release evidence must be regenerated on its integrated candidate.

## Final 0.4.0 source refresh

[Final candidate summary](../eval/final-candidate/results/summary.json) and [method/scope](../eval/final-candidate/README.md) bind all rerun datasets to source 16eaf563b9376d2c73fdc98c85eccdb37f82b235 and its binary hash. The same finite 3/3 and 16/16 oracle projections, 16 history comparisons, 200 measured incremental comparisons plus warm-ups and mutation-policy outcomes passed. External exclusions/unknowns and runtime misses remain visible. These refreshes do not expand the eligibility/safety scope of the initial studies.
