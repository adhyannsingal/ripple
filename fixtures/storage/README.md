# Storage fixtures

`basic.json` describes Unicode/CRLF source, a candidate call and an unresolved boundary. `support.rs` constructs independent core-validated FILE, TEST_FILE, FUNCTION and OPAQUE_BOUNDARY records with typed edges, facts, diagnostics and source/config/ignore/package/binary-lockfile manifests. These are storage contract fixtures, not output from a complete resolver or index builder.

`cargo test --locked -p ripple-index` exercises canonical reopen round trips, archived generations, injected write rollback, foreign keys, concurrent readers/writers, stale counters, derived-summary repair, incompatible versions, corruption, observed symlink rejection and input/storage size limits. The reader race pins the actual loader before another connection publishes a newer generation.
