# ADR 0016: Validated immutable SQLite generations

Status: accepted and implemented in M2-T03.

## Decision

Schema v1 uses STRICT tables for generations, captured inputs, typed nodes, file/symbol details, edges and unresolved references. Composite foreign keys keep endpoints and details within one generation. Semantic IDs remain keys; ordinals only retain record order. Typed JSON retains complete facts/provenance alongside constrained relational identity columns, checked against decoded records on load. The application ID is `RIPL`; `user_version` and a migration checksum reject foreign, changed or unsupported schema history. There is no automatic upgrade from other versions.

A publisher validates compatibility, manifest coverage, lengths, raw SHA-256 digests and core snapshot invariants before writing. One immediate transaction inserts all rows, marks completion and changes the singleton active pointer. A generation must equal the committed current counter plus one; a counter peek is not a reservation. Failed transactions preserve the old generation. Completed archives are immutable through the API and remain available by generation ID.

Connections use WAL, foreign keys, FULL synchronous mode, trusted-schema off and defensive mode. Writers return an explicit busy error without waiting. A read transaction pins the selected generation before loading its records; validation finishes before an owned snapshot is returned. This follows SQLite's documented [isolation](https://www.sqlite.org/isolation.html) and [pragma](https://www.sqlite.org/pragma.html) behavior.

Captured-input hash v1 is SHA-256 of exact raw bytes, distinct from syntax hash domains. A separate domain-tagged seal covers metadata plus canonical snapshot state. It detects accidental changes including otherwise-valid missing relationships, but is not authentication. All input roles retain bytes; only source roles require UTF-8. Effective config/ignore policy and revision provenance stay in snapshot metadata/manifests. Storage does not prove live filesystem freshness or extraction correctness.

After DB commit, the derived `metadata.json` is written through a synced same-directory temporary file and rename. Failure returns a successful publication receipt with `Pending`, since the database already committed. Explicit recovery validates and summarizes the latest committed generation while holding the writer slot, preventing a stale concurrent recovery from overwriting a newer summary. A publication receipt's summary may therefore describe a newer generation than its own committed generation. SQLite remains authoritative.

Limits are 16 MiB per input, 256 MiB captured bytes per generation, 64 MiB per JSON record/summary, 256 MiB aggregate serialized records and one million persisted records. Load preflights stored row/blob sizes. These limits bound accepted data, not total process memory or SQLite query cost. The storage directory must be private and local. Observed symlinks/nonregular files are rejected; newly created Unix directory/database permissions are 0700/0600. Concurrent hostile path replacement and network filesystem guarantees are outside scope.

## Evidence and consequences

Storage fixtures cover canonical round trips, candidate edges/unresolved boundaries, binary package/lock inputs, rollback, foreign keys, stale writers, concurrent reads, corruption/version rejection and summary recovery. Archives currently have no garbage collection. No benchmark or power-loss fault-injection result is claimed. M2-T05 composes capture, resolution, storage and bounded freshness verification; traversal consumes loaded graph snapshots rather than SQL at each hop.
