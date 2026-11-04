# Incremental regression seeds

`basic-seeds.json` pins policy `ripple-property-basic-v1`, eight seeds, twelve default edits and a hard 64-edit limit. The generated suite builds 3–6-module call chains, a forwarding barrel, a top-level test and a missing-import consumer. Each six-step cycle changes a body, toggles an optional signature parameter, adds/deletes a missing provider, retargets a re-export and adds/deletes an extra file. No project scripts or Git checkout are involved.

Run the default suite (also included in `cargo test --workspace --locked` on CI):

```sh
cargo test --locked -p ripple-index --test properties_basic -- --nocapture
RIPPLE_PROPERTY_SEED=42 cargo test --locked -p ripple-index --test properties_basic generated_edit_sequences_equal_cold_after_every_step -- --nocapture
RIPPLE_PROPERTY_SEED=42 RIPPLE_PROPERTY_STEPS=64 cargo test --locked -p ripple-index --test properties_basic generated_edit_sequences_equal_cold_after_every_step -- --nocapture
```

On PowerShell, set `$env:RIPPLE_PROPERTY_SEED='42'` and run the command without the assignment prefix. Default CI must leave these overrides unset to run all pinned cases.

Each edit is compared with a fresh cold index in a separate directory. The suite then reopens the DB and performs a no-op refresh, checking full analysis equality and zero no-op parses. Deliberate edge deletion, confidence mutation and stale diagnostic injection must produce a mismatch with a replay seed.

Real equality failures report the first divergent seed/step and can save the bounded current inputs/error under an explicitly selected `RIPPLE_PROPERTY_FAILURE_DIR`. Reproduce the failing prefix with `RIPPLE_PROPERTY_STEPS=step+1`, minimize it further, repair the defect and commit the minimized fixture/seed here before integration. No actual generated mismatch was found in the initial campaign; no shrunk-failure or unbounded-correctness claim is made. Longer sequences, cycles, Unicode/config/concurrent adversaries and real-history replay are M6 work.

T03 additionally introduces `resolved-body-v1`: the same eight project seeds have the missing provider present before indexing, then three stable-coordinate body edits each run through cold/reload/no-op comparison. Its 24 comparisons exercise fully resolved contexts without changing the original basic-v1 seed policy. The expanded 120-comparison guard is first run on the conservative updater before selective production code changes.

## M6 adversarial campaign

`adversarial-seeds.json` pins four seeds × 32 edits = 128 additional cold comparisons (248 including the original suites). Each 16-step cycle moves cyclic star barrels through conflicting and explicit exports, parse failure/recovery, UTF-8 comment and CRLF span shifts, alias config retargeting, ignore/unignore, delete/recreate and package-input changes. Every edit also checks persistence round-trip and no-op equality. The fault guard additionally changes a call's source span to detect stale provenance. Deterministic before-verification hooks in `incremental_tests` cover membership add/delete, config and source races with two-attempt recovery and independent cold equality; existing persistent-race/rollback and after-verification freshness tests remain required. These hooks are bounded interleavings, not exhaustive OS scheduling.

```sh
cargo test --locked -p ripple-index --test properties_basic -- --nocapture
RIPPLE_ADVERSARIAL_STEPS=256 cargo test --locked -p ripple-index --test properties_basic adversarial_sequences -- --nocapture
RIPPLE_ADVERSARIAL_SEED=42 RIPPLE_ADVERSARIAL_STEPS=64 cargo test --locked -p ripple-index --test properties_basic adversarial_sequences -- --nocapture
```

The longer campaign has 4 × 256 = 1,024 comparisons; hard limit 256 edits per seed. Environment overrides must remain unset in default CI. Set `RIPPLE_PROPERTY_FAILURE_DIR` to persist a divergent current input set. A reproduction needs the pinned seed and failing prefix length (printed in the panic); minimize before committing a regression. No mismatch found means no shrunk regression is claimed.
