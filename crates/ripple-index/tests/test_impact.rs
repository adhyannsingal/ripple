use ripple_core::{
    graph::Confidence,
    identity::RepoPath,
    results::{QueryLimits, RevisionSide},
    revision::{ComparisonSpec, RevisionGraphs},
};
use ripple_impact::{
    revision_impact::{self, RevisionView},
    tests::{self, TestPresence},
};
use ripple_index::snapshots::{self, RevisionSnapshot};
use std::{collections::BTreeSet, fs, path::Path, process::Command};
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

#[test]
fn revision_test_selection_keeps_partitions_witnesses_uncertainty_and_limits() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/test-impact/cases.json")).unwrap();
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
        let candidate_index = |snapshot: &RevisionSnapshot| {
            let mut data: ripple_core::index::SnapshotData =
                serde_json::from_value(serde_json::to_value(snapshot.index()).unwrap()).unwrap();
            if case["candidate"] == true {
                let mut edges = data.graph.edges().to_vec();
                edges
                    .iter_mut()
                    .find(|e| e.kind == ripple_core::graph::EdgeKind::Calls)
                    .unwrap()
                    .confidence = Confidence::Candidate;
                data.graph = ripple_core::graph::GraphSnapshot::new(
                    data.graph.nodes().to_vec(),
                    edges,
                    data.graph.unresolved().to_vec(),
                )
                .unwrap();
            }
            data.validate(snapshot.sources()).unwrap()
        };
        let old_index = candidate_index(&old);
        let new_index = candidate_index(&new);
        let mut old_view = view(&old);
        old_view.index = &old_index;
        let mut new_view = view(&new);
        new_view.index = &new_index;
        let result = revision_impact::analyze(
            &trees.changes,
            old_view,
            new_view,
            QueryLimits {
                depth: case["depth"].as_u64().map(|v| v as usize),
                display_limit: case["limit"].as_u64().unwrap_or(100) as usize,
            },
        )
        .unwrap();
        let selection = tests::select(&result);
        let expected = |key: &str| {
            case[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p.as_str().unwrap().to_string())
                .collect::<BTreeSet<_>>()
        };
        let actual = |set: &BTreeSet<RepoPath>| {
            set.iter()
                .map(|p| p.as_str().to_string())
                .collect::<BTreeSet<_>>()
        };
        assert_eq!(
            actual(&selection.active_head.symbol_backed),
            expected("symbol_backed"),
            "{} symbol",
            case["name"]
        );
        assert_eq!(
            actual(&selection.active_head.fallback_only),
            expected("fallback_only"),
            "{} fallback",
            case["name"]
        );
        assert_eq!(
            actual(&selection.base_only.combined),
            expected("base_only"),
            "{} deleted",
            case["name"]
        );
        assert_eq!(
            actual(&selection.present_unindexed_head.combined),
            expected("present_unindexed"),
            "{} unindexed",
            case["name"]
        );
        let union = selection
            .active_head
            .symbol_backed
            .union(&selection.active_head.module_fallback)
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(selection.active_head.combined, union);
        if case["incomplete"] == true {
            assert!(
                selection.completeness.analysis_incomplete,
                "{}",
                case["name"]
            );
            assert!(!selection.completeness.reasons.is_empty());
        }
        assert_eq!(
            selection.completeness.depth_truncated,
            case["truncated"].as_bool().unwrap_or(false)
        );
        let bound =
            RevisionGraphs::new(trees.changes.revisions().clone(), &old_index, &new_index).unwrap();
        for file in &selection.files {
            assert!(!file.symbol_backed.is_empty() || !file.module_fallback.is_empty());
            for w in file.symbol_backed.iter().chain(&file.module_fallback) {
                bound.validate_witness(w).unwrap();
                assert_eq!(w.target, file.file);
            }
            if file.presence == TestPresence::AbsentHead {
                assert!(
                    file.symbol_backed
                        .iter()
                        .chain(&file.module_fallback)
                        .all(|w| w.revision == RevisionSide::Base)
                );
            }
        }
        if let Some(paths) = case["base_symbol_only"].as_array() {
            for path in paths {
                let file = selection
                    .files
                    .iter()
                    .find(|f| f.file.path().as_str() == path.as_str().unwrap())
                    .unwrap();
                assert!(!file.symbol_backed.is_empty());
                assert!(
                    file.symbol_backed
                        .iter()
                        .all(|w| w.revision == RevisionSide::Base)
                );
            }
        }
        if case["candidate"] == true {
            assert!(
                selection
                    .files
                    .iter()
                    .flat_map(|f| &f.symbol_backed)
                    .any(|w| w.confidence() == Confidence::Candidate)
            );
        }
        if case["limit"] == 0 {
            assert_eq!(selection.active_head.combined.len(), 2);
            assert!(result.displayed().is_empty());
        }
    }
}
