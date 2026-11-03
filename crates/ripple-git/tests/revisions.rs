use ripple_core::{
    identity::RepoPath,
    revision::{ComparisonSpec, FileChangeKind, LineRange},
    source::SourceTree,
};
use ripple_git::revisions::Repository;
use std::{fs, path::Path, process::Command};
fn git(root: &Path, args: &[&str]) -> String {
    let mut cmd = Command::new("git");
    for (k, _) in std::env::vars_os() {
        if k.to_string_lossy().to_ascii_uppercase().starts_with("GIT_") {
            cmd.env_remove(k);
        }
    }
    let out = cmd
        .args([
            "-c",
            "core.autocrlf=false",
            "-c",
            "core.hooksPath=",
            "-c",
            "commit.gpgsign=false",
            "-C",
        ])
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}
fn repo() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    git(d.path(), &["init", "-q"]);
    git(d.path(), &["config", "user.name", "Ripple Fixture"]);
    git(
        d.path(),
        &["config", "user.email", "fixture@ripple.invalid"],
    );
    d
}
fn commit(root: &Path) -> String {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "fixture", "--allow-empty"]);
    git(root, &["rev-parse", "HEAD"]).trim().into()
}
fn compare(root: &Path, a: &str, b: &str) -> ripple_git::revisions::RevisionSnapshots {
    Repository::open(root)
        .unwrap()
        .compare(&ComparisonSpec::parse(&format!("{a}..{b}")).unwrap())
        .unwrap()
}
fn write(root: &Path, path: &str, bytes: impl AsRef<[u8]>) {
    let p = root.join(path);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}
#[test]
fn immutable_unicode_trees_reuse_scanner_ignores_and_preserve_dirty_index() {
    let d = repo();
    let root = d.path();
    write(root, "src/space Ω.ts", "export function a() {}\n");
    write(root, ".gitignore", "ignored/\n");
    write(root, "ignored/no.ts", "invisible");
    write(root, "node_modules/no.ts", "invisible");
    let a = commit(root);
    write(root, "src/space Ω.ts", "export function b() {}\n");
    let b = commit(root);
    write(root, "src/space Ω.ts", "STAGED\r\n");
    git(root, &["add", "src/space Ω.ts"]);
    write(root, "src/space Ω.ts", "DIRTY\r\n");
    write(root, "untracked.ts", "leave me");
    let index = fs::read(root.join(".git/index")).unwrap();
    let head = fs::read(root.join(".git/HEAD")).unwrap();
    let refs = git(root, &["show-ref"]);
    let result = compare(root, &a, &b);
    let before = ripple_parse::scanner::discover(&result.base).unwrap();
    let after = ripple_parse::scanner::discover(&result.head).unwrap();
    assert_eq!(before.files.len(), 1);
    assert!(
        before
            .sources
            .files()
            .next()
            .unwrap()
            .text()
            .contains("function a")
    );
    assert!(
        after
            .sources
            .files()
            .next()
            .unwrap()
            .text()
            .contains("function b")
    );
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), index);
    assert_eq!(fs::read(root.join(".git/HEAD")).unwrap(), head);
    assert_eq!(git(root, &["show-ref"]), refs);
    assert_eq!(fs::read(root.join("src/space Ω.ts")).unwrap(), b"DIRTY\r\n");
    assert_eq!(fs::read(root.join("untracked.ts")).unwrap(), b"leave me");
    let pinned = result.changes.revisions().clone();
    git(root, &["update-ref", "refs/heads/moving", &a]);
    let old = Repository::open(root)
        .unwrap()
        .resolve(&ComparisonSpec::parse("moving..HEAD").unwrap())
        .unwrap();
    git(root, &["update-ref", "refs/heads/moving", &b]);
    assert_eq!(
        Repository::open(root)
            .unwrap()
            .compare_pair(old)
            .unwrap()
            .changes
            .revisions(),
        &pinned
    );
}
#[test]
fn add_delete_rename_empty_commit_and_line_ranges_are_explicit() {
    let d = repo();
    let r = d.path();
    write(r, "delete.ts", "one\r\ntwo");
    write(
        r,
        "rename Ω.ts",
        "export function unchanged() { return 123456789; }\n",
    );
    write(r, "edit.ts", "a\r\nb\r\nc\r\n");
    let a = commit(r);
    fs::remove_file(r.join("delete.ts")).unwrap();
    fs::rename(r.join("rename Ω.ts"), r.join("renamed space.ts")).unwrap();
    write(r, "add.ts", "new\nline");
    write(r, "edit.ts", "a\r\ninserted\r\nb\r\nc\r\n");
    let b = commit(r);
    let x = compare(r, &a, &b);
    let find = |kind| x.changes.files().iter().find(|f| f.kind == kind).unwrap();
    assert_eq!(find(FileChangeKind::Renamed).rename_similarity, Some(100));
    assert!(find(FileChangeKind::Renamed).hunks.is_empty());
    assert_eq!(
        find(FileChangeKind::Added).hunks[0].old,
        LineRange { start: 0, count: 0 }
    );
    assert_eq!(
        find(FileChangeKind::Added).hunks[0].new,
        LineRange { start: 1, count: 2 }
    );
    assert_eq!(
        find(FileChangeKind::Deleted).hunks[0].new,
        LineRange { start: 0, count: 0 }
    );
    assert_eq!(
        find(FileChangeKind::Deleted).hunks[0].old,
        LineRange { start: 1, count: 2 }
    );
    let h = &find(FileChangeKind::Modified).hunks[0];
    assert_eq!(h.old, LineRange { start: 1, count: 0 });
    assert_eq!(h.new, LineRange { start: 2, count: 1 });
    let reverse = compare(r, &b, &a);
    let h = &reverse
        .changes
        .files()
        .iter()
        .find(|f| f.kind == FileChangeKind::Modified)
        .unwrap()
        .hunks[0];
    assert_eq!(h.old, LineRange { start: 2, count: 1 });
    assert_eq!(h.new, LineRange { start: 1, count: 0 });
    let c = commit(r);
    assert!(compare(r, &b, &c).changes.files().is_empty());
    assert!(compare(r, &a, &a).changes.files().is_empty());
}
#[test]
fn binary_non_utf8_empty_and_no_newline_changes_are_visible() {
    let d = repo();
    let r = d.path();
    write(r, "x.ts", "abc");
    let a = commit(r);
    write(r, "x.ts", "abcd");
    write(r, "binary.ts", b"a\0b");
    write(r, "invalid.ts", [0xff, 0xfe]);
    write(r, "empty.ts", "");
    let b = commit(r);
    let x = compare(r, &a, &b);
    assert_eq!(x.diagnostics.len(), 2);
    let scan = ripple_parse::scanner::discover(&x.head);
    assert!(scan.is_err());
    let changed = x
        .changes
        .files()
        .iter()
        .find(|f| f.new_path.as_ref().is_some_and(|p| p.as_str() == "x.ts"))
        .unwrap();
    assert_eq!(changed.hunks[0].old, LineRange { start: 1, count: 1 });
    assert_eq!(changed.hunks[0].new, LineRange { start: 1, count: 1 });
    assert!(
        x.changes
            .files()
            .iter()
            .find(|f| f
                .new_path
                .as_ref()
                .is_some_and(|p| p.as_str() == "empty.ts"))
            .unwrap()
            .hunks
            .is_empty()
    );
}
#[test]
fn malicious_refs_and_missing_objects_fail_without_shell_or_options() {
    let d = repo();
    let r = d.path();
    let a = commit(r);
    let repo = Repository::open(r).unwrap();
    for bad in [
        "--help..HEAD",
        "HEAD...HEAD",
        "missing..HEAD",
        "HEAD;touch ripple-pwned..HEAD",
        "$(touch ripple-pwned)..HEAD",
    ] {
        assert!(
            ComparisonSpec::parse(bad)
                .map(|s| repo.compare(&s).is_err())
                .unwrap_or(true)
        );
    }
    assert!(
        repo.compare(&ComparisonSpec {
            base_ref: "--help".into(),
            head_ref: "HEAD".into()
        })
        .is_err()
    );
    assert!(!r.join("ripple-pwned").exists());
    assert!(compare(r, &a, &a).changes.files().is_empty());
    let zeros = ripple_core::revision::CommitId::new("0".repeat(40)).unwrap();
    assert!(
        repo.compare_pair(ripple_core::revision::RevisionPair {
            base: zeros.clone(),
            head: zeros
        })
        .is_err()
    );
}
#[test]
fn oversized_historical_inputs_fail_with_a_bounded_error() {
    let d = repo();
    write(d.path(), "huge.ts", vec![b'x'; 8 * 1024 * 1024 + 1]);
    let a = commit(d.path());
    let x = compare(d.path(), &a, &a);
    assert!(
        x.head
            .read(&RepoPath::new("huge.ts").unwrap())
            .unwrap_err()
            .to_string()
            .contains("8 MiB")
    );
}
#[cfg(unix)]
#[test]
fn symlinks_never_read_targets_and_external_diff_drivers_never_run() {
    use std::os::unix::fs::symlink;
    let d = repo();
    let r = d.path();
    write(r, "real.ts", "old\n");
    symlink("real.ts", r.join("link.ts")).unwrap();
    write(r, ".gitattributes", "*.ts diff=trap\n");
    let a = commit(r);
    write(r, "real.ts", "new\n");
    let b = commit(r);
    let script = r.join("trap.sh");
    fs::write(&script, "#!/bin/sh\ntouch executed\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
    git(
        r,
        &["config", "diff.trap.command", script.to_str().unwrap()],
    );
    git(
        r,
        &["config", "diff.trap.textconv", script.to_str().unwrap()],
    );
    let x = compare(r, &a, &b);
    assert!(!r.join("executed").exists());
    assert!(x.base.read(&RepoPath::new("link.ts").unwrap()).is_err());
    let scan = ripple_parse::scanner::discover(&x.base).unwrap();
    assert_eq!(scan.files.len(), 1);
    assert!(!scan.diagnostics.is_empty());
}

#[test]
fn edited_rename_hunks_and_disappearing_objects_are_explicit() {
    let d = repo();
    let r = d.path();
    write(
        r,
        "before.ts",
        "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\n",
    );
    let a = commit(r);
    fs::rename(r.join("before.ts"), r.join("after.ts")).unwrap();
    write(
        r,
        "after.ts",
        "one\ntwo\nchanged\nfour\nfive\nsix\nseven\neight\nnine\nten\n",
    );
    let b = commit(r);
    let result = compare(r, &a, &b);
    let file = &result.changes.files()[0];
    assert_eq!(file.kind, FileChangeKind::Renamed);
    assert_eq!(file.hunks[0].old, LineRange { start: 3, count: 1 });
    assert_eq!(file.hunks[0].new, LineRange { start: 3, count: 1 });
    assert_eq!(result.base.revision().as_str(), a);
    let id = git(r, &["rev-parse", "HEAD:after.ts"]);
    let id = id.trim();
    let object = r.join(".git/objects").join(&id[..2]).join(&id[2..]);
    #[cfg(windows)]
    {
        let mut permissions = fs::metadata(&object).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&object, permissions).unwrap();
    }
    fs::remove_file(object).unwrap();
    assert!(
        result
            .head
            .read(&RepoPath::new("after.ts").unwrap())
            .is_err()
    );
}
