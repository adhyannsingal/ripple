# 0.4.0 candidate release checklist

Status: candidate preparation / final verification in progress. No public tag, GitHub Release or crate publication has occurred.

## Candidate and distribution

The versioned ZIP includes Ripple, MIT license, README/changelog, BUILD metadata, dynamic dependency output, resolved Cargo.lock dependency notices and per-member hashes in its external manifest. Cargo package publication remains disabled; delivery is binary archives. Rust 1.99.1 and the lockfile are pinned. SQLite is bundled/static; no SQLite server or separately installed SQLite library is needed. Windows candidate builds explicitly use static CRT; Unix builds retain system runtime dependencies shown by the loader audit. No fully static/older-OS compatibility claim.

Target candidates: x86_64 GNU Linux, x86_64 Intel macOS and x86_64 Windows MSVC through the manually dispatched Candidate artifacts workflow; Apple Silicon macOS through the local host. Each build installs from locked source, packages/audits, verifies hashes and runs all six commands. Separate fresh hosted runners download/unpack and smoke the three CI archives without a Rust build, npm install, database server or project scripts. Hosted runners have preinstalled tools; this is not a proof of a hermetic OS image lacking Node. Product smoke invokes only Python/Git/Ripple.

Archives are unsigned and not notarized. SHA256SUMS detects byte changes; it is not an author signature. Verify against the manifest from the intended source/workflow before running binaries. Runtime dependencies, tested OS/architecture and hashes are recorded, not generalized to every older OS. CI artifact retention is 14 days and requires GitHub authentication; it is a review channel, not a permanent release URL.

## Reproduction

Packaging/verification uses Python stdlib (planning remains Python 3.9+ compatible), cargo/rustc on the build host, normal locked Cargo registry archives and the platform loader-audit tool (otool/ldd/dumpbin). The owner-approved MIT license and exact upstream notices are retained; Cargo.lock archive hashes bind registry notice bytes. The notice list includes build/dev/other-target packages and is not a precise linked-code SBOM.

Run the manual Candidate artifacts workflow from the chosen source. Locally build/install with cargo install --locked --path crates/ripple-cli --root /tmp/ripple-install, then invoke scripts/release.py prepare with --binary, a new --output directory and the actual rustc host as --target. Verify with scripts/release.py verify --directory PACKAGE_DIR --smoke. Do not relabel a host-built binary as another target.

## Required final evidence

- [ ] Full fmt/locked build/strict Clippy/Rust tests, planning/replay/mutation metric tests.
- [ ] All six commands and quickstart from fresh local installation and unpacked archives.
- [ ] Three-target candidate build plus separate fresh-runner archive smoke.
- [ ] Final-source owned and external oracle projections; exclusions/unknowns preserved.
- [ ] Final-source pinned history replay with zero canonical mismatches.
- [ ] Final-source compiling mutation recall with explicit invalid/timeout/survivor/miss categories.
- [ ] Final-source update and verified workflow samples, RSS/index-size and rejected optimization evidence.
- [ ] Dependency/secret audit, package inventory/checksums/notices and migration/output compatibility review.
- [ ] Artifact manifest linked and no known blocking candidate defect.

Historical methods: [evaluation](evaluation.md), [benchmarks](benchmarks.md), [mutation](../eval/mutation/README.md), [replay](../eval/commit-replay/README.md), [oracle](../eval/oracle/README.md). Final evidence will be linked below after checks actually complete.

## Compatibility and publication handoff

JSON format v1 and SQLite schema v2 remain unchanged. Semantic analyzer/cache/hash versions, rather than package version alone, determine reuse. Unsupported schema state requires explicit safe archive/rebuild; --full does not erase incompatible state. Captured-content freshness is retained; no metadata-only shortcut ships. Test results remain potentially related, never sufficient. Known dynamic/global-state misses and narrow TS/config scope are documented limitations, not hidden pass criteria.

After final verification, an owner may separately authorize a public v0.4.0 tag/release on the recorded candidate source with the recorded archives/checksums and draft notes. No rebuild under a different source should inherit these evidence claims. Broader runtime test safety, large/cold-cache/concurrent/archive-growth workloads, signing/notarization and package-manager publication remain separate future work.
