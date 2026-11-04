mod support;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
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
fn paths(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["identity"]["path"].as_str().unwrap().to_owned())
        .collect()
}
fn fixtures() -> Value {
    serde_json::from_str(include_str!("../../../fixtures/revision-impact/cases.json")).unwrap()
}
#[test]
fn diff_reports_all_classifications_from_independent_expected_git_cases() {
    let cases: Value =
        serde_json::from_str(include_str!("../../../fixtures/changes/cases.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let (r, pair) = pair(case);
        let output = document(call(&r, &["diff", &pair, "--format", "json"]));
        assert_eq!(output["format_version"], 1);
        assert_eq!(output["command"], "diff");
        let mut actual = output["data"]["result"]["changes"]["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                json!([
                    s["identity"]["qualified_name"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|x| x.as_str().unwrap())
                        .collect::<Vec<_>>()
                        .join("."),
                    s["kind"]
                ])
            })
            .collect::<Vec<_>>();
        let mut expected = case["expected"].as_array().unwrap().clone();
        actual.sort_by_key(Value::to_string);
        expected.sort_by_key(Value::to_string);
        assert_eq!(actual, expected, "{}", case["name"]);
        assert!(!r.root().join(".ripple").exists());
    }
}
#[test]
fn diff_preserves_revision_evidence_unknowns_test_presence_and_global_policy() {
    for case in fixtures().as_array().unwrap() {
        let (r, pair) = pair(case);
        let output = document(call(&r, &["diff", &pair, "--format", "json"]));
        let result = &output["data"]["result"];
        let evidence = result["evidence"]["symbol_backed"].as_array().unwrap();
        for (side, expected) in [("base", "base_f_test"), ("head", "head_f_test")] {
            let Some(expected) = case[expected].as_bool() else {
                continue;
            };
            let exists = evidence.iter().any(|w| {
                w["revision"] == side
                    && w["seed"]["identity"]["qualified_name"] == json!(["f"])
                    && w["target"]["identity"]["path"] == "smoke.test.ts"
            });
            assert_eq!(exists, expected, "{} {side}", case["name"]);
        }
        if case["base_only"] == true {
            assert_eq!(paths(&result["tests"]["base_only"]), vec!["smoke.test.ts"]);
        }
        if case["present_unindexed"] == true {
            assert_eq!(
                paths(&result["tests"]["present_unindexed_head"]),
                vec!["smoke.test.ts"]
            );
            assert!(paths(&result["tests"]["base_only"]).is_empty());
        }
        if case["global_invalidation"] == true {
            assert_eq!(result["changes"]["global_invalidation"], true);
            assert!(
                !result["base"]["global_fallback"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
        if case["incomplete"] == true {
            assert_eq!(output["incomplete_analysis"], true);
        }
        if case["empty"] == true {
            assert_eq!(result["rows"], json!([]));
        }
        if case["file_only"] == true {
            assert_eq!(result["evidence"]["symbol_backed"], json!([]));
            assert_eq!(
                paths(&result["tests"]["fallback_only"]),
                vec!["smoke.test.ts"]
            );
        }
        if case["config_diagnostic"] == true {
            assert_eq!(output["incomplete_analysis"], true);
            for side in ["base", "head"] {
                assert!(
                    result[side]["snapshot_diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|d| d.as_str().unwrap().contains("moduleResolution"))
                );
            }
        }
        if case["name"] == "unknown_boundary" {
            assert!(
                !result["base"]["query"]["impact"]["unresolved"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            assert!(
                !result["head"]["query"]["impact"]["unresolved"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
    }
}
#[test]
fn diff_limits_are_explicit_and_complete_json_is_repeatable() {
    let case = &fixtures()[1];
    let (r, pair) = pair(case);
    let all = call(&r, &["diff", &pair, "--format", "json"]);
    let repeated = call(&r, &["--format", "json", "diff", &pair]);
    assert!(all.status.success() && repeated.status.success());
    assert_eq!(all.stdout, repeated.stdout);
    let all: Value = serde_json::from_slice(&all.stdout).unwrap();
    let limited = document(call(
        &r,
        &["diff", &pair, "--format", "json", "--limit", "0"],
    ));
    for key in ["evidence", "rows", "tests", "changes"] {
        assert_eq!(all["data"]["result"][key], limited["data"]["result"][key]);
    }
    assert!(
        limited["data"]["result"]["display_omitted"]
            .as_u64()
            .unwrap()
            > 0
    );
    let depth = document(call(
        &r,
        &["diff", &pair, "--format", "json", "--depth", "0"],
    ));
    assert_eq!(depth["incomplete_analysis"], true);
    for side in ["base", "head"] {
        assert!(
            !depth["data"]["result"][side]["query"]["impact"]["truncation"]["depth_boundary_nodes"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
    let human = String::from_utf8(call(&r, &["diff", &pair]).stdout).unwrap();
    for text in [
        "BODY_MODIFIED",
        "Base SymbolBacked witness",
        "Head SymbolBacked witness",
        "ModuleFallback witness",
        "Potentially related test files",
        "nodes by depth",
        "Head indexed test files",
    ] {
        assert!(human.contains(text), "{text}");
    }
    let empty = document(call(&r, &["diff", "HEAD..HEAD", "--format", "json"]));
    assert_eq!(empty["data"]["result"]["rows"], json!([]));
    assert_eq!(empty["data"]["result"]["changes"]["files"], json!([]));
}
fn bytes(root: &std::path::Path) -> BTreeMap<String, Vec<u8>> {
    let mut result = BTreeMap::new();
    for entry in fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            for (p, v) in bytes(&path) {
                result.insert(format!("{name}/{p}"), v);
            }
        } else {
            result.insert(name, fs::read(path).unwrap());
        }
    }
    result
}
#[test]
fn diff_ignores_dirty_work_and_preserves_index_refs_and_existing_persistent_state() {
    let (r, pair) = pair(&fixtures()[1]);
    r.write(".gitignore", ".ripple/\n").unwrap();
    document(call(&r, &["index", ".", "--format", "json"]));
    r.write("lib.ts", "staged invalid worktree").unwrap();
    r.git(&["add", "lib.ts"]).unwrap();
    r.write("lib.ts", "dirty invalid worktree").unwrap();
    r.write("untracked.ts", "untracked invalid worktree")
        .unwrap();
    let before = bytes(r.root());
    let first = call(&r, &["diff", &pair, "--format", "json"]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(before, bytes(r.root()));
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ripple"));
    cmd.current_dir(r.root())
        .args(["diff", &pair, "--format", "json"])
        .env("GIT_DIR", "/nonexistent")
        .env("GIT_INDEX_FILE", "/nonexistent");
    let redirected = run_with_timeout(cmd, Duration::from_secs(90)).unwrap();
    assert!(redirected.status.success());
    assert_eq!(first.stdout, redirected.stdout);
    assert_eq!(before, bytes(r.root()));
}
#[test]
fn invalid_refs_comparisons_modes_and_usage_never_produce_success_data() {
    let (r, _) = pair(&fixtures()[1]);
    for (comparison, code) in [
        ("main...HEAD", "invalid_comparison"),
        ("HEAD", "invalid_comparison"),
        ("..HEAD", "invalid_comparison"),
        ("missing..HEAD", "revision_unavailable"),
        ("HEAD^{tree}..HEAD", "revision_unavailable"),
    ] {
        let out = call(&r, &["diff", comparison, "--format", "json"]);
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stdout.is_empty());
        assert!(String::from_utf8_lossy(&out.stderr).contains(code));
    }
    let a = String::from_utf8(r.git(&["rev-parse", "HEAD"]).unwrap().stdout).unwrap();
    r.write("asset.bin", [0, 1, 0, 2]).unwrap();
    let b = commit(&r);
    let data = document(call(
        &r,
        &["diff", &format!("{}..{b}", a.trim()), "--format", "json"],
    ));
    assert_eq!(data["incomplete_analysis"], true);
    assert_eq!(
        data["data"]["git_diagnostics"][0]["reason"],
        "binary_or_non_utf8"
    );
}
#[test]
fn high_fanout_is_summarized_without_pruning_test_selection() {
    let r = GitRepo::new().unwrap();
    r.write("lib.ts", "export function hub() { return 1; }\n")
        .unwrap();
    for i in 0..51 {
        r.write(
            &format!("t{i}.test.ts"),
            "import { hub } from './lib'; hub();\n",
        )
        .unwrap();
    }
    let a = commit(&r);
    r.write("lib.ts", "export function hub() { return 2; }\n")
        .unwrap();
    let b = commit(&r);
    let pair = format!("{a}..{b}");
    let data = document(call(
        &r,
        &["diff", &pair, "--format", "json", "--limit", "1"],
    ));
    assert_eq!(
        data["data"]["result"]["tests"]["combined"]
            .as_array()
            .unwrap()
            .len(),
        51
    );
    assert_eq!(
        data["data"]["result"]["base"]["query"]["summaries"][0]["hubs"][0]["direct_dependents"],
        51
    );
    let human = String::from_utf8(call(&r, &["diff", &pair, "--limit", "1"]).stdout).unwrap();
    assert!(human.contains("51 direct dependents"));
    assert!(human.contains("omitted from human display"));
}

#[test]
fn human_output_escapes_direction_markers_and_json_retains_original_paths() {
    let r = GitRepo::new().unwrap();
    let name = "direction\u{202e}.ts";
    r.write(name, "export function f() { return 1; }\n")
        .unwrap();
    let a = commit(&r);
    r.write(name, "export function f() { return 2; }\n")
        .unwrap();
    let b = commit(&r);
    let pair = format!("{a}..{b}");
    let human = String::from_utf8(call(&r, &["diff", &pair]).stdout).unwrap();
    assert!(!human.contains('\u{202e}'));
    assert!(human.contains("\\u202e"));
    let data = document(call(&r, &["diff", &pair, "--format", "json"]));
    assert_eq!(
        data["data"]["result"]["changes"]["files"][0]["change"]["new_path"],
        name
    );
}
