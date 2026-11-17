mod support;
use std::process::Command;
use support::{GitRepo, ripple};

#[test]
fn quotas_and_prune_are_explicit_and_leave_active_generation_usable() {
    let repo = GitRepo::new().unwrap();
    repo.write("lib.ts", "export function f(){return 1;}\n")
        .unwrap();
    repo.commit_all("fixture").unwrap();
    let call = |args: &[&str]| {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ripple"));
        c.current_dir(repo.root()).args(args);
        support::run(c).unwrap()
    };
    for _ in 0..2 {
        assert!(
            call(&["index", ".", "--max-generations", "2"])
                .status
                .success()
        );
    }
    let refused = call(&["index", ".", "--max-generations", "2", "--format", "json"]);
    assert!(!refused.status.success());
    assert!(refused.stdout.is_empty());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("quota"));
    let before = call(&["status", "--format", "json"]);
    assert!(before.status.success());
    let maintenance = call(&[
        "index",
        ".",
        "--prune-to",
        "1",
        "--compact",
        "--format",
        "json",
    ]);
    assert!(maintenance.status.success(), "{:?}", maintenance);
    let value: serde_json::Value = serde_json::from_slice(&maintenance.stdout).unwrap();
    assert_eq!(value["data"]["maintenance"], "prune");
    assert_eq!(value["data"]["result"]["active_generation"], 2);
    assert_eq!(
        value["data"]["result"]["removed_generations"],
        serde_json::json!([1])
    );
    assert_eq!(call(&["status", "--format", "json"]).stdout, before.stdout);
    assert!(
        call(&["index", ".", "--max-generations", "2"])
            .status
            .success()
    );
}
#[test]
fn invalid_resource_options_fail_usage_validation() {
    for args in [
        vec!["--timeout-seconds", "0", "status"],
        vec!["index", "--max-index-mib", "0"],
        vec!["index", "--max-generations", "0"],
        vec!["index", "--prune-to", "0"],
        vec!["index", "--compact"],
        vec!["index", "--prune-to", "1", "--full"],
    ] {
        let out = ripple(&args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
    }
}
#[cfg(unix)]
#[test]
fn deadline_and_sigint_kill_and_reap_the_active_git_child() {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        thread,
        time::{Duration, Instant},
    };
    for signal in ["", "-INT", "-TERM"] {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        fs::create_dir(&bin).unwrap();
        let marker = dir.path().join("child.pid");
        let fake = bin.join("git");
        fs::write(
            &fake,
            "#!/bin/sh\nprintf '%s' \"$$\" > \"$RIPPLE_TEST_CHILD_MARKER\"\nexec /bin/sleep 30\n",
        )
        .unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
        let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        )))
        .unwrap();
        let mut c = Command::new(env!("CARGO_BIN_EXE_ripple"));
        c.args([
            "index",
            ".",
            "--timeout-seconds",
            if !signal.is_empty() { "10" } else { "1" },
        ])
        .current_dir(dir.path())
        .env("PATH", path)
        .env("RIPPLE_TEST_CHILD_MARKER", &marker)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
        let start = Instant::now();
        let mut child = c.spawn().unwrap();
        while !marker.exists() && start.elapsed() < Duration::from_secs(3) {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(marker.exists(), "Git child never started");
        let pid = fs::read_to_string(&marker).unwrap();
        if !signal.is_empty() {
            assert!(
                Command::new("kill")
                    .args([signal, &child.id().to_string()])
                    .status()
                    .unwrap()
                    .success()
            );
        }
        while child.try_wait().unwrap().is_none() && start.elapsed() < Duration::from_secs(6) {
            thread::sleep(Duration::from_millis(10));
        }
        if child.try_wait().unwrap().is_none() {
            let _ = child.kill();
            let _ = child.wait();
            panic!("cancellation did not terminate command");
        }
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains(if !signal.is_empty() {
                "command_cancelled"
            } else {
                "deadline_exceeded"
            }),
            "{error}"
        );
        assert!(
            !Command::new("kill")
                .args(["-0", pid.trim()])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .unwrap()
                .success(),
            "Git child still exists"
        );
        assert!(!dir.path().join(".ripple").exists());
    }
}
