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
