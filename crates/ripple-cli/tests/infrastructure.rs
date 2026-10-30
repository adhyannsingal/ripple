#[path = "../../../fixtures/support/mod.rs"]
mod fixtures;
mod support;

use serde_json::json;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
use support::GitRepo;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn catalog_has_independent_expected_facts_and_portable_source_lists() {
    let cases = fixtures::load_cases(&workspace()).unwrap();
    let named = cases.iter().find(|c| c.id == "named-import").unwrap();
    let shadowed = cases.iter().find(|c| c.id == "shadowed-import").unwrap();
    let unrelated = cases.iter().find(|c| c.id == "unrelated-exports").unwrap();
    for case in &cases {
        assert_eq!(
            case.files,
            case.files
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        );
        for file in &case.files {
            assert!(!fs::read_to_string(case.root.join(file)).unwrap().is_empty());
        }
    }
    assert_eq!(
        named.expected["calls"][0]["target"],
        "src/auth.ts::authenticate"
    );
    assert_eq!(
        shadowed.expected["forbidden_calls"][0]["target"],
        "src/auth.ts::login"
    );
    assert_eq!(
        unrelated.expected["symbol_backed_tests"],
        json!(["tests/a.test.ts"])
    );
    assert_eq!(
        unrelated.expected["fallback_only_tests"],
        json!(["tests/b.test.ts"])
    );
    assert!(unrelated.files.iter().any(|f| f.ends_with(".tsx")));
}

#[test]
fn fixture_paths_cannot_escape_their_root() {
    for invalid in [
        "../outside",
        "a/./b",
        "a//b",
        "a/b/",
        "/outside",
        r"C:\outside",
        ".",
        "a/../../b",
        "",
    ] {
        assert!(fixtures::checked_join(Path::new("fixture"), invalid).is_err());
    }
}

#[test]
fn temporary_git_commits_are_deterministic_and_handle_spaces() {
    let a = GitRepo::new().unwrap();
    let b = GitRepo::new().unwrap();
    assert!(
        a.root()
            .to_string_lossy()
            .contains("repository with spaces")
    );
    for repo in [&a, &b] {
        repo.write("src/file with spaces.ts", "export function f() {}\n")
            .unwrap();
    }
    let first = a.commit_all("fixture: add exported function").unwrap();
    assert_eq!(
        first,
        b.commit_all("fixture: add exported function").unwrap()
    );
    assert_eq!(first.len(), 40);
    a.write(
        "src/file with spaces.ts",
        "export function f() { return 1; }\n",
    )
    .unwrap();
    let second = a.commit_all("fixture: change function body").unwrap();
    assert_ne!(first, second);
    let output = a.git(&["diff", "--name-only", &first, &second]).unwrap();
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "src/file with spaces.ts"
    );
}

#[test]
fn a_real_fixture_can_be_committed_without_copying_expectations_into_the_repo() {
    let case = fixtures::load_cases(&workspace())
        .unwrap()
        .into_iter()
        .find(|c| c.id == "named-import")
        .unwrap();
    let repo = GitRepo::new().unwrap();
    for relative in &case.files {
        repo.write(relative, fs::read(case.root.join(relative)).unwrap())
            .unwrap();
    }
    repo.commit_all("fixture: named import project").unwrap();
    let files = repo.git(&["ls-files"]).unwrap();
    assert_eq!(
        String::from_utf8(files.stdout)
            .unwrap()
            .lines()
            .collect::<Vec<_>>(),
        ["src/auth.ts", "src/checkout.ts"]
    );
    assert!(
        repo.git(&["status", "--porcelain"])
            .unwrap()
            .stdout
            .is_empty()
    );
}

#[test]
fn temporary_repositories_reject_outside_writes_and_are_cleaned_up() {
    let repo = GitRepo::new().unwrap();
    let path = repo.root().to_path_buf();
    assert!(repo.write("../outside.ts", "bad").is_err());
    assert!(repo.write("/outside.ts", "bad").is_err());
    drop(repo);
    assert!(!path.exists());
}

#[test]
fn json_assertions_allow_object_order_but_reject_trailing_logs() {
    support::assert_json(br#"{"b":2,"a":1}"#, &json!({"a":1,"b":2}));
    assert!(
        std::panic::catch_unwind(|| support::assert_json(b"{}\nextra log", &json!({}))).is_err()
    );
}

// A controlled child-process target: normal suite execution is a no-op.
#[test]
fn process_probe() {
    match std::env::var("RIPPLE_TEST_PROCESS_PROBE").as_deref() {
        Ok("burst") => {
            print!("{}", "x".repeat(256 * 1024));
            eprint!("{}", "y".repeat(256 * 1024));
        }
        Ok("sleep") => std::thread::sleep(std::time::Duration::from_secs(5)),
        _ => {}
    }
}

#[test]
fn process_capture_drains_both_pipes_without_deadlock() {
    let mut child = std::process::Command::new(std::env::current_exe().unwrap());
    child
        .args(["--exact", "process_probe", "--nocapture"])
        .env("RIPPLE_TEST_PROCESS_PROBE", "burst");
    let output = support::run(child).unwrap();
    assert!(output.status.success());
    assert!(output.stdout.len() >= 256 * 1024);
    assert!(output.stderr.len() >= 256 * 1024);
}

#[test]
fn timed_out_direct_child_is_killed_and_reaped() {
    let mut child = std::process::Command::new(std::env::current_exe().unwrap());
    child
        .args(["--exact", "process_probe", "--nocapture"])
        .env("RIPPLE_TEST_PROCESS_PROBE", "sleep");
    let error =
        support::run_with_timeout(child, std::time::Duration::from_millis(300)).unwrap_err();
    assert!(error.contains("killed and reaped"));
}

#[test]
fn git_hash_environment_cannot_change_fixture_format() {
    let mut child = std::process::Command::new(std::env::current_exe().unwrap());
    child
        .args([
            "--exact",
            "temporary_git_commits_are_deterministic_and_handle_spaces",
        ])
        .env("GIT_DEFAULT_HASH", "sha256");
    let output = support::run(child).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}
