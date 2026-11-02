# SQLite storage library

`ripple_index::storage::Store` persists validated caller-supplied snapshots in `.ripple/index.db`. M2-T05 now composes cold publication and live freshness verification through index/status/impact.

1. Open a private local state directory with `Store::open(directory, expected_versions)`. Unknown/nonempty foreign databases and incompatible schema/migration history fail explicitly.
2. Build an `IndexSnapshot` validated against captured sources. Supply one `InputBytes` for every manifest entry, including source/config/ignore/package/lockfile roles. `input_hash(bytes)` computes raw SHA-256 version 1; syntax fingerprints use separate framing/domains. Retain effective-config and ignore-policy provenance in the core metadata/manifest.
3. Use `next_generation()` as a hint, then publish with `publish(&snapshot, &inputs)`. A competing publisher may invalidate the hint; recompute/revalidate rather than overwrite. Publication requires the next positive generation and verifies actual captured bytes. Errors before commit preserve the previous generation.
4. Inspect the successful `PublishReceipt`. Its generation is committed even when `metadata` is `Pending`; repair with `recover_metadata()` instead of treating it as an unpublished generation. Recovery always uses the latest committed generation, which may be newer than the receipt under concurrent writers.
5. `load()` returns `None` only for an empty index, or one complete validated `LoadedGeneration` containing snapshot, source snapshot and all captured bytes. `load_generation(id)` accesses an immutable archive. Loads check compatibility, record identities, byte hashes, core invariants and a metadata/canonical-state integrity seal.

SQLite is authoritative. Missing, malformed or stale regular `metadata.json` files can be reconstructed with `recover_metadata()`. A nonregular/symlink summary path produces an explicit repair error; remove the obstruction only when safe. The metadata file contains provenance/counts, not a second graph authority.

A busy writer returns `StorageError::Busy` immediately. Reads pin a generation under a transaction and can coexist with publication; two writers cannot interleave publication. The API never deletes old generations. No migration from unsupported schema versions or archive garbage collection is implemented.

Inputs are capped at 16 MiB each and 256 MiB total; JSON records/summary at 64 MiB each, aggregate records at 256 MiB and persisted rows at one million per generation. Unsupported oversized state fails explicitly. Storage assumes a caller-owned local directory; observed symlinks/nonregular database sidecars are rejected, but adversarial replacement races, hard links and network filesystems are outside the containment guarantee. Integrity seals are corruption checks, not authentication. Performance and power-loss fault injection are not measured.

Run `cargo test --locked -p ripple-index` for the fixture, rollback, corruption, concurrency and recovery tests. See [ADR 0016](adr/0016-storage-generations.md) and [storage fixtures](../fixtures/storage/README.md).

The cold builder additionally writes `modules-N.json` as a derived per-generation module-evidence cache after publication. Missing/corrupt/pending cache files do not override SQLite; current queries reconstruct module completeness from captured facts/config. Future cache reuse must run module artifact validation and index freshness checks. See [cold-index policy](adr/0021-cold-index.md).
