# ADR 0032 — Licensing, evidence and candidate release policy

Status: accepted (M8-T03).

The owner explicitly selected MIT during M8. The project license uses the configured human Git identity; upstream copyright/license notices retain their own authors. Cargo package/binary distribution metadata and third-party notices are audited in T04. No Codex attribution is added.

Final candidate readiness requires integrated source identity, all six installed commands on supported OS runners, fixture/oracle/replay/mutation/performance evidence with explicit scopes, checksums and an artifact manifest. Known unsupported constructs and designed mutation misses remain disclosed; no safety/latency guarantee is invented. Unsafe profile optimizations can be rejected with evidence. Public tag/package/GitHub Release publication is a separate action from preparing reviewable CI artifacts.

Persistence remains schema v2 and format-v1 JSON; internal package version changes alone do not silently migrate or destroy state. Incompatible schemas require explicit archive/rebuild; corrupt state fails. Candidate archives include the executable, project license/docs, dependency notices and content hashes. Git is a runtime prerequisite; bundled SQLite needs no server or separately installed SQLite library. Exact dynamic runtime dependencies and tested host/architecture are recorded, not generalized to untested OS versions.
