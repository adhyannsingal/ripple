mod support;
use serde_json::Value;
use std::process::{Command, Output};
use support::{GitRepo, run};
fn call(repo: &GitRepo, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ripple"));
    command.current_dir(repo.root()).args(args);
    run(command).unwrap()
}
fn repo() -> GitRepo {
    let r = GitRepo::new().unwrap();
    r.write(".gitignore", ".ripple/\n").unwrap();
    r.write(
        "lib.ts",
        "export function a() { return 1; } export function b() { return 2; }",
    )
    .unwrap();
    r.write(
        "caller.ts",
        "import { a } from './lib'; export function caller() { return a(); }",
    )
    .unwrap();
    r.write("a.test.ts", "import { caller } from './caller'; caller();")
        .unwrap();
    r.write("b.test.ts", "import { b } from './lib'; b();")
        .unwrap();
    r.commit_all("fixture").unwrap();
    r
}
fn json(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn cold_index_status_and_impact_reopen_with_both_evidence_tracks() {
    let r = repo();
    let index = json(call(&r, &["index", ".", "--format", "json"]));
    assert_eq!(index["data"]["counts"]["files"], 4);
    assert_eq!(index["data"]["generation"], 1);
    assert_eq!(
        json(call(&r, &["status", "--format", "json"]))["data"]["freshness"],
        "verified"
    );
    let impact = json(call(
        &r,
        &["impact", "a", "--format", "json", "--limit", "0"],
    ));
    let tests = &impact["data"]["result"]["tests"];
    assert_eq!(tests["symbol_backed"].as_array().unwrap().len(), 1);
    assert_eq!(tests["fallback_only"].as_array().unwrap().len(), 1);
    assert_eq!(tests["combined"].as_array().unwrap().len(), 2);
    assert!(
        impact["data"]["result"]["impact"]["truncation"]["display_omitted"]
            .as_u64()
            .unwrap()
            > 0
    );
    let human = call(&r, &["impact", "a"]);
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(text.contains("SymbolBacked witness"));
    assert!(text.contains("ModuleFallback witness"));
    assert!(text.contains("Potentially related tests"));
    assert!(text.contains("configured_import_binding"));
}
#[test]
fn stale_content_membership_and_ambiguity_are_actionable_errors() {
    let r = repo();
    json(call(&r, &["index", ".", "--format", "json"]));
    r.write("extra.ts", "export function a() {}").unwrap();
    let stale = call(&r, &["status"]);
    assert!(!stale.status.success());
    assert!(stale.stdout.is_empty());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("stale_index"));
    json(call(&r, &["index", ".", "--format", "json"]));
    let ambiguous = call(&r, &["impact", "a"]);
    assert!(!ambiguous.status.success());
    assert!(String::from_utf8_lossy(&ambiguous.stderr).contains("ambiguous_symbol"));
    assert!(call(&r, &["impact", "lib.ts::a"]).status.success());
    r.write(
        "lib.ts",
        "export function a(){ return 99; } export function b() {} ",
    )
    .unwrap();
    assert!(!call(&r, &["impact", "lib.ts::a"]).status.success());
}
#[test]
fn missing_index_never_produces_success_data() {
    let r = repo();
    let missing = call(&r, &["status", "--format", "json"]);
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
    assert!(!r.root().join(".ripple").exists());
}
#[test]
fn git_redirection_environment_does_not_select_another_repository() {
    let r = repo();
    let mut command = Command::new(env!("CARGO_BIN_EXE_ripple"));
    command
        .current_dir(r.root())
        .args(["index", ".", "--format", "json"])
        .env("GIT_DIR", r.root().join("not-a-repo"))
        .env("git_work_tree", r.root().join("not-a-repo"))
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "core.fsmonitor")
        .env("GIT_CONFIG_VALUE_0", "echo NOT_ALLOWED");
    let result = json(run(command).unwrap());
    assert_eq!(result["data"]["counts"]["files"], 4);
}
#[test]
fn schema_mismatch_is_rejected_with_refresh_instructions_without_success_output() {
    use std::io::{Seek, SeekFrom, Write};
    let r = repo();
    json(call(&r, &["index", ".", "--format", "json"]));
    let mut db = std::fs::OpenOptions::new()
        .write(true)
        .open(r.root().join(".ripple/index.db"))
        .unwrap();
    db.seek(SeekFrom::Start(60)).unwrap();
    db.write_all(&999u32.to_be_bytes()).unwrap();
    drop(db);
    let out = call(&r, &["status", "--format", "json"]);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("incompatible"));
}
#[test]
fn package_policy_is_visible_and_blocks_false_precise_imports() {
    let r = repo();
    r.write(
        "package.json",
        "{\"type\":\"module\",\"exports\":\"./lib.ts\"}",
    )
    .unwrap();
    let indexed = json(call(&r, &["index", ".", "--format", "json"]));
    assert_eq!(indexed["incomplete_analysis"], true);
    assert!(
        indexed["data"]["config_diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "unsupported_package_policy")
    );
    let impact = json(call(&r, &["impact", "a", "--format", "json"]));
    assert_eq!(
        impact["data"]["result"]["tests"]["symbol_backed"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}
#[test]
fn index_never_rewrites_the_users_git_index_or_tracked_files() {
    let r = repo();
    let before = std::fs::read(r.root().join(".git/index")).unwrap();
    let source = std::fs::read(r.root().join("lib.ts")).unwrap();
    json(call(&r, &["index", ".", "--format", "json"]));
    assert_eq!(before, std::fs::read(r.root().join(".git/index")).unwrap());
    assert_eq!(source, std::fs::read(r.root().join("lib.ts")).unwrap());
}
