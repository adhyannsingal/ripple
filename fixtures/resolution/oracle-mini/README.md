# Oracle-backed resolver composition regression

These four sources are byte-identical to the owned evaluation mini corpus. Manually expected pairs: `main.ts::caller → lib.ts::café` CALLS (two occurrences), `main.ts::reference → lib.ts::café` REFERENCES, `view.tsx::View → main.ts::caller` CALLS. The alias crosses a re-export barrel and targets a single overloaded implementation. A parameter shadows the import in `shadow`; no corresponding symbol edge is permitted. Astral/non-ASCII text precedes the call sites.

M3-T03 observed no supported mismatch in either the 3-edge mini or 16-edge external projection. This is a stronger composition/provenance regression, not a claimed resolver fix or expansion. `crates/ripple-resolve/tests/calls.rs` asserts the exact pairs, occurrence byte slices, confidence/rule and overload declaration span. Evaluation fixtures should stay byte-identical; the test checks that equality.
