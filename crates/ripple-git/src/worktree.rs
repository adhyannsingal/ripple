//! Minimal read-only provenance adapter; immutable endpoint/diff support remains ripple-git work.
use ripple_core::revision::{CommitId, SnapshotOrigin};
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
fn run(root: &Path, args: &[&str]) -> Result<(bool, Vec<u8>), String> {
    let mut cmd = Command::new("git");
    for (name, _) in std::env::vars_os() {
        if name
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("GIT_")
        {
            cmd.env_remove(name);
        }
    }
    cmd.args([
        "--no-optional-locks",
        "--no-pager",
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.untrackedCache=false",
        "-C",
    ])
    .arg(root)
    .args(args)
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .env(
        "GIT_CONFIG_GLOBAL",
        if cfg!(windows) { "NUL" } else { "/dev/null" },
    )
    .env("GIT_TERMINAL_PROMPT", "0")
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("Git is required: {e}"))?;
    let stdout = child.stdout.take().ok_or("missing Git stdout")?;
    let stderr = child.stderr.take().ok_or("missing Git stderr")?;
    let reader = |stream: Box<dyn Read + Send>| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            stream
                .take(8 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map(|_| bytes)
        })
    };
    let out = reader(Box::new(stdout));
    let err = reader(Box::new(stderr));
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break Err("Git command timed out or failed to wait".to_string());
            }
        }
    };
    let output = out
        .join()
        .map_err(|_| "Git stdout reader failed")?
        .map_err(|e| e.to_string())?;
    let errors = err
        .join()
        .map_err(|_| "Git stderr reader failed")?
        .map_err(|e| e.to_string())?;
    if output.len() > 8 * 1024 * 1024 || errors.len() > 8 * 1024 * 1024 {
        return Err("Git output exceeds 8 MiB".into());
    }
    Ok((status?.success(), output))
}
pub fn observe(root: &Path) -> Result<SnapshotOrigin, String> {
    let (ok, top) = run(root, &["rev-parse", "--show-toplevel"])?;
    if !ok {
        return Err("index root must be a Git repository root".into());
    }
    let path = std::str::from_utf8(&top)
        .map_err(|_| "non-UTF-8 Git root unsupported")?
        .trim_end_matches(['\r', '\n']);
    if std::fs::canonicalize(path).map_err(|e| e.to_string())? != root {
        return Err("use the Git repository root as --root".into());
    }
    let (ok, filters) = run(
        root,
        &[
            "config",
            "--null",
            "--get-regexp",
            r"^filter\..*\.(clean|process)$",
        ],
    )?;
    if ok && !filters.is_empty() {
        return Err(
            "external Git clean/process filters are unsupported; no scripts were executed".into(),
        );
    }
    let (ok, head) = run(root, &["rev-parse", "--verify", "HEAD"])?;
    let head = if ok {
        Some(
            CommitId::new(
                std::str::from_utf8(&head)
                    .map_err(|_| "invalid Git HEAD")?
                    .trim()
                    .to_string(),
            )
            .map_err(|e| e.to_string())?,
        )
    } else {
        let (ok, _) = run(root, &["symbolic-ref", "-q", "HEAD"])?;
        if !ok {
            return Err("cannot read Git HEAD".into());
        }
        None
    };
    let (ok, status) = run(
        root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignore-submodules=all",
            "--",
            ".",
            ":(exclude).ripple",
        ],
    )?;
    if !ok {
        return Err("cannot observe Git worktree status".into());
    }
    Ok(SnapshotOrigin::Worktree {
        head,
        dirty: !status.is_empty(),
    })
}
