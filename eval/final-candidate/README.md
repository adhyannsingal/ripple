# Final 0.4.0 candidate evidence

The [results summary](results/summary.json) pins integrated source 16eaf563b9376d2c73fdc98c85eccdb37f82b235, the measured ARM64 binary hash and version. Raw oracle, replay, mutation, update and verified-workflow data are retained under results/. Upstream MIT notice is retained for derived external evidence.

Reproduce by building both index examples (incremental_evaluate and workflow_benchmark) in release mode and the CLI from that source, installing pinned Node 24.22.1 / TypeScript 6.0.3 through the oracle lockfile, then running run.py with binary/node/typescript/incremental-helper/workflow-helper paths and a new output directory. Reproduction uses network only for pinned corpus acquisition. It executes only owned mutation tests, never fetched project scripts. Full commands are in each corresponding harness README and raw command fields.

Results: oracle mini 3/3 and external source projection 16/16 precision/recall within eligibility policy; exclusions/unknowns retained. Replay 16/16 across two clean campaigns and 200 measured update comparisons plus 10 warm-ups have zero full canonical mismatches. Mutations: symbol 3/5, combined/import 4/5, all-tests 5/5; compile-invalid, timeout and survivor exclusions remain explicit. Workflow timings separate real Git/content verification from cached storage/traversal, with RSS/size/state/hardware/raw samples.

The first provisional timing pass overlapped local Cargo tests and is not reported here. This complete final-source campaign ran after those tests stopped; background desktop load remains uncontrolled. No optimization gain or general accuracy/safety/latency guarantee is claimed. The two full-repository test demos remain incomplete and separate from the four-source oracle projection. Statistics are nearest rank on finite small workloads; p99 of 20 samples is the maximum, not a robust tail estimate.

The candidate archive manifest is dist/artifact-manifest.json, and actual gate outcomes are in docs/release-checklist.md. Runtime/archive verification does not imply a public release/tag exists.
