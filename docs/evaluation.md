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
