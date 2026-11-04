//! Deterministic generated equality guard; no selective cutoff is allowed before this passes.
use ripple_core::{
    graph::{Confidence, EdgeKind, GraphSnapshot},
    index::SnapshotData,
    revision::SnapshotOrigin,
    syntax::{Diagnostic, Severity},
};
use ripple_index::{
    build,
    fingerprints::CanonicalAnalysis,
    incremental,
    storage::{LoadedGeneration, Store},
};
use std::{collections::BTreeMap, fs, path::Path};
const POLICY: &str = "ripple-property-basic-v1";
fn origin(_: &Path) -> Result<SnapshotOrigin, String> {
    Ok(SnapshotOrigin::Worktree {
        head: None,
        dirty: true,
    })
}
fn load(root: &Path) -> LoadedGeneration {
    Store::open(root.join(".ripple"), build::versions())
        .unwrap()
        .load()
        .unwrap()
        .unwrap()
}
fn canonical(g: &LoadedGeneration) -> CanonicalAnalysis {
    CanonicalAnalysis::new(&g.snapshot, g.analysis.as_ref().unwrap()).unwrap()
}
fn write(root: &Path, path: &str, text: &str) {
    fs::write(root.join(path), text).unwrap();
}
fn project(root: &Path, seed: u64) -> usize {
    let width = 3 + (seed % 4) as usize;
    write(root, "m0.ts", "export function f0() { return 0; }\n");
    for i in 1..width {
        write(
            root,
            &format!("m{i}.ts"),
            &format!(
                "import {{ f{} }} from './m{}';\nexport function f{i}() {{ return f{}(); }}\n",
                i - 1,
                i - 1,
                i - 1
            ),
        );
    }
    write(
        root,
        "barrel.ts",
        &format!(
            "export {{ f{} as f0 }} from './m{}';\n",
            width - 1,
            width - 1
        ),
    );
    write(
        root,
        "main.test.ts",
        "import { f0 } from './barrel'; f0();\n",
    );
    write(
        root,
        "consumer.ts",
        "import { late } from './late'; export function consume() { return late(); }\n",
    );
    width
}
fn inputs(root: &Path) -> BTreeMap<String, String> {
    fs::read_dir(root)
        .unwrap()
        .filter_map(|e| {
            let e = e.unwrap();
            e.path().is_file().then(|| {
                (
                    e.file_name().to_string_lossy().into_owned(),
                    fs::read_to_string(e.path()).unwrap(),
                )
            })
        })
        .collect()
}
fn mismatch(
    seed: u64,
    step: usize,
    actual: &LoadedGeneration,
    expected: &LoadedGeneration,
) -> Result<(), String> {
    if canonical(actual) == canonical(expected) {
        return Ok(());
    }
    let parts = ripple_core::canonical::CanonicalState::from_snapshot(&actual.snapshot)
        .differences(&ripple_core::canonical::CanonicalState::from_snapshot(
            &expected.snapshot,
        ));
    Err(format!(
        "{POLICY} mismatch seed={seed} step={step}; snapshot sections={parts:?}; module/config/discovery evidence also compared. Reproduce: RIPPLE_PROPERTY_SEED={seed} cargo test -p ripple-index --test properties_basic generated_edit_sequences_equal_cold_after_every_step -- --nocapture"
    ))
}
fn independent(root: &Path) -> (tempfile::TempDir, LoadedGeneration) {
    let cold = tempfile::tempdir().unwrap();
    for (p, text) in inputs(root) {
        write(cold.path(), &p, &text);
    }
    build::cold_index(cold.path(), origin).unwrap();
    let loaded = load(cold.path());
    (cold, loaded)
}
#[test]
fn generated_edit_sequences_equal_cold_after_every_step() {
    let bounds: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/incremental/regressions/basic-seeds.json"
    ))
    .unwrap();
    assert_eq!(bounds["policy"], POLICY);
    let seeds = match std::env::var("RIPPLE_PROPERTY_SEED") {
        Ok(s) => vec![
            s.parse::<u64>()
                .expect("RIPPLE_PROPERTY_SEED must be a decimal u64"),
        ],
        Err(_) => bounds["seeds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_u64().unwrap())
            .collect(),
    };
    let steps = std::env::var("RIPPLE_PROPERTY_STEPS")
        .map(|s| s.parse::<usize>().expect("invalid step bound"))
        .unwrap_or(bounds["steps"].as_u64().unwrap() as usize);
    assert!((1..=bounds["maximum_steps"].as_u64().unwrap() as usize).contains(&steps));
    for &seed in &seeds {
        let root = tempfile::tempdir().unwrap();
        let width = project(root.path(), seed);
        incremental::refresh(root.path(), origin).unwrap();
        let mut body = 0;
        let mut signature = false;
        let mut state = seed;
        for step in 0..steps {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            match step % 6 {
                0 => {
                    body = step + 1;
                }
                1 => {
                    signature = !signature;
                }
                2 => write(
                    root.path(),
                    "late.ts",
                    &format!("export function late() {{ return {}; }}\n", state % 100),
                ),
                3 => {
                    let p = root.path().join("late.ts");
                    if p.exists() {
                        fs::remove_file(p).unwrap();
                    }
                }
                4 => {
                    let target = (state % (width as u64)) as usize;
                    write(
                        root.path(),
                        "barrel.ts",
                        &format!("export {{ f{target} as f0 }} from './m{target}';\n"),
                    );
                }
                _ => {
                    let p = root.path().join("extra.ts");
                    if p.exists() {
                        fs::remove_file(p).unwrap();
                    } else {
                        write(
                            root.path(),
                            "extra.ts",
                            &format!("export function extra() {{ return {}; }}\n", state % 100),
                        );
                    }
                }
            }
            if step % 6 <= 1 {
                write(
                    root.path(),
                    "m0.ts",
                    &format!(
                        "export function f0({}) {{ return {body}; }}\n",
                        if signature { "x?: number" } else { "" }
                    ),
                );
            }
            let outcome = incremental::refresh(root.path(), origin)
                .unwrap_or_else(|e| panic!("{POLICY} seed={seed} step={step}: {e}"));
            assert_eq!(
                outcome.update.parsed_files + outcome.update.reused_files,
                outcome.counts.files
            );
            let updated = load(root.path());
            let (_copy, cold) = independent(root.path());
            if let Err(error) = mismatch(seed, step, &updated, &cold) {
                if let Ok(dest) = std::env::var("RIPPLE_PROPERTY_FAILURE_DIR") {
                    let dest = std::path::PathBuf::from(dest);
                    fs::create_dir_all(&dest).unwrap();
                    let case = serde_json::json!({"policy":POLICY,"seed":seed,"step":step,"inputs":inputs(root.path()),"error":error});
                    fs::write(
                        dest.join(format!("seed-{seed}-step-{step}.json")),
                        serde_json::to_vec_pretty(&case).unwrap(),
                    )
                    .unwrap();
                }
                panic!("{error}");
            }
            assert_eq!(
                canonical(&updated),
                canonical(&load(root.path())),
                "reload seed={seed} step={step}"
            );
            let no_op = incremental::refresh(root.path(), origin).unwrap();
            assert_eq!(
                no_op.update.parsed_files, 0,
                "no-op seed={seed} step={step}"
            );
            assert_eq!(
                canonical(&updated),
                canonical(&load(root.path())),
                "idempotence seed={seed} step={step}"
            );
        }
    }
    println!(
        "{POLICY}: {} seeds x {steps} edits = {} cold comparisons, plus reload/idempotence checks",
        seeds.len(),
        seeds.len() * steps
    );
}
#[test]
fn deliberately_stale_edges_confidence_and_diagnostics_are_detected_with_reproducible_seed() {
    let root = tempfile::tempdir().unwrap();
    project(root.path(), 42);
    incremental::refresh(root.path(), origin).unwrap();
    let expected = load(root.path());
    for fault in 0..3 {
        let mut actual = load(root.path());
        let mut data: SnapshotData =
            serde_json::from_value(serde_json::to_value(&actual.snapshot).unwrap()).unwrap();
        match fault {
            0 => {
                data.graph = GraphSnapshot::new(
                    data.graph.nodes().to_vec(),
                    data.graph
                        .edges()
                        .iter()
                        .filter(|e| e.kind != EdgeKind::Calls)
                        .cloned()
                        .collect(),
                    data.graph.unresolved().to_vec(),
                )
                .unwrap();
            }
            1 => {
                let mut edges = data.graph.edges().to_vec();
                edges
                    .iter_mut()
                    .find(|e| e.kind == EdgeKind::Calls)
                    .unwrap()
                    .confidence = Confidence::Candidate;
                data.graph = GraphSnapshot::new(
                    data.graph.nodes().to_vec(),
                    edges,
                    data.graph.unresolved().to_vec(),
                )
                .unwrap();
            }
            _ => data.files[0].diagnostics.push(Diagnostic {
                code: "injected_stale_diagnostic".into(),
                severity: Severity::Warning,
                message: "deliberate test fault".into(),
                span: None,
            }),
        }
        actual.snapshot = data.validate(&actual.sources).unwrap();
        let error = mismatch(42, 7, &actual, &expected).unwrap_err();
        assert!(error.contains("seed=42 step=7"));
        assert!(error.contains("RIPPLE_PROPERTY_SEED=42"));
    }
}

#[test]
fn resolved_body_sequences_equal_cold_before_and_after_cutoff() {
    let seeds: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/incremental/regressions/basic-seeds.json"
    ))
    .unwrap();
    for seed in seeds["seeds"].as_array().unwrap() {
        let seed = seed.as_u64().unwrap();
        let root = tempfile::tempdir().unwrap();
        project(root.path(), seed);
        write(
            root.path(),
            "late.ts",
            "export function late() { return 1; }\n",
        );
        incremental::refresh(root.path(), origin).unwrap();
        for body in 1..=3 {
            write(
                root.path(),
                "m0.ts",
                &format!("export function f0() {{ return {body}; }}\n"),
            );
            incremental::refresh(root.path(), origin).unwrap();
            let actual = load(root.path());
            let (_copy, cold) = independent(root.path());
            mismatch(seed, body, &actual, &cold).unwrap();
            assert_eq!(canonical(&actual), canonical(&load(root.path())));
            let outcome = incremental::refresh(root.path(), origin).unwrap();
            assert_eq!(outcome.update.parsed_files, 0);
            assert_eq!(canonical(&actual), canonical(&load(root.path())));
        }
    }
    println!(
        "resolved-body-v1: 8 seeds x 3 edits = 24 cold comparisons, plus reload/idempotence checks"
    );
}
