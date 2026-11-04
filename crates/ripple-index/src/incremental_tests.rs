use crate::{
    build::{self, Phase},
    build_capture,
    fingerprints::CanonicalAnalysis,
    incremental::{self, RebuildReason},
    storage::{LoadedGeneration, Store},
};
use ripple_core::{index::SnapshotData, revision::SnapshotOrigin};
use std::{fs, path::Path};
fn origin(_: &Path) -> Result<SnapshotOrigin, String> {
    Ok(SnapshotOrigin::Worktree {
        head: None,
        dirty: true,
    })
}
fn fixture() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    for (p, text) in [
        (
            "lib.ts",
            include_str!("../../../fixtures/incremental/files/lib.ts"),
        ),
        (
            "caller.ts",
            include_str!("../../../fixtures/incremental/files/caller.ts"),
        ),
        (
            "main.test.ts",
            include_str!("../../../fixtures/incremental/files/main.test.ts"),
        ),
    ] {
        fs::write(d.path().join(p), text).unwrap();
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
fn canonical(g: &LoadedGeneration) -> CanonicalAnalysis {
    CanonicalAnalysis::new(&g.snapshot, g.analysis.as_ref().unwrap()).unwrap()
}
fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_name() == ".ripple" || e.file_name() == ".git" {
            continue;
        }
        let dst = to.join(e.file_name());
        if e.path().is_dir() {
            copy(&e.path(), &dst);
        } else {
            fs::copy(e.path(), dst).unwrap();
        }
    }
}
fn equals_cold(root: &Path) {
    let fresh = tempfile::tempdir().unwrap();
    copy(root, fresh.path());
    build::cold_index(fresh.path(), origin).unwrap();
    assert_eq!(canonical(&load(root)), canonical(&load(fresh.path())));
}
#[test]
fn no_op_body_add_delete_and_recovery_reuse_parse_facts_and_equal_cold() {
    let d = fixture();
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert_eq!(out.update.parsed_files, 0);
    assert_eq!(out.update.reused_files, 3);
    assert_eq!(out.update.reference_files_resolved, 0);
    equals_cold(d.path());
    fs::write(
        d.path().join("lib.ts"),
        "export function f() { return 9; }\n",
    )
    .unwrap();
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert_eq!(out.update.parsed_files, 1);
    assert_eq!(out.update.reused_files, 2);
    assert_eq!(out.update.changed[0].as_str(), "lib.ts");
    equals_cold(d.path());
    fs::write(d.path().join("extra.ts"), "export function extra() {}\n").unwrap();
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert_eq!(out.update.parsed_files, 1);
    assert_eq!(out.update.reused_files, 3);
    assert_eq!(out.update.added[0].as_str(), "extra.ts");
    equals_cold(d.path());
    fs::remove_file(d.path().join("lib.ts")).unwrap();
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert_eq!(out.update.parsed_files, 0);
    assert_eq!(out.update.deleted[0].as_str(), "lib.ts");
    assert!(out.incomplete);
    equals_cold(d.path());
    fs::write(d.path().join("caller.ts"), "export function broken( {\n").unwrap();
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert_eq!(out.update.parsed_files, 1);
    equals_cold(d.path());
}
#[test]
fn each_captured_global_input_and_ignore_edit_forces_full_parse() {
    for (path, bytes) in [
        ("tsconfig.json", b"{}".as_slice()),
        ("package.json", b"{}"),
        ("package-lock.json", b"{}"),
        ("npm-shrinkwrap.json", b"{}"),
        ("pnpm-lock.yaml", b"lockfileVersion: 9"),
        ("yarn.lock", b"# lock"),
        ("bun.lock", b"{}"),
        ("bun.lockb", b"\0\xff"),
        (".gitignore", b"# comment"),
        (".ignore", b"# comment"),
    ] {
        let d = fixture();
        fs::write(d.path().join(path), bytes).unwrap();
        let out = incremental::refresh(d.path(), origin).unwrap();
        assert_eq!(
            out.update.rebuild_reason,
            Some(RebuildReason::GlobalInputs),
            "{path}"
        );
        assert_eq!(out.update.parsed_files, 3);
        assert_eq!(out.update.reused_files, 0);
        equals_cold(d.path());
    }
    let d = fixture();
    fs::create_dir(d.path().join("settings")).unwrap();
    fs::write(
        d.path().join("tsconfig.json"),
        "{\"extends\":\"./settings/base.json\"}",
    )
    .unwrap();
    fs::write(d.path().join("settings/base.json"), "{}").unwrap();
    incremental::refresh(d.path(), origin).unwrap();
    fs::write(
        d.path().join("settings/base.json"),
        "{\"compilerOptions\":{\"strict\":true}}",
    )
    .unwrap();
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert_eq!(out.update.rebuild_reason, Some(RebuildReason::GlobalInputs));
    equals_cold(d.path());
}
#[test]
fn unavailable_cache_versions_and_schema_have_explicit_safe_behaviour() {
    let d = fixture();
    let g = load(d.path());
    let mut data: SnapshotData =
        serde_json::from_value(serde_json::to_value(&g.snapshot).unwrap()).unwrap();
    data.metadata.generation = 2;
    let mut store = Store::open(d.path().join(".ripple"), build::versions()).unwrap();
    store
        .publish(&data.validate(&g.sources).unwrap(), &g.inputs)
        .unwrap();
    drop(store);
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert_eq!(
        out.update.rebuild_reason,
        Some(RebuildReason::MissingAnalysis)
    );
    assert_eq!(out.update.parsed_files, 3);
    equals_cold(d.path());
    let d = fixture();
    let g = load(d.path());
    let mut old = build::versions();
    old.analyzer_version = "old-analyzer".into();
    let mut data: SnapshotData =
        serde_json::from_value(serde_json::to_value(&g.snapshot).unwrap()).unwrap();
    data.metadata.generation = 2;
    data.metadata.versions = old.clone();
    let mut store = Store::open(d.path().join(".ripple"), old).unwrap();
    store
        .publish(&data.validate(&g.sources).unwrap(), &g.inputs)
        .unwrap();
    drop(store);
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert_eq!(
        out.update.rebuild_reason,
        Some(RebuildReason::IncompatibleVersions)
    );
    assert_eq!(out.update.parsed_files, 3);
    assert!(!out.update.source_delta_available);
    equals_cold(d.path());
    let d = fixture();
    let c = rusqlite::Connection::open(d.path().join(".ripple/index.db")).unwrap();
    let text: String = c
        .query_row(
            "SELECT metadata_json FROM generations WHERE id=1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let mut metadata: serde_json::Value = serde_json::from_str(&text).unwrap();
    metadata["versions"]["hash_version"] = serde_json::json!(99);
    c.execute(
        "UPDATE generations SET metadata_json=?1 WHERE id=1",
        [metadata.to_string()],
    )
    .unwrap();
    drop(c);
    let out = incremental::refresh(d.path(), origin).unwrap();
    assert_eq!(
        out.update.rebuild_reason,
        Some(RebuildReason::IncompatibleVersions)
    );
    assert_eq!(out.update.reused_files, 0);
    equals_cold(d.path());
    let d = fixture();
    let c = rusqlite::Connection::open(d.path().join(".ripple/index.db")).unwrap();
    c.pragma_update(None, "user_version", 1).unwrap();
    drop(c);
    let error = incremental::refresh(d.path(), origin).unwrap_err();
    assert!(error.contains("full rebuild required"));
    assert!(error.contains("archive"));
    let c = rusqlite::Connection::open(d.path().join(".ripple/index.db")).unwrap();
    assert_eq!(
        c.query_row("SELECT count(*) FROM generations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}
#[test]
fn parser_capture_and_sql_failure_preserve_the_previous_generation() {
    let d = fixture();
    let previous = load(d.path());
    let before = canonical(&previous);
    let captured = build_capture::capture(d.path(), origin).unwrap();
    let error = incremental::prepare(
        &captured,
        Some(&previous),
        Some(RebuildReason::ExplicitFull),
        |_| Err("injected hard parser failure".into()),
    )
    .unwrap_err();
    assert!(error.contains("parser failure"));
    assert_eq!(before, canonical(&load(d.path())));
    fs::write(d.path().join("lib.ts"), [0xff]).unwrap();
    assert!(incremental::refresh(d.path(), origin).is_err());
    assert_eq!(before, canonical(&load(d.path())));
    fs::write(d.path().join("lib.ts"), "export function f(){return 7;}\n").unwrap();
    let c = rusqlite::Connection::open(d.path().join(".ripple/index.db")).unwrap();
    c.execute_batch("CREATE TRIGGER fail_analysis BEFORE INSERT ON analysis_artifacts BEGIN SELECT RAISE(ABORT,'injected write failure'); END;").unwrap();
    drop(c);
    assert!(incremental::refresh(d.path(), origin).is_err());
    assert_eq!(before, canonical(&load(d.path())));
    let c = rusqlite::Connection::open(d.path().join(".ripple/index.db")).unwrap();
    assert_eq!(
        c.query_row("SELECT count(*) FROM generations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}
#[test]
fn recapture_retries_account_for_actual_parse_work_and_never_publish_mixed_inputs() {
    let d = fixture();
    fs::write(d.path().join("lib.ts"), "export function f(){return 2;}\n").unwrap();
    let out = build::refresh_inner(d.path(), origin, |attempt, phase| {
        if attempt == 1 && matches!(phase, Phase::BeforeVerification) {
            fs::write(d.path().join("lib.ts"), "export function f(){return 3;}\n").unwrap();
        }
    })
    .unwrap();
    assert_eq!(out.attempts, 2);
    assert_eq!(out.update.parsed_files, 1);
    assert_eq!(out.update.total_parsed_files, 2);
    equals_cold(d.path());
    let before = canonical(&load(d.path()));
    let out = build::refresh_inner(d.path(), origin, |attempt, phase| {
        if matches!(phase, Phase::BeforeVerification) {
            fs::write(d.path().join(".ignore"), format!("# {attempt}\n")).unwrap();
        }
    });
    assert!(out.is_err());
    assert_eq!(before, canonical(&load(d.path())));
    build::refresh_inner(d.path(), origin, |_, phase| {
        if matches!(phase, Phase::AfterVerification) {
            fs::write(d.path().join("lib.ts"), "export function f(){return 4;}\n").unwrap();
        }
    })
    .unwrap();
    assert!(
        build::load_current(d.path(), origin)
            .unwrap_err()
            .contains("stale")
    );
}
