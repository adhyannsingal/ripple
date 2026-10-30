use ripple_core::{identity::RepoPath, source::SourceProvider};
use ripple_parse::scanner::*;
use std::{collections::BTreeMap, fs};
fn path(s: &str) -> RepoPath {
    RepoPath::new(s).unwrap()
}
struct MemoryTree(BTreeMap<RepoPath, Vec<u8>>);
impl SourceTree for MemoryTree {
    fn entries(&self, directory: Option<&RepoPath>) -> Result<Vec<TreeEntry>, ScanError> {
        let prefix = directory.map_or(String::new(), |p| format!("{p}/"));
        let mut entries = BTreeMap::new();
        for p in self.0.keys() {
            if let Some(tail) = p.as_str().strip_prefix(&prefix) {
                let (name, kind) = match tail.split_once('/') {
                    Some((first, _)) => (first, EntryKind::Directory),
                    None => (tail, EntryKind::File),
                };
                entries.insert(path(&format!("{prefix}{name}")), kind);
            }
        }
        Ok(entries
            .into_iter()
            .map(|(path, kind)| TreeEntry { path, kind })
            .collect())
    }
    fn read(&self, path: &RepoPath) -> Result<Vec<u8>, ScanError> {
        self.0
            .get(path)
            .cloned()
            .ok_or(ScanError("missing input".into()))
    }
}
#[test]
fn shared_policy_matches_worktree_and_revision_like_inputs() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/scanner/cases.json")).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let mut entries = BTreeMap::new();
    for (name, text) in fixture["files"].as_object().unwrap() {
        let file = temp.path().join(name);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text.as_str().unwrap()).unwrap();
        entries.insert(path(name), text.as_str().unwrap().as_bytes().to_vec());
    }
    let provider = Worktree::new(temp.path().join("src/..")).unwrap();
    assert_eq!(provider.root(), fs::canonicalize(temp.path()).unwrap());
    let a = provider.discover().unwrap();
    assert_eq!(a, provider.discover().unwrap());
    assert_eq!(a, discover(&MemoryTree(entries)).unwrap());
    assert_eq!(provider.capture().unwrap(), a.sources);
    assert_eq!(
        serde_json::json!(a.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>()),
        fixture["expected_sources"]
    );
    assert_eq!(
        serde_json::json!(
            a.files
                .iter()
                .filter(|f| f.is_test)
                .map(|f| f.path.as_str())
                .collect::<Vec<_>>()
        ),
        fixture["expected_tests"]
    );
    assert!(a.inputs.iter().any(|i| i.path.as_str() == "package.json"));
    assert!(a.inputs.iter().any(|i| i.path.as_str() == "src/.gitignore"));
    assert_eq!(
        a.sources.get(&path("src/untracked.ts")).unwrap().text(),
        "export const fresh = 1;"
    );
}
#[test]
fn nested_roots_do_not_inherit_host_or_ancestor_ignores() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join(".gitignore"), "*.ts").unwrap();
    fs::create_dir(temp.path().join("project")).unwrap();
    fs::write(temp.path().join("project/a.ts"), "export {};").unwrap();
    assert_eq!(
        Worktree::new(temp.path().join("project"))
            .unwrap()
            .discover()
            .unwrap()
            .files
            .len(),
        1
    );
}
#[test]
fn invalid_source_and_ignore_bytes_fail_without_lossy_conversion() {
    for name in ["a.ts", ".gitignore"] {
        let input = MemoryTree(BTreeMap::from([(path(name), vec![0xff])]));
        assert!(discover(&input).unwrap_err().to_string().contains("UTF-8"));
    }
    let big = MemoryTree(BTreeMap::from([(
        path("a.ts"),
        vec![b' '; MAX_INPUT_BYTES as usize + 1],
    )]));
    assert!(discover(&big).unwrap_err().to_string().contains("exceeds"));
}
#[test]
fn path_case_collisions_are_rejected() {
    let input = MemoryTree(BTreeMap::from([
        (path("A.ts"), vec![]),
        (path("a.ts"), vec![]),
    ]));
    assert!(
        discover(&input)
            .unwrap_err()
            .to_string()
            .contains("case-colliding")
    );
}
#[cfg(unix)]
#[test]
fn symlinks_inside_and_outside_root_are_never_followed() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("a.ts"), "export {};").unwrap();
    fs::write(outside.path().join("secret.ts"), "outside").unwrap();
    symlink(temp.path().join("a.ts"), temp.path().join("alias.ts")).unwrap();
    symlink(outside.path(), temp.path().join("outside")).unwrap();
    symlink(temp.path(), temp.path().join("cycle")).unwrap();
    let result = Worktree::new(temp.path()).unwrap().discover().unwrap();
    assert_eq!(result.files.len(), 1);
    assert_eq!(result.diagnostics.len(), 3);
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.code == "symlink_skipped")
    );
}
