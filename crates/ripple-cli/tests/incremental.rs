mod support;
use ripple_index::{build, fingerprints::CanonicalAnalysis, storage::Store};
use serde_json::Value;
use std::{
    fs,
    process::{Command, Output},
    time::Duration,
};
use support::{GitRepo, run_with_timeout};
fn call(repo: &GitRepo, args: &[&str]) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ripple"));
    c.current_dir(repo.root()).args(args);
    run_with_timeout(c, Duration::from_secs(90)).unwrap()
}
fn json(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn fixture() -> GitRepo {
    let r = GitRepo::new().unwrap();
    r.write(".gitignore", ".ripple/\n").unwrap();
    r.write("lib.ts", "export function f() { return 1; }\n")
        .unwrap();
    r.write(
        "caller.ts",
        "import { f } from './lib'; export function caller() { return f(); }\n",
    )
    .unwrap();
    r.write(
        "main.test.ts",
        "import { caller } from './caller'; caller();\n",
    )
    .unwrap();
    r.commit_all("fixture").unwrap();
    r
}
fn canonical(r: &GitRepo) -> CanonicalAnalysis {
    let g = Store::open(r.root().join(".ripple"), build::versions())
        .unwrap()
        .load()
        .unwrap()
        .unwrap();
    CanonicalAnalysis::new(&g.snapshot, g.analysis.as_ref().unwrap()).unwrap()
}
fn equals_full(r: &GitRepo) {
    let before = canonical(r);
    let out = json(call(r, &["index", "--full", "--format", "json"]));
    assert_eq!(out["data"]["update"]["rebuild_reason"], "explicit_full");
    assert_eq!(before, canonical(r));
}
#[test]
fn fresh_no_op_body_signature_config_and_force_full_report_actual_work() {
    let r = fixture();
    let index = fs::read(r.root().join(".git/index")).unwrap();
    let first = json(call(&r, &["index", "--format", "json"]));
    assert_eq!(first["data"]["update"]["rebuild_reason"], "empty_index");
    assert_eq!(first["data"]["update"]["parsed_files"], 3);
    let before = canonical(&r);
    let no_op = json(call(&r, &["index", ".", "--format", "json"]));
    assert_eq!(no_op["data"]["update"]["parsed_files"], 0);
    assert_eq!(no_op["data"]["update"]["reused_files"], 3);
    assert_eq!(no_op["data"]["update"]["module_files_resolved"], 0);
    assert_eq!(no_op["data"]["update"]["reference_files_resolved"], 0);
    assert_eq!(before, canonical(&r));
    r.write("lib.ts", "export function f() { return 2; }\n")
        .unwrap();
    let out = json(call(&r, &["index", "--format", "json"]));
    assert_eq!(out["data"]["update"]["parsed_files"], 1);
    assert_eq!(out["data"]["update"]["reused_files"], 2);
    assert_eq!(out["data"]["update"]["module_files_resolved"], 0);
    assert_eq!(out["data"]["update"]["reference_files_resolved"], 1);
    equals_full(&r);
    r.write("lib.ts", "export function f(x?: number) { return 2; }\n")
        .unwrap();
    let out = json(call(&r, &["index", "--format", "json"]));
    assert_eq!(out["data"]["update"]["parsed_files"], 1);
    assert_eq!(out["data"]["update"]["module_files_resolved"], 3);
    equals_full(&r);
    r.write("tsconfig.json", "{\"compilerOptions\":{\"strict\":true}}")
        .unwrap();
    let out = json(call(&r, &["index", "--format", "json"]));
    assert_eq!(out["data"]["update"]["rebuild_reason"], "global_inputs");
    assert_eq!(out["data"]["update"]["parsed_files"], 3);
    assert_eq!(out["data"]["update"]["reused_files"], 0);
    equals_full(&r);
    let full = json(call(&r, &["index", ".", "--full", "--format", "json"]));
    assert_eq!(full["data"]["update"]["parsed_files"], 3);
    assert_eq!(full["data"]["update"]["reference_files_resolved"], 3);
    assert_eq!(index, fs::read(r.root().join(".git/index")).unwrap());
}
#[test]
fn failed_refresh_keeps_previous_generation_and_stale_queries_require_refresh() {
    let r = fixture();
    let first = json(call(&r, &["index", "--format", "json"]));
    let before = canonical(&r);
    let text = fs::read(r.root().join("lib.ts")).unwrap();
    r.write("lib.ts", [0xff]).unwrap();
    let fail = call(&r, &["index", "--format", "json"]);
    assert_eq!(fail.status.code(), Some(1));
    assert!(fail.stdout.is_empty());
    assert_eq!(before, canonical(&r));
    r.write("lib.ts", &text).unwrap();
    let status = json(call(&r, &["status", "--format", "json"]));
    assert_eq!(
        status["data"]["metadata"]["generation"],
        first["data"]["generation"]
    );
    r.write("lib.ts", "export function f() { return 3; }\n")
        .unwrap();
    let stale = call(&r, &["status", "--format", "json"]);
    assert!(!stale.status.success());
    assert!(stale.stdout.is_empty());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("stale_index"));
    let impact = call(&r, &["impact", "f"]);
    assert!(!impact.status.success());
    json(call(&r, &["index", "--format", "json"]));
    assert_eq!(
        json(call(&r, &["status", "--format", "json"]))["data"]["freshness"],
        "verified"
    );
    equals_full(&r);
}
#[test]
fn human_help_and_json_distinguish_full_rebuild_from_incremental_refresh() {
    let r = fixture();
    let help = call(&r, &["index", "--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--full"));
    let first = call(&r, &["index"]);
    assert!(first.status.success());
    let text = String::from_utf8(first.stdout).unwrap();
    for label in [
        "Update strategy:",
        "Full rebuild reason:",
        "Accepted attempt:",
        "Module analysis:",
        "Reference analysis:",
        "Source changes:",
    ] {
        assert!(text.contains(label));
    }
    let warm = call(&r, &["index"]);
    assert!(warm.status.success());
    assert!(String::from_utf8_lossy(&warm.stdout).contains("0 files parsed, 3 parse facts reused"));
    let out = json(call(&r, &["--format", "json", "index", "--full"]));
    assert_eq!(out["format_version"], 1);
    assert_eq!(out["command"], "index");
    assert_eq!(out["data"]["update"]["strategy"], "full");
    let bad = call(&r, &["index", "--full=maybe"]);
    assert_eq!(bad.status.code(), Some(2));
    assert!(bad.stdout.is_empty());
}

#[test]
fn force_full_does_not_silently_replace_an_unsupported_database_format() {
    use std::io::{Seek, SeekFrom, Write};
    let r = fixture();
    json(call(&r, &["index", "--format", "json"]));
    let mut db = fs::OpenOptions::new()
        .write(true)
        .open(r.root().join(".ripple/index.db"))
        .unwrap();
    db.seek(SeekFrom::Start(60)).unwrap();
    db.write_all(&999u32.to_be_bytes()).unwrap();
    drop(db);
    let out = call(&r, &["index", "--full", "--format", "json"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(text.contains("full rebuild required"));
    assert!(text.contains("archive"));
    let data = fs::read(r.root().join(".ripple/index.db")).unwrap();
    assert_eq!(&data[60..64], &999u32.to_be_bytes());
}
