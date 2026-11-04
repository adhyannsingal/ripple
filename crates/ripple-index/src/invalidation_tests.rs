use crate::{
    build,
    fingerprints::CanonicalAnalysis,
    incremental,
    storage::{LoadedGeneration, Store},
};
use ripple_core::{
    graph::{Confidence, EdgeKind, GraphSnapshot},
    index::SnapshotData,
    revision::SnapshotOrigin,
};
use std::{fs, path::Path};
fn origin(_: &Path) -> Result<SnapshotOrigin, String> {
    Ok(SnapshotOrigin::Worktree {
        head: None,
        dirty: true,
    })
}
fn root(files: &[(&str, &str)]) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    for (p, s) in files {
        fs::write(d.path().join(p), s).unwrap();
    }
    build::cold_index(d.path(), origin).unwrap();
    d
}
fn load(root: &Path) -> LoadedGeneration {
    Store::open(root.join(".ripple"), build::versions())
        .unwrap()
        .load()
        .unwrap()
        .unwrap()
}
fn equal_cold(root: &Path) {
    let fresh = tempfile::tempdir().unwrap();
    for e in fs::read_dir(root).unwrap() {
        let e = e.unwrap();
        if e.path().is_file() {
            fs::copy(e.path(), fresh.path().join(e.file_name())).unwrap();
        }
    }
    build::cold_index(fresh.path(), origin).unwrap();
    let a = load(root);
    let b = load(fresh.path());
    assert_eq!(
        CanonicalAnalysis::new(&a.snapshot, a.analysis.as_ref().unwrap()).unwrap(),
        CanonicalAnalysis::new(&b.snapshot, b.analysis.as_ref().unwrap()).unwrap()
    );
}
fn fixture() -> tempfile::TempDir {
    root(&[
        ("lib.ts", "export function f() { return 1; }\n"),
        (
            "caller.ts",
            "import { f } from './lib'; export function caller() { return f(); }\n",
        ),
        (
            "main.test.ts",
            "import { caller } from './caller'; caller();\n",
        ),
    ])
}
#[test]
fn stable_body_cutoff_avoids_importer_module_and_reference_resolution() {
    let d = fixture();
    fs::write(
        d.path().join("lib.ts"),
        "export function f() { return 2; }\n",
    )
    .unwrap();
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert_eq!(out.update.parsed_files, 1);
    assert!(out.update.module_cache_reused);
    assert_eq!(out.update.module_files_resolved, 0);
    assert_eq!(out.update.reference_files_resolved, 1);
    assert_eq!(out.update.reference_files_reused, 2);
    assert!(
        out.update
            .invalidation_candidates
            .iter()
            .any(|p| p.as_str() == "main.test.ts")
    );
    assert!(
        out.update
            .unchanged_external_surfaces
            .iter()
            .any(|p| p.as_str() == "lib.ts")
    );
    equal_cold(d.path());
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert_eq!(out.update.reference_files_resolved, 0);
    assert_eq!(out.update.module_files_resolved, 0);
    equal_cold(d.path());
    fs::write(
        d.path().join("lib.ts"),
        "export function f() { return 12345; }\n",
    )
    .unwrap();
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert!(!out.update.module_cache_reused);
    assert_eq!(out.update.module_files_resolved, 3);
    equal_cold(d.path());
}
#[test]
fn signature_exports_delete_rename_and_missing_targets_match_cold() {
    let d = fixture();
    for text in [
        "export function f(x?: number) { return 2; }\n",
        "export function renamed() { return 2; }\n",
        "export function f() { return 3; }\n",
    ] {
        fs::write(d.path().join("lib.ts"), text).unwrap();
        let out = incremental::refresh(d.path(), origin).unwrap();
        assert!(!out.update.module_cache_reused);
        equal_cold(d.path());
    }
    fs::rename(d.path().join("lib.ts"), d.path().join("moved.ts")).unwrap();
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert!(out.incomplete);
    equal_cold(d.path());
    fs::rename(d.path().join("moved.ts"), d.path().join("lib.ts")).unwrap();
    incremental::refresh(d.path(), origin).unwrap();
    equal_cold(d.path());
    fs::remove_file(d.path().join("lib.ts")).unwrap();
    incremental::refresh(d.path(), origin).unwrap();
    equal_cold(d.path());
    fs::write(
        d.path().join("lib.ts"),
        "export function f() { return 4; }\n",
    )
    .unwrap();
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert!(!out.incomplete);
    assert!(out.update.reference_files_resolved >= 1);
    equal_cold(d.path());
}
#[test]
fn cyclic_ambiguous_reexports_classes_and_alias_changes_keep_fixed_point_evidence() {
    let d = root(&[
        ("lib.ts", "export function f() { return 1; }\n"),
        ("a.ts", "export * from './b'; export { f } from './lib';\n"),
        ("b.ts", "export * from './a';\n"),
        ("main.test.ts", "import { f } from './b'; f();\n"),
    ]);
    fs::write(
        d.path().join("lib.ts"),
        "export function f(x?: number) { return 1; }\n",
    )
    .unwrap();
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert_eq!(out.update.invalidation_candidates.len(), 4);
    equal_cold(d.path());
    let d = root(&[
        ("left.ts", "export function f() {}\n"),
        ("right.ts", "export function f() {}\n"),
        (
            "barrel.ts",
            "export * from './left'; export * from './right';\n",
        ),
        ("main.test.ts", "import { f } from './barrel'; f();\n"),
    ]);
    assert!(load(d.path()).analysis.unwrap().modules.incomplete);
    fs::write(d.path().join("right.ts"), "export function g() {}\n").unwrap();
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert!(!out.incomplete);
    equal_cold(d.path());
    let d = root(&[
        ("lib.ts", "export class Box { method() { return 1; } }\n"),
        ("main.test.ts", "import { Box } from './lib'; Box;\n"),
    ]);
    for s in [
        "export class Box { method() { return 2; } }\n",
        "export class Box { method(x?: number) { return 2; } other() {} }\n",
    ] {
        fs::write(d.path().join("lib.ts"), s).unwrap();
        incremental::refresh(d.path(), origin).unwrap();
        equal_cold(d.path());
    }
    let d = root(&[
        (
            "tsconfig.json",
            "{\"compilerOptions\":{\"paths\":{\"@lib\":[\"./one\"]}}}",
        ),
        ("one.ts", "export function f() {}\n"),
        ("two.ts", "export function f() { return 2; }\n"),
        ("main.test.ts", "import { f } from '@lib'; f();\n"),
    ]);
    fs::write(
        d.path().join("tsconfig.json"),
        "{\"compilerOptions\":{\"paths\":{\"@lib\":[\"./two\"]}}}",
    )
    .unwrap();
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert!(out.update.rebuild_reason.is_some());
    assert!(!out.update.module_cache_reused);
    equal_cold(d.path());
}
#[test]
fn candidates_are_re_resolved_and_unknowns_are_not_silently_lost() {
    let d = fixture();
    let g = load(d.path());
    let mut data: SnapshotData =
        serde_json::from_value(serde_json::to_value(&g.snapshot).unwrap()).unwrap();
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
    data.metadata.generation = 2;
    let snapshot = data.validate(&g.sources).unwrap();
    Store::open(d.path().join(".ripple"), build::versions())
        .unwrap()
        .publish_analysis(&snapshot, &g.inputs, g.analysis.as_ref())
        .unwrap();
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert!(out.update.reference_files_resolved >= 1);
    equal_cold(d.path());
    let d = root(&[
        ("lib.ts", "export function f() { return 1; }\n"),
        (
            "main.test.ts",
            "import { f } from './lib'; f(); dynamic();\n",
        ),
    ]);
    let before = load(d.path()).snapshot.graph().unresolved().len();
    assert!(before > 0);
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert!(out.incomplete);
    assert_eq!(load(d.path()).snapshot.graph().unresolved().len(), before);
    equal_cold(d.path());
    fs::write(
        d.path().join("lib.ts"),
        "export function f() { unknown(); return 2; }\n",
    )
    .unwrap();
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert!(out.incomplete);
    equal_cold(d.path());
}
