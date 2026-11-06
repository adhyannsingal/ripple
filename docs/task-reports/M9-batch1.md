# November 6 practical validation and hardening batch

Status: done (M9-T01/T02/T03); M9-T04 remains pending.

- Fixed SEC-01: manifests capped at 1 MiB, compressed ZIPs at 160 MiB, regular opened handles and streamed hashing; expansion/path/inventory checks retained. Original four archives verify unchanged.
- Added standalone discovery's 256 MiB captured-byte limit; indexing already had a corresponding wrapper. This is not a process-memory cap.
- Compiled and independently ran all tests in an owned small Git repository: baseline 3/3 passed; deliberate regression failed only checkout.test.ts. Ripple selected it symbol-backed, kept shipping.test.ts fallback-only and excluded unrelated.test.ts. Stale detection, refresh, cold equality and user Git/index/main-database-file preservation passed.
- Actual verification: 277 Rust, 37 Python, 6 Node tests; fmt/strict Clippy; Linux/macOS/Windows CI and new oracle/mutation/practical CI; dependency/history/evidence scans with zero findings. [Raw evidence and methods](../examples/small-repo/README.md).
- Limits: Eight unresolved occurrences/incomplete analysis remain explicit even in this fixture. A small passing exercise is not full TypeScript semantics or a safe-skip guarantee. Five-sample warm timings are exploratory. Original M8 artifacts remain pinned to their earlier source; these changes are not repackaged/publicly released.
- Handoff: Working local developer preview for documented TS/TSX scope. Next is M9-T04: command-wide cancellation and subprocess cleanup, plus explicit index quota/retention policy. No existing user state, caches or graphify-out was removed.
