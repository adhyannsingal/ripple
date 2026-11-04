use crate::{
    build,
    snapshots::{self, RevisionSnapshot},
};
use ripple_core::{
    canonical::CanonicalState,
    graph::{EdgeKind, NodeId},
    identity::RepoPath,
    results::{EvidenceKind, QueryLimits, RevisionSide},
    revision::{ComparisonSpec, RevisionGraphs},
};
use ripple_impact::revision_impact::{self, RevisionView};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn p(s: &str) -> RepoPath {
    RepoPath::new(s).unwrap()
}
fn git(root: &Path, args: &[&str]) -> String {
    let mut c = Command::new("git");
    for (k, _) in std::env::vars_os() {
        if k.to_string_lossy().to_ascii_uppercase().starts_with("GIT_") {
            c.env_remove(k);
        }
    }
    let out = c
        .args([
            "--no-optional-locks",
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
fn write(root: &Path, files: &serde_json::Value) {
    for (name, text) in files.as_object().unwrap() {
        let dest = root.join(name);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::write(dest, text.as_str().unwrap()).unwrap();
    }
}
fn commit(root: &Path) -> String {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "fixture", "--allow-empty"]);
    git(root, &["rev-parse", "HEAD"]).trim().into()
}
fn view(s: &RevisionSnapshot) -> RevisionView<'_> {
    RevisionView {
        index: s.index(),
        sources: s.sources(),
        global_inputs: s.global_inputs(),
        existing_paths: s.existing_paths(),
        incomplete: s.incomplete(),
        diagnostics: s.diagnostics(),
    }
}
fn snapshots(
    root: &Path,
    a: &str,
    b: &str,
) -> (
    ripple_git::revisions::RevisionSnapshots,
    RevisionSnapshot,
    RevisionSnapshot,
) {
    let trees = ripple_git::revisions::Repository::open(root)
        .unwrap()
        .compare(&ComparisonSpec::parse(&format!("{a}..{b}")).unwrap())
        .unwrap();
    let old = snapshots::build(&trees.base).unwrap();
    let new = snapshots::build(&trees.head).unwrap();
    (trees, old, new)
}
fn f_seed(node: &NodeId) -> bool {
    matches!(node,NodeId::Symbol(s) if s.qualified_name().display_name()=="f")
}
#[test]
fn real_revision_fixtures_preserve_both_evidence_tracks_and_old_relationships() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/revision-impact/cases.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let dir = repo();
        let root = dir.path();
        write(root, &case["before"]);
        let a = commit(root);
        for name in case["before"].as_object().unwrap().keys() {
            fs::remove_file(root.join(name)).unwrap();
        }
        write(root, &case["after"]);
        let b = commit(root);
        let (trees, old, new) = snapshots(root, &a, &b);
        let result = revision_impact::analyze(
            &trees.changes,
            view(&old),
            view(&new),
            QueryLimits::default(),
        )
        .unwrap();
        let has_f_test = |side| {
            result.evidence().symbol_backed().iter().any(|w| {
                w.revision == side && f_seed(&w.seed) && w.target.path() == &p("smoke.test.ts")
            })
        };
        if let Some(expected) = case["base_f_test"].as_bool() {
            assert_eq!(
                has_f_test(RevisionSide::Base),
                expected,
                "{} base",
                case["name"]
            );
        }
        if let Some(expected) = case["head_f_test"].as_bool() {
            assert_eq!(
                has_f_test(RevisionSide::Head),
                expected,
                "{} head",
                case["name"]
            );
        }
        let bound =
            RevisionGraphs::new(trees.changes.revisions().clone(), old.index(), new.index())
                .unwrap();
        for w in result
            .evidence()
            .symbol_backed()
            .iter()
            .chain(result.evidence().module_fallback())
        {
            bound.validate_witness(w).unwrap();
            assert_ne!(w.revision, RevisionSide::Worktree);
        }
        if case["base_only"] == true {
            assert!(
                result
                    .tests()
                    .base_only
                    .iter()
                    .any(|n| n.path() == &p("smoke.test.ts"))
            );
            assert!(result.tests().active_head.is_empty());
        }
        if case["present_unindexed"] == true {
            assert!(result.tests().base_only.is_empty());
            assert!(
                result
                    .tests()
                    .present_unindexed_head
                    .iter()
                    .any(|n| n.path() == &p("smoke.test.ts"))
            );
            assert!(new.existing_paths().contains(&p("smoke.test.ts")));
            assert!(new.sources().get(&p("smoke.test.ts")).is_none());
        }
        if case["config_diagnostic"] == true {
            assert!(result.incomplete());
            assert!(!old.config_diagnostics().is_empty());
            assert!(!result.base().snapshot_diagnostics.is_empty());
            assert!(!result.head().snapshot_diagnostics.is_empty());
        }
        if case["empty"] == true {
            assert!(result.evidence().all_nodes().is_empty());
            assert!(result.rows().is_empty());
            let same = ripple_git::revisions::Repository::open(root)
                .unwrap()
                .compare(&ComparisonSpec::parse(&format!("{a}..{a}")).unwrap())
                .unwrap();
            let x = revision_impact::analyze(
                &same.changes,
                view(&old),
                view(&old),
                QueryLimits::default(),
            )
            .unwrap();
            assert!(x.rows().is_empty());
        }
        if case["file_only"] == true {
            assert!(result.evidence().symbol_backed().is_empty());
            assert!(
                result
                    .tests()
                    .module_fallback
                    .iter()
                    .any(|n| n.path() == &p("smoke.test.ts"))
            );
        }
        if case["incomplete"] == true {
            assert!(result.incomplete());
            assert!(!result.base().query.impact.unresolved.is_empty());
            assert!(!result.head().query.impact.unresolved.is_empty());
        }
        if case["global_invalidation"] == true {
            assert!(result.changes().global_invalidation());
            assert!(result.evidence().symbol_backed().is_empty());
            assert!(!result.base().global_fallback.is_empty());
            assert!(!result.head().global_fallback.is_empty());
            assert!(
                result
                    .evidence()
                    .module_fallback()
                    .iter()
                    .all(|w| w.evidence_kind == EvidenceKind::ModuleFallback)
            );
            assert!(!result.tests().active_head.is_empty());
            assert!(old.global_inputs().contains(&p("settings/base.json")));
            let target = |s: &RevisionSnapshot| {
                s.index()
                    .graph()
                    .edges()
                    .iter()
                    .find(|e| e.kind == EdgeKind::Calls && e.source.path() == &p("caller.ts"))
                    .unwrap()
                    .target
                    .path()
                    .clone()
            };
            assert_eq!(target(&old), p("one.ts"));
            assert_eq!(target(&new), p("two.ts"));
        }
        // Display limiting never reduces complete evidence, candidates or graph traversal.
        let hidden = revision_impact::analyze(
            &trees.changes,
            view(&old),
            view(&new),
            QueryLimits {
                depth: None,
                display_limit: 0,
            },
        )
        .unwrap();
        assert!(hidden.displayed().is_empty());
        assert_eq!(hidden.tests(), result.tests());
        assert_eq!(hidden.evidence(), result.evidence());
        assert_eq!(hidden.rows(), result.rows());
        assert_eq!(hidden.display_omitted(), hidden.rows().len());
        if case["name"] == "removed_call" {
            let limited = revision_impact::analyze(
                &trees.changes,
                view(&old),
                view(&new),
                QueryLimits {
                    depth: Some(0),
                    display_limit: 100,
                },
            )
            .unwrap();
            assert!(limited.incomplete());
            assert!(
                !limited
                    .base()
                    .query
                    .impact
                    .truncation
                    .depth_boundary_nodes
                    .is_empty()
            );
            assert!(
                !limited
                    .head()
                    .query
                    .impact
                    .truncation
                    .depth_boundary_nodes
                    .is_empty()
            );
        }
        let repeat = snapshots::build(&trees.base).unwrap();
        assert_eq!(
            CanonicalState::from_snapshot(old.index()),
            CanonicalState::from_snapshot(repeat.index())
        );
    }
}
fn file_bytes(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut result = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            if e.file_type().unwrap().is_dir() {
                stack.push(e.path());
            } else {
                result.insert(
                    e.path().strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(e.path()).unwrap(),
                );
            }
        }
    }
    result
}
#[test]
fn revision_builds_and_queries_never_publish_or_change_existing_worktree_state() {
    let d = repo();
    let r = d.path();
    fs::write(r.join(".gitignore"), ".ripple/\n").unwrap();
    fs::write(r.join("lib.ts"), "export function f() { return 1; }\n").unwrap();
    fs::write(r.join("smoke.test.ts"), "import { f } from './lib'; f();\n").unwrap();
    let a = commit(r);
    fs::write(r.join("lib.ts"), "export function f() { return 2; }\n").unwrap();
    let b = commit(r);
    build::cold_index(r, ripple_git::worktree::observe).unwrap();
    let local = file_bytes(&r.join(".ripple"));
    fs::write(r.join("lib.ts"), "STAGED").unwrap();
    git(r, &["add", "lib.ts"]);
    fs::write(r.join("lib.ts"), "DIRTY\r\n").unwrap();
    fs::write(r.join("untracked.ts"), "UNTRACKED").unwrap();
    let index = fs::read(r.join(".git/index")).unwrap();
    let head = fs::read(r.join(".git/HEAD")).unwrap();
    let refs = git(r, &["show-ref"]);
    let status = git(r, &["status", "--porcelain=v1", "-z"]);
    let (trees, old, new) = snapshots(r, &a, &b);
    revision_impact::analyze(
        &trees.changes,
        view(&old),
        view(&new),
        QueryLimits::default(),
    )
    .unwrap();
    assert_eq!(local, file_bytes(&r.join(".ripple")));
    assert_eq!(index, fs::read(r.join(".git/index")).unwrap());
    assert_eq!(head, fs::read(r.join(".git/HEAD")).unwrap());
    assert_eq!(refs, git(r, &["show-ref"]));
    assert_eq!(status, git(r, &["status", "--porcelain=v1", "-z"]));
    assert_eq!(fs::read(r.join("lib.ts")).unwrap(), b"DIRTY\r\n");
    assert_eq!(fs::read(r.join("untracked.ts")).unwrap(), b"UNTRACKED");
}
#[test]
fn view_provenance_global_inputs_and_presence_cannot_be_silently_mislabelled() {
    let d = repo();
    let r = d.path();
    fs::write(r.join("lib.ts"), "export function f() {}\n").unwrap();
    fs::write(r.join("package.json"), "{}").unwrap();
    let a = commit(r);
    fs::write(r.join("lib.ts"), "export function f() { return 1; }\n").unwrap();
    let b = commit(r);
    let (trees, old, new) = snapshots(r, &a, &b);
    assert!(
        revision_impact::analyze(
            &trees.changes,
            view(&new),
            view(&old),
            QueryLimits::default()
        )
        .is_err()
    );
    let empty = Default::default();
    let mut bad = view(&old);
    bad.global_inputs = &empty;
    assert!(
        revision_impact::analyze(&trees.changes, bad, view(&new), QueryLimits::default()).is_err()
    );
    let only_sources = old.index().files().iter().map(|f| f.path.clone()).collect();
    let mut bad = view(&old);
    bad.existing_paths = &only_sources;
    assert!(
        revision_impact::analyze(&trees.changes, bad, view(&new), QueryLimits::default()).is_err()
    );
    let mut bad = view(&old);
    bad.existing_paths = &empty;
    assert!(
        revision_impact::analyze(&trees.changes, bad, view(&new), QueryLimits::default()).is_err()
    );
    build::cold_index(r, ripple_git::worktree::observe).unwrap();
    let current = build::load_current(r, ripple_git::worktree::observe).unwrap();
    let mut bad = view(&old);
    bad.index = &current.loaded.snapshot;
    assert!(
        revision_impact::analyze(&trees.changes, bad, view(&new), QueryLimits::default()).is_err()
    );
}
#[cfg(unix)]
#[test]
fn committed_symlinks_are_unindexed_presence_not_deleted_and_config_links_fail() {
    use std::os::unix::fs::symlink;
    let d = repo();
    let r = d.path();
    fs::write(r.join("lib.ts"), "export function f() { return 1; }\n").unwrap();
    fs::write(r.join("smoke.test.ts"), "import { f } from './lib'; f();\n").unwrap();
    let a = commit(r);
    fs::remove_file(r.join("smoke.test.ts")).unwrap();
    symlink("../../outside.ts", r.join("smoke.test.ts")).unwrap();
    fs::write(r.join("lib.ts"), "export function f() { return 2; }\n").unwrap();
    let b = commit(r);
    let (trees, old, new) = snapshots(r, &a, &b);
    let result = revision_impact::analyze(
        &trees.changes,
        view(&old),
        view(&new),
        QueryLimits::default(),
    )
    .unwrap();
    assert!(result.incomplete());
    assert!(result.tests().base_only.is_empty());
    assert!(!result.tests().present_unindexed_head.is_empty());
    symlink("../../outside-config.json", r.join("tsconfig.json")).unwrap();
    let c = commit(r);
    let trees = ripple_git::revisions::Repository::open(r)
        .unwrap()
        .compare(&ComparisonSpec::parse(&format!("{b}..{c}")).unwrap())
        .unwrap();
    assert!(snapshots::build(&trees.head).is_err());
}
