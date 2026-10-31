mod support;
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use support::{GitRepo, run};
use tempfile::TempDir;
fn inspect(root: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ripple"));
    command.current_dir(root).arg("inspect").args(args);
    run(command).unwrap()
}
fn fixture() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/inspect/project");
    fs::create_dir(dir.path().join("src")).unwrap();
    for file in ["empty.ts", "src/caller.ts", "src/view.tsx"] {
        fs::copy(root.join(file), dir.path().join(file)).unwrap();
    }
    dir
}
fn json_report(root: &Path, file: &str) -> Value {
    let output = inspect(root, &[file, "--format", "json"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn sorted(mut values: Vec<Value>) -> Vec<Value> {
    values.sort_by_key(Value::to_string);
    values
}
#[test]
fn independent_ts_and_tsx_output_goldens_preserve_complete_syntax_evidence() {
    let root = fixture();
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/inspect");
    let expected: Value =
        serde_json::from_str(&fs::read_to_string(fixture_dir.join("expected.json")).unwrap())
            .unwrap();
    for case in expected["cases"].as_array().unwrap() {
        let file = case["path"].as_str().unwrap();
        let output = inspect(root.path(), &[file, "--format", "json"]);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        let data = &report["data"];
        assert_eq!(report["format_version"], 1);
        assert_eq!(report["command"], "inspect");
        assert_eq!(report["incomplete_analysis"], false);
        assert_eq!(report["diagnostics"], json!([]));
        assert_eq!(data["analysis_stage"], "syntax");
        assert_eq!(data["resolution_available"], false);
        assert_eq!(
            data["analyzer_version"],
            ripple_parse::hashes::EXTRACTOR_VERSION
        );
        for field in ["path", "language", "is_test"] {
            assert_eq!(data[field], case[field], "{file}: {field}");
        }
        let symbols=data["symbols"].as_array().unwrap().iter().map(|s|json!({"name":s["identity"]["qualified_name"],"kind":s["identity"]["kind"],"exported":s["exported"]})).collect();
        assert_eq!(
            sorted(symbols),
            sorted(case["symbols"].as_array().unwrap().clone()),
            "{file} symbols"
        );
        let imports=data["imports"].as_array().unwrap().iter().map(|i|json!({"specifier":i["specifier"],"bindings":i["bindings"].as_array().unwrap().iter().map(|b|json!({"imported":b["imported"],"local":b["local"],"type_only":b["type_only"]})).collect::<Vec<_>>()})).collect();
        assert_eq!(
            sorted(imports),
            sorted(case["imports"].as_array().unwrap().clone()),
            "{file} imports"
        );
        assert_eq!(
            sorted(
                data["exports"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|e| e["kind"].clone())
                    .collect()
            ),
            sorted(case["exports"].as_array().unwrap().clone()),
            "{file} exports"
        );
        let refs=data["references"].as_array().unwrap().iter().map(|r|json!({"text":r["text"],"kind":r["kind"],"owner":r["owner"]["symbol"]["qualified_name"]})).collect();
        assert_eq!(
            sorted(refs),
            sorted(case["references"].as_array().unwrap().clone()),
            "{file} refs"
        );
        let source = fs::read_to_string(root.path().join(file)).unwrap();
        for reference in data["references"].as_array().unwrap() {
            let start = reference["span"]["start"]["byte"].as_u64().unwrap() as usize;
            let end = reference["span"]["end"]["byte"].as_u64().unwrap() as usize;
            assert_eq!(&source[start..end], reference["text"].as_str().unwrap());
            assert!(reference["scope"].is_number());
        }
        for symbol in data["symbols"].as_array().unwrap() {
            for field in ["signature_hash", "body_hash"] {
                assert_eq!(symbol[field]["version"], 1);
                assert_eq!(symbol[field]["digest"].as_str().unwrap().len(), 64);
            }
        }
    }
}
#[test]
fn empty_human_report_matches_independent_full_golden() {
    let root = fixture();
    let output = inspect(root.path(), &["empty.ts"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        include_str!("../../../fixtures/inspect/empty-human.txt").replace("\r\n", "\n")
    );
}
#[test]
fn reports_are_byte_stable_across_repeats_locations_and_flag_positions() {
    let a = fixture();
    let b = fixture();
    for format in ["human", "json"] {
        let first = inspect(a.path(), &["src/caller.ts", "--format", format]);
        let repeated = inspect(a.path(), &["src/caller.ts", "--format", format]);
        let relocated = inspect(b.path(), &["src/caller.ts", "--format", format]);
        assert!(first.status.success());
        assert_eq!(first.stdout, repeated.stdout);
        assert_eq!(first.stdout, relocated.stdout);
        assert_eq!(first.stderr, repeated.stderr);
        assert_eq!(first.stderr, relocated.stderr);
        let mut command = Command::new(env!("CARGO_BIN_EXE_ripple"));
        command
            .current_dir(a.path())
            .args(["--format", format, "inspect", "src/caller.ts"]);
        assert_eq!(first.stdout, run(command).unwrap().stdout);
    }
}
#[test]
fn root_is_explicit_and_paths_with_spaces_and_unicode_work() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project with spaces");
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("café.test.ts"),
        "export function café(){return 1;}",
    )
    .unwrap();
    let output = inspect(
        dir.path(),
        &[
            "café.test.ts",
            "--root",
            root.to_str().unwrap(),
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["data"]["path"], "café.test.ts");
    assert_eq!(report["data"]["is_test"], true);
    assert!(
        !String::from_utf8(output.stdout)
            .unwrap()
            .contains(root.to_str().unwrap())
    );
}
#[test]
fn recovered_syntax_returns_report_and_stderr_diagnostics_without_claiming_completeness() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("bad.ts"),
        "export function broken( { run(;",
    )
    .unwrap();
    for format in ["human", "json"] {
        let output = inspect(root.path(), &["bad.ts", "--format", format]);
        assert_eq!(output.status.code(), Some(0));
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("parse_error") || stderr.contains("parse_missing"));
        if format == "json" {
            let report: Value = serde_json::from_str(&stdout).unwrap();
            assert_eq!(report["incomplete_analysis"], true);
            assert_eq!(report["data"]["parse_status"], "RECOVERED");
            assert!(!report["diagnostics"].as_array().unwrap().is_empty());
        } else {
            assert!(stdout.contains("Incomplete analysis: yes"));
            assert!(!stdout.contains("Unrecognized or malformed syntax"));
        }
    }
}
#[test]
fn unsupported_targets_are_visible_and_do_not_become_resolved_edges() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("dynamic.ts"),
        "function f(x:any){x.run(); import('./x');}",
    )
    .unwrap();
    let report = json_report(root.path(), "dynamic.ts");
    assert_eq!(report["incomplete_analysis"], true);
    assert_eq!(report["data"]["resolution_available"], false);
    let codes = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(
        codes.contains(&"unsupported_call_target") && codes.contains(&"unsupported_dynamic_import")
    );
    assert!(report["data"].get("edges").is_none());
}
#[test]
fn ignored_excluded_missing_and_directory_targets_fail_with_no_success_document() {
    let root = fixture();
    fs::write(root.path().join(".gitignore"), "ignored.ts\n").unwrap();
    fs::write(root.path().join("ignored.ts"), "run();").unwrap();
    fs::create_dir(root.path().join("dist")).unwrap();
    fs::write(root.path().join("dist/output.ts"), "run();").unwrap();
    fs::create_dir(root.path().join("directory.ts")).unwrap();
    for file in ["missing.ts", "ignored.ts", "dist/output.ts", "directory.ts"] {
        let output = inspect(root.path(), &[file, "--format", "json"]);
        assert_eq!(output.status.code(), Some(1), "{file}");
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("file_not_discovered"));
    }
}
#[test]
fn invalid_paths_extensions_and_formats_are_usage_errors() {
    let root = fixture();
    for file in [
        "../caller.ts",
        "/caller.ts",
        "C:\\caller.ts",
        "src/../caller.ts",
        "caller.js",
        "",
    ] {
        let output = inspect(root.path(), &[file, "--format", "json"]);
        assert_eq!(output.status.code(), Some(2), "{file}");
        assert!(output.stdout.is_empty());
    }
}
#[test]
fn invalid_root_utf8_and_oversized_inputs_fail_operationally() {
    let root = fixture();
    let output = inspect(
        root.path(),
        &["empty.ts", "--root", "absent", "--format", "json"],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    fs::write(root.path().join("invalid.ts"), [0xff, 0xfe]).unwrap();
    let output = inspect(root.path(), &["invalid.ts", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid UTF-8"));
    fs::remove_file(root.path().join("invalid.ts")).unwrap();
    let file = fs::File::create(root.path().join("large.ts")).unwrap();
    file.set_len(ripple_parse::scanner::MAX_INPUT_BYTES + 1)
        .unwrap();
    let output = inspect(root.path(), &["large.ts", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
}
#[test]
fn module_test_callbacks_keep_file_ownership() {
    let root = fixture();
    fs::write(
        root.path().join("flow.spec.tsx"),
        "import {run} from './mod'; test('flow',()=>run());",
    )
    .unwrap();
    let report = json_report(root.path(), "flow.spec.tsx");
    assert_eq!(report["data"]["is_test"], true);
    for reference in report["data"]["references"].as_array().unwrap() {
        assert_eq!(reference["owner"], json!({"kind":"FILE"}));
    }
}
#[test]
fn untrusted_reference_text_is_escaped_in_human_and_json_output() {
    let root = fixture();
    fs::write(root.path().join("control.ts"), "object['\u{1b}[31m']();").unwrap();
    for format in ["human", "json"] {
        let output = inspect(root.path(), &["control.ts", "--format", format]);
        assert!(output.status.success());
        assert!(!output.stdout.contains(&0x1b));
        assert!(!output.stderr.contains(&0x1b));
        if format == "json" {
            let report: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert!(
                report["data"]["references"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["text"].as_str().unwrap().contains('\u{1b}'))
            );
        }
    }
}
#[test]
fn inspect_does_not_modify_git_or_create_index_state() {
    let repo = GitRepo::new().unwrap();
    repo.write("file.ts", "export function f(){}").unwrap();
    repo.commit_all("fixture").unwrap();
    let head = repo.git(&["rev-parse", "HEAD"]).unwrap().stdout;
    let before = repo
        .git(&["status", "--porcelain=v1", "--untracked-files=all"])
        .unwrap()
        .stdout;
    assert!(
        inspect(repo.root(), &["file.ts", "--format", "json"])
            .status
            .success()
    );
    assert_eq!(head, repo.git(&["rev-parse", "HEAD"]).unwrap().stdout);
    assert_eq!(
        before,
        repo.git(&["status", "--porcelain=v1", "--untracked-files=all"])
            .unwrap()
            .stdout
    );
    assert!(!repo.root().join(".ripple").exists());
}
#[cfg(unix)]
#[test]
fn scanner_symlinks_remain_visible_without_following_selected_or_other_links() {
    use std::os::unix::fs::symlink;
    let root = fixture();
    symlink("empty.ts", root.path().join("link.ts")).unwrap();
    let output = inspect(root.path(), &["link.ts", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let output = inspect(root.path(), &["empty.ts", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["incomplete_analysis"], true);
    assert_eq!(report["diagnostics"][0]["path"], "link.ts");
    assert_eq!(report["diagnostics"][0]["code"], "symlink_skipped");
    assert!(String::from_utf8_lossy(&output.stderr).contains("symlink_skipped"));
}
#[test]
fn populated_human_report_matches_independent_template_and_json_hash_fields() {
    let root = fixture();
    let report = json_report(root.path(), "src/caller.ts");
    let symbol = &report["data"]["symbols"][0];
    let template = include_str!("../../../fixtures/inspect/caller-human.txt").replace("\r\n", "\n");
    let signature = format!(
        "v{}:{}",
        symbol["signature_hash"]["version"],
        symbol["signature_hash"]["digest"].as_str().unwrap()
    );
    let body = format!(
        "v{}:{}",
        symbol["body_hash"]["version"],
        symbol["body_hash"]["digest"].as_str().unwrap()
    );
    let expected = template
        .replace("${SIGNATURE_HASH}", &signature)
        .replace("${BODY_HASH}", &body);
    let output = inspect(root.path(), &["src/caller.ts"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    let path = root.path().join("src/caller.ts");
    let source = fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
    fs::write(&path, source.replace('\n', "\r\n")).unwrap();
    let crlf = inspect(root.path(), &["src/caller.ts"]);
    assert!(crlf.status.success());
    assert_eq!(String::from_utf8(crlf.stdout).unwrap(), expected);
}
