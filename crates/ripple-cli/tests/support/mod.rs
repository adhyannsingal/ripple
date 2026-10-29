//! Integration-test helpers. Each test binary uses a subset of this module.
#![allow(dead_code)]

use serde_json::Value;
use std::io::Read;
use std::{
    fs,
    path::{Component, Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;

pub fn run(command: Command) -> Result<Output, String> {
    run_with_timeout(command, Duration::from_secs(20))
}

pub fn run_with_timeout(mut command: Command, timeout: Duration) -> Result<Output, String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let mut stdout = child.stdout.take().ok_or("missing stdout pipe")?;
    let mut stderr = child.stderr.take().ok_or("missing stderr pipe")?;
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            result => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(match result {
                    Err(error) => error.to_string(),
                    _ => format!("fixture process exceeded {timeout:?}; killed and reaped"),
                });
            }
        }
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| "stdout reader panicked")?
        .map_err(|e| e.to_string())?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "stderr reader panicked")?
        .map_err(|e| e.to_string())?;
    Ok(Output {
        status: status?,
        stdout,
        stderr,
    })
}

pub fn ripple(args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ripple"));
    command.args(args);
    run(command).expect("ripple process must complete")
}

pub fn assert_json(actual: &[u8], expected: &Value) {
    let value: Value =
        serde_json::from_slice(actual).expect("stdout must be exactly one JSON document");
    assert_eq!(&value, expected);
}

#[derive(Debug)]
pub struct GitRepo {
    root: PathBuf,
    temporary: TempDir,
}
impl GitRepo {
    pub fn new() -> Result<Self, String> {
        let temporary = tempfile::Builder::new()
            .prefix("ripple-fixture-")
            .tempdir()
            .map_err(|e| e.to_string())?;
        let root = temporary.path().join("repository with spaces");
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        fs::create_dir_all(temporary.path().join("empty-template")).map_err(|e| e.to_string())?;
        fs::create_dir_all(temporary.path().join("no-hooks")).map_err(|e| e.to_string())?;
        fs::write(temporary.path().join("gitconfig"), "").map_err(|e| e.to_string())?;
        let repo = Self { root, temporary };
        let template = format!(
            "--template={}",
            repo.temporary.path().join("empty-template").display()
        );
        repo.git(&["init", "--quiet", "--initial-branch=main", &template])?;
        // These identities are deterministic fixture data in this disposable repo only.
        // Project commits always use the user's configured author, unchanged.
        for (key, value) in [
            ("user.name", "Ripple Fixture"),
            ("user.email", "fixture@ripple.invalid"),
            ("commit.gpgsign", "false"),
            ("core.autocrlf", "false"),
        ] {
            repo.git(&["config", "--local", key, value])?;
        }
        Ok(repo)
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn write(&self, relative: &str, bytes: impl AsRef<[u8]>) -> Result<(), String> {
        if relative.is_empty()
            || relative.contains(['\\', ':'])
            || Path::new(relative)
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err("fixture write must stay relative to its repository".into());
        }
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().ok_or("missing parent")?).map_err(|e| e.to_string())?;
        fs::write(path, bytes).map_err(|e| e.to_string())
    }
    pub fn git(&self, args: &[&str]) -> Result<Output, String> {
        let hooks = self.temporary.path().join("no-hooks");
        let mut command = Command::new("git");
        command
            .current_dir(&self.root)
            .arg("-c")
            .arg(format!("core.hooksPath={}", hooks.display()))
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", self.temporary.path().join("gitconfig"))
            .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z");
        for key in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_COMMON_DIR",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_CONFIG_COUNT",
            "GIT_CONFIG_PARAMETERS",
            "GIT_AUTHOR_NAME",
            "GIT_AUTHOR_EMAIL",
            "GIT_COMMITTER_NAME",
            "GIT_COMMITTER_EMAIL",
        ] {
            command.env_remove(key);
        }
        let output = run(command)?;
        if !output.status.success() {
            return Err(format!(
                "git {args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(output)
    }
    pub fn commit_all(&self, message: &str) -> Result<String, String> {
        self.git(&["add", "--all"])?;
        self.git(&["commit", "--quiet", "--message", message])?;
        let output = self.git(&["rev-parse", "HEAD"])?;
        Ok(String::from_utf8(output.stdout)
            .map_err(|e| e.to_string())?
            .trim()
            .into())
    }
}
