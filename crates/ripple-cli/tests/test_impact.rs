mod support;
use serde_json::{Value, json};
use std::{
    fs,
    process::{Command, Output},
    time::Duration,
};
use support::{GitRepo, run_with_timeout};
fn call(repo: &GitRepo, args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ripple"));
    cmd.current_dir(repo.root()).args(args);
    run_with_timeout(cmd, Duration::from_secs(90)).unwrap()
}
fn document(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn commit(repo: &GitRepo) -> String {
    repo.git(&["add", "--all"]).unwrap();
    repo.git(&["commit", "-qm", "fixture", "--allow-empty"])
        .unwrap();
    String::from_utf8(repo.git(&["rev-parse", "HEAD"]).unwrap().stdout)
        .unwrap()
        .trim()
        .into()
}
fn pair(case: &Value) -> (GitRepo, String) {
    let r = GitRepo::new().unwrap();
    r.write(
        ".gitattributes",
        "*.ts text eol=lf\n*.tsx text eol=lf\n*.json text eol=lf\n",
    )
    .unwrap();
    for (path, text) in case["before"].as_object().unwrap() {
        r.write(path, text.as_str().unwrap()).unwrap();
    }
    let a = commit(&r);
    for path in case["before"].as_object().unwrap().keys() {
        fs::remove_file(r.root().join(path)).unwrap();
    }
    for (path, text) in case["after"].as_object().unwrap() {
        r.write(path, text.as_str().unwrap()).unwrap();
    }
    let b = commit(&r);
    (r, format!("{a}..{b}"))
}

fn verify_ratios(data: &Value) {
    let selection = &data["selection"];
    let total = data["counts"]["total_head_test_files"].as_u64().unwrap();
    for key in ["symbol_backed", "combined", "module_fallback"] {
        let count = selection["active_head"][key].as_array().unwrap().len() as u64;
        let ratio = &data["selection_ratios"][key];
        assert_eq!(ratio["numerator"], count);
        assert_eq!(ratio["denominator"], total);
        assert!(count <= total);
        if total == 0 {
            assert!(ratio["fraction"].is_null());
        } else {
            assert_eq!(
                ratio["fraction"].as_f64().unwrap(),
                count as f64 / total as f64
            );
        }
    }
    assert_eq!(data["sufficiency_guaranteed"], false);
}
#[test]
fn command_preserves_partitions_witnesses_ratios_and_display_independence() {
    let cases: Value =
        serde_json::from_str(include_str!("../../../fixtures/test-impact/cases.json")).unwrap();
    for case in cases
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["candidate"] != true && c["depth"].is_null())
    {
        let (repo, pair) = pair(case);
        let output = document(call(
            &repo,
            &["test", &pair, "--format", "json", "--limit", "0"],
        ));
        assert_eq!(output["command"], "test");
        let data = &output["data"];
        verify_ratios(data);
        for key in ["symbol_backed", "fallback_only"] {
            assert_eq!(
                data["selection"]["active_head"][key], case[key],
                "{} {key}",
                case["name"]
            );
        }
        assert_eq!(
            data["selection"]["base_only"]["combined"],
            case["base_only"]
        );
        assert_eq!(
            data["selection"]["present_unindexed_head"]["combined"],
            case["present_unindexed"]
        );
        if case["incomplete"] == true {
            assert_eq!(output["incomplete_analysis"], true);
        }
        assert!(
            data["selection"]["completeness"]["selected_paths_complete_within_depth"]
                .as_bool()
                .unwrap()
        );
        let again = call(&repo, &["test", &pair, "--format", "json", "--limit", "0"]);
        assert_eq!(
            serde_json::to_vec(&output).unwrap(),
            serde_json::to_vec(&serde_json::from_slice::<Value>(&again.stdout).unwrap()).unwrap()
        );
        let human = call(&repo, &["test", &pair]);
        assert!(human.status.success());
        let text = String::from_utf8(human.stdout).unwrap();
        for expected in [
            "Potentially related test files",
            "Selection ratio symbol-backed",
            "Selection ratio combined",
            "not a sufficiency guarantee",
            "No tests were executed",
        ] {
            assert!(text.contains(expected));
        }
        if !case["fallback_only"].as_array().unwrap().is_empty() {
            assert!(text.contains("[fallback-only]"));
        }
        assert!(!repo.root().join(".ripple").exists());
    }
}
#[test]
fn empty_zero_tests_added_deleted_and_depth_truncation_are_explicit() {
    let cases = [
        json!({"before":{"lib.ts":"export function f(){return 1;}"},"after":{"lib.ts":"export function f(){return 2;}"},"total":0,"selected":0,"deleted":0}),
        json!({"before":{"a.test.ts":"export function a(){return 1;}"},"after":{"a.test.ts":"export function a(){return 1;}"},"total":1,"selected":0,"deleted":0}),
        json!({"before":{"lib.ts":"export function f(){return 1;}"},"after":{"lib.ts":"export function f(){return 1;}","a.test.ts":"import {f} from './lib'; f();"},"total":1,"selected":1,"deleted":0}),
        json!({"before":{"a.test.ts":"export function a(){return 1;}"},"after":{},"total":0,"selected":0,"deleted":1}),
    ];
    for case in cases {
        let (repo, pair) = pair(&case);
        let output = document(call(&repo, &["test", &pair, "--format", "json"]));
        let data = &output["data"];
        verify_ratios(data);
        assert_eq!(data["counts"]["total_head_test_files"], case["total"]);
        assert_eq!(data["counts"]["combined_selected"], case["selected"]);
        assert_eq!(data["counts"]["base_only"], case["deleted"]);
    }
    let fixtures: Value =
        serde_json::from_str(include_str!("../../../fixtures/test-impact/cases.json")).unwrap();
    let (repo, pair) = pair(&fixtures[0]);
    let output = document(call(
        &repo,
        &["test", &pair, "--depth", "0", "--format", "json"],
    ));
    verify_ratios(&output["data"]);
    assert_eq!(output["incomplete_analysis"], true);
    assert_eq!(
        output["data"]["selection"]["completeness"]["depth_truncated"],
        true
    );
}
#[test]
fn invalid_refs_fail_without_success_and_analysis_preserves_user_state() {
    let case = json!({"before":{"lib.ts":"export function f(){return 1;}","a.test.ts":"import {f} from './lib'; f();"},"after":{"lib.ts":"export function f(){return 2;}","a.test.ts":"import {f} from './lib'; f();"}});
    let (repo, pair) = pair(&case);
    repo.write("lib.ts", "staged content").unwrap();
    repo.git(&["add", "lib.ts"]).unwrap();
    repo.write("lib.ts", "dirty content").unwrap();
    repo.write(".ripple/user-owned", "keep").unwrap();
    repo.write("untracked.ts", "export function untracked(){} ")
        .unwrap();
    let index = fs::read(repo.root().join(".git/index")).unwrap();
    let status = repo
        .git(&["status", "--porcelain=v1", "-z"])
        .unwrap()
        .stdout;
    document(call(&repo, &["test", &pair, "--format", "json"]));
    assert_eq!(index, fs::read(repo.root().join(".git/index")).unwrap());
    assert_eq!(
        status,
        repo.git(&["status", "--porcelain=v1", "-z"])
            .unwrap()
            .stdout
    );
    assert_eq!(
        fs::read_to_string(repo.root().join(".ripple/user-owned")).unwrap(),
        "keep"
    );
    assert_eq!(
        fs::read_to_string(repo.root().join("lib.ts")).unwrap(),
        "dirty content"
    );
    assert!(!repo.root().join(".ripple/index.db").exists());
    for comparison in ["A...B", "missing..HEAD", "--all..HEAD"] {
        let output = call(&repo, &["test", comparison, "--format", "json"]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}
