# Verified incremental indexing

`ripple index ROOT` captures current supported source/config/ignore inputs and refreshes the persistent graph, reusing analysis only where its inputs are proven unchanged. `ripple index ROOT --full` explicitly reparses and resolves everything. Neither mode checks out/stages files, runs project scripts or fetches Git objects.

```sh
ripple index .
ripple index . --format json
ripple index . --full
ripple status
ripple impact src/lib.ts::f --format json
```

A fresh index performs a full build. Repeated calls publish complete immutable generations, including semantic no-ops: zero parses is not a claim of zero writes, constant index size or end-to-end speed. Archived generations currently have no garbage collection.

## Capture, parse reuse and invalidation

1. Observe Git provenance and capture supported path membership and raw bytes once. Capture config/inheritance, package/lock/ignore inputs, discovered diagnostics and missing auxiliary probes as well as TS/TSX.
2. Load a compatible validated generation and its sealed analysis cache. Reuse a file's facts only when its captured content digest/length and analyzer/hash/schema/cache policy match. Parse/extract added/changed supported files; omit deleted facts from the new generation.
3. Non-source manifest, effective config, missing auxiliary probes or discovery policy changes require a reported full parse/rebuild. Missing analysis cache or incompatible analyzer/hash metadata also triggers full work. Corrupt state fails explicitly. Unsupported on-disk schema formats require safe archive/rebuild, not silent migration or deletion; `--full` does not bypass format checks.
4. Reuse module analysis for identical inputs or the narrow supported body case below. Otherwise run the full bounded import/re-export fixed point, preserving ambiguity, failed lookups and unknown exports.
5. Propagate changed facts/surfaces and candidate records through old and new lookup dependencies, including missing probes and re-export cycles. Reuse a reference file only with unchanged complete facts/import bindings/config and no prior candidate semantic edge. If reached by invalidation, its external surface must additionally be known and unchanged. Retain reference-level unknowns only under those same input proofs. Rebuild current definitions and all module edges/unknown boundary ordinals.
6. Recapture and verify source/config/ignore/HEAD membership and bytes before publishing snapshot, captured bytes and analysis cache in one SQLite transaction. Retry at most twice (three capture attempts). Continued changes, parser/build errors or failed writes preserve the previous committed generation. Metadata/module sidecars are derived and may report pending after a successful DB commit.

The optimized body case requires stable coordinates, inventory/config/policy and complete supported facts. Compare all module-relevant fields after excluding only symbol body hashes and references: keep signature hashes, imports/exports, scopes/bindings, parse state, diagnostics and declaration/signature/body spans. Missing hashes do not prove equivalence. A body literal `1` → `2` can qualify; changed lengths/locations, signatures/shapes and uncertain contexts fall back to full module analysis. This proves unchanged inputs for Ripple's supported resolver, not TypeScript API equivalence or inferred return-type semantics.

Freshness is content-verified, not OS-atomic. An edit after the final verification can make a newly published view stale; status/impact reject it until refreshed. No stat-only shortcut or silent query refresh is used. The production index crate remains independent of Git subprocesses; the CLI supplies the Git provenance provider.

## Reporting

JSON keeps the format-v1 index envelope and existing generation/counts/diagnostics fields, adding `data.update`:

| Field | Meaning |
|---|---|
| `strategy`, `rebuild_reason` | Full or guarded selective work and the explicit fallback reason, if any. |
| `parsed_files`, `reused_files` | Files parsed/facts reused in the accepted capture attempt. |
| `module_files_resolved`, `module_cache_reused` | Files processed by module resolution, or reuse of validated module evidence. Non-equivalent contexts use full module analysis. |
| `reference_files_resolved`, `reference_files_reused` | Files processed/skipped in lexical-reference resolution after proof checks. |
| `total_parsed_files` | Actual parsing across accepted and rejected capture attempts; no success report exists after a hard failed update. |
| `source_delta_available`, `added`, `changed`, `deleted` | Source changes only when previous membership is available, or the prior index is known empty. An explicit/incompatible full build does not invent historical additions. |
| `invalidation_candidates` | Deterministic watched-path closure, including changed/deleted roots; these are candidates, not the actual resolved-file count. |
| `unchanged_external_surfaces` | Known matching module export fingerprints; input equality is still required for reuse. |

Human output displays accepted-attempt work, retry parse totals, reasons and delta availability. Counters describe analyzer work, not latency or test safety. Some unknown contexts still require reference work on a no-op. Input verification, validation, graph reconstruction and complete generation writes remain broad; performance is not measured.

## Correctness and reproduction

`CanonicalState` compares captured input hashes, facts/spans/diagnostics and complete typed graph/provenance/confidence/unresolved records. `CanonicalAnalysis` additionally compares module/config/discovery evidence and missing probes. Operational origin/time/generation/DB IDs are excluded; lookup precedence and witness-hop order are retained. The required invariant is full canonical incremental = independent cold state.

The generated guard was integrated before cutoff. It now runs 96 basic edit comparisons, 24 fully resolved body comparisons and 128 adversarial edit comparisons, persistence reloads, no-op idempotence and deliberately stale output faults. Targeted tests cover global invalidators, real SQL rollback, parser/capture failures, concurrent verification, moves/deletes, signatures/classes/aliases, cyclic/ambiguous barrels, candidates and missing-target repair. See [seed/replay policy](../fixtures/incremental/regressions/README.md).

```sh
cargo test --workspace --locked
cargo test --locked -p ripple-index --test properties_basic -- --nocapture
python3 scripts/m6-smoke.py --binary target/debug/ripple
python3 scripts/m6-smoke.py --install-root /path/to/install
```

The installed smoke includes every prior implemented command and refresh/`--full`/stale/failure sequences on Linux, macOS and Windows. The extended four-seed adversarial campaign also passed 1,024 comparisons. Pinned history replay passed 16/16 steps across two clean campaigns with identical semantic/counter/diagnostic results; [raw evidence](../eval/results/replay/README.md) retains all unknowns and scope exclusions. This is finite equality evidence; dedicated test selection/safety remains M7–M8. No power-loss fault-injection or test-sufficiency guarantee is claimed. See [cache](adr/0025-incremental-cache.md), [refresh](adr/0026-verified-refresh.md), [generated guard](adr/0027-generated-equality.md), [selective proof](adr/0028-selective-resolution.md) and [CLI policy](adr/0029-incremental-cli.md).

## M6 release measurements

The v0.3.0-dev candidate has [reproducible release measurements](../bench/results/README.md): 20 samples each for cold, no-op, stable body, export and configuration changes on a pinned four-source external projection and a 65-file supported synthetic chain. All 200 measured comparisons plus 10 discarded warm-ups matched independent cold analysis. Body updates parse one file and reuse the rest; global config changes parse all files. The supported body case resolves zero module files and one reference file.

On the recorded machine, supported-chain body p50/p95 was 27.04/28.11 ms versus independent cold 28.35/29.75 ms. External body was 49.48/52.95 ms versus cold 48.95/51.80 ms. The measurements show modest or absent small-workload latency gains; no general speedup or latency bound is claimed. Caches are warm, application indexes are fresh per sample with one prior generation for updates, and library timing excludes process startup, real Git provenance and canonical comparison. Raw data records hardware, toolchain, hashes, counters, all cases and scope limits. CI checks correctness and installed behavior, without absolute timing thresholds.
