use crate::changes::*;
use ripple_core::{
    identity::{ProjectKey, RepoPath},
    revision::{
        CommitId, ComparisonSpec, DiffHunk, FileChange, FileChangeKind, LineRange, RevisionChanges,
        RevisionPair,
    },
    source::{SourceFile, SourceSnapshot},
    syntax::FileFacts,
};
use std::{collections::BTreeSet, fs, path::Path, process::Command};
fn p(s: &str) -> RepoPath {
    RepoPath::new(s).unwrap()
}
fn facts(sources: &SourceSnapshot) -> Vec<FileFacts> {
    sources
        .files()
        .map(|s| {
            let parsed =
                ripple_parse::parser::parse(s.path().clone(), s.text().as_bytes()).unwrap();
            ripple_parse::extract(&parsed, ProjectKey::single_project()).unwrap()
        })
        .collect()
}
fn id(n: char) -> CommitId {
    CommitId::new(n.to_string().repeat(40)).unwrap()
}
fn git(root: &Path, args: &[&str]) -> String {
    let mut c = Command::new("git");
    for (k, _) in std::env::vars_os() {
        if k.to_string_lossy().to_ascii_uppercase().starts_with("GIT_") {
            c.env_remove(k);
        }
    }
    let o = c
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
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap()
}
fn write(root: &Path, files: &serde_json::Value) {
    for (path, text) in files.as_object().unwrap() {
        let dest = root.join(path);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::write(dest, text.as_str().unwrap()).unwrap();
    }
}
fn commit(root: &Path) -> String {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "fixture", "--allow-empty"]);
    git(root, &["rev-parse", "HEAD"]).trim().into()
}
#[test]
fn git_backed_independent_classification_goldens() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/changes/cases.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q"]);
        git(root, &["config", "user.name", "Ripple Fixture"]);
        git(root, &["config", "user.email", "fixture@ripple.invalid"]);
        write(root, &case["before"]);
        let a = commit(root);
        for name in case["before"].as_object().unwrap().keys() {
            fs::remove_file(root.join(name)).unwrap();
        }
        write(root, &case["after"]);
        let b = commit(root);
        let repo = ripple_git::revisions::Repository::open(root).unwrap();
        let snapshots = repo
            .compare(&ComparisonSpec::parse(&format!("{a}..{b}")).unwrap())
            .unwrap();
        let old = ripple_parse::scanner::discover(&snapshots.base)
            .unwrap()
            .sources;
        let new = ripple_parse::scanner::discover(&snapshots.head)
            .unwrap()
            .sources;
        let old_facts = facts(&old);
        let new_facts = facts(&new);
        let global = case["global_inputs"]
            .as_array()
            .map(|xs| xs.iter().map(|x| p(x.as_str().unwrap())).collect())
            .unwrap_or_default();
        let classify_current = |af: &[FileFacts], bf: &[FileFacts]| {
            classify(
                &snapshots.changes,
                Endpoint {
                    revision: snapshots.base.revision(),
                    analyzer_version: "fixture-v1",
                    sources: &old,
                    facts: af,
                    global_inputs: &global,
                },
                Endpoint {
                    revision: snapshots.head.revision(),
                    analyzer_version: "fixture-v1",
                    sources: &new,
                    facts: bf,
                    global_inputs: &global,
                },
            )
            .unwrap()
        };
        let result = classify_current(&old_facts, &new_facts);
        let mut actual = result
            .symbols()
            .iter()
            .map(|s| serde_json::json!([s.identity.qualified_name().display_name(), s.kind]))
            .collect::<Vec<_>>();
        actual.sort_by_key(serde_json::Value::to_string);
        let mut expected = case["expected"].as_array().unwrap().clone();
        expected.sort_by_key(serde_json::Value::to_string);
        assert_eq!(actual, expected, "case {}", case["name"]);
        assert_eq!(result.files().len(), snapshots.changes.files().len());
        if case["both_changed"] == true {
            let s = &result.symbols()[0];
            assert_eq!(s.signature_changed, Some(true));
            assert_eq!(s.body_changed, Some(true));
        }
        if case["outside"] == true {
            assert!(
                result
                    .files()
                    .iter()
                    .any(|f| f.reasons.contains(&Reason::OutsideSymbols))
            );
        }
        if case["global_invalidation"] == true {
            assert!(result.global_invalidation());
        }
        if case["incomplete"] == true {
            assert!(result.incomplete());
        }
        if case["rename"] == true {
            assert!(
                result.files()[0]
                    .reasons
                    .contains(&Reason::RenameAsDeleteAdd)
            );
            assert_ne!(
                result.symbols()[0].identity.path(),
                result.symbols()[1].identity.path()
            );
        }
        // Ordering of input files does not change the result or JSON report.
        let mut reversed_old = old_facts.clone();
        reversed_old.reverse();
        let mut reversed_new = new_facts.clone();
        reversed_new.reverse();
        assert_eq!(result, classify_current(&reversed_old, &reversed_new));
    }
}
fn simple_inputs() -> (
    SourceSnapshot,
    SourceSnapshot,
    Vec<FileFacts>,
    Vec<FileFacts>,
    RevisionChanges,
) {
    let a = SourceSnapshot::new([SourceFile::new(
        p("a.ts"),
        "function f() { return 1; }\n".into(),
    )])
    .unwrap();
    let b = SourceSnapshot::new([SourceFile::new(
        p("a.ts"),
        "function f() { return 2; }\n".into(),
    )])
    .unwrap();
    let af = facts(&a);
    let bf = facts(&b);
    let changes = RevisionChanges::new(
        RevisionPair {
            base: id('a'),
            head: id('b'),
        },
        vec![FileChange {
            old_path: Some(p("a.ts")),
            new_path: Some(p("a.ts")),
            kind: FileChangeKind::Modified,
            rename_similarity: None,
            hunks: vec![DiffHunk {
                old: LineRange { start: 1, count: 1 },
                new: LineRange { start: 1, count: 1 },
            }],
        }],
    )
    .unwrap();
    (a, b, af, bf, changes)
}
#[test]
fn invalid_endpoint_provenance_spans_and_inventory_fail() {
    let (a, b, af, bf, c) = simple_inputs();
    let globals = BTreeSet::new();
    let run = |revision: &CommitId, version: &str, old: &[FileFacts], new: &[FileFacts]| {
        classify(
            &c,
            Endpoint {
                revision,
                analyzer_version: version,
                sources: &a,
                facts: old,
                global_inputs: &globals,
            },
            Endpoint {
                revision: &c.revisions().head,
                analyzer_version: "v1",
                sources: &b,
                facts: new,
                global_inputs: &globals,
            },
        )
    };
    assert!(run(&id('c'), "v1", &af, &bf).is_err());
    assert!(run(&c.revisions().base, "other", &af, &bf).is_err());
    assert!(run(&c.revisions().base, "v1", &[], &bf).is_err());
    let mut bad = af.clone();
    bad[0].path = p("other.ts");
    assert!(run(&c.revisions().base, "v1", &bad, &bf).is_err());
    let mut duplicate = af.clone();
    duplicate.push(af[0].clone());
    assert!(run(&c.revisions().base, "v1", &duplicate, &bf).is_err());
}
#[test]
fn hash_format_mismatch_is_uncertain_never_unchanged() {
    let (a, b, af, mut bf, c) = simple_inputs();
    bf[0].symbols[0].signature_hash.as_mut().unwrap().version += 1;
    let global = BTreeSet::new();
    let result = classify(
        &c,
        Endpoint {
            revision: &c.revisions().base,
            analyzer_version: "v1",
            sources: &a,
            facts: &af,
            global_inputs: &global,
        },
        Endpoint {
            revision: &c.revisions().head,
            analyzer_version: "v1",
            sources: &b,
            facts: &bf,
            global_inputs: &global,
        },
    )
    .unwrap();
    assert!(result.incomplete());
    assert_eq!(result.symbols()[0].kind, ChangeKind::Uncertain);
    assert_eq!(result.symbols()[0].signature_changed, None);
}
#[test]
fn zero_width_anchors_and_out_of_bounds_are_checked() {
    let (a, _, af, _, _) = simple_inputs();
    // Insert a complete line at file start; old anchor 0 is not a symbol owner.
    let b = SourceSnapshot::new([SourceFile::new(
        p("a.ts"),
        "// 😀\nfunction f() { return 1; }\n".into(),
    )])
    .unwrap();
    let bf = facts(&b);
    let global = BTreeSet::new();
    let make = |line| {
        RevisionChanges::new(
            RevisionPair {
                base: id('a'),
                head: id('b'),
            },
            vec![FileChange {
                old_path: Some(p("a.ts")),
                new_path: Some(p("a.ts")),
                kind: FileChangeKind::Modified,
                rename_similarity: None,
                hunks: vec![DiffHunk {
                    old: LineRange {
                        start: line,
                        count: 0,
                    },
                    new: LineRange { start: 1, count: 1 },
                }],
            }],
        )
        .unwrap()
    };
    let run = |c: &RevisionChanges| {
        classify(
            c,
            Endpoint {
                revision: &c.revisions().base,
                analyzer_version: "v1",
                sources: &a,
                facts: &af,
                global_inputs: &global,
            },
            Endpoint {
                revision: &c.revisions().head,
                analyzer_version: "v1",
                sources: &b,
                facts: &bf,
                global_inputs: &global,
            },
        )
    };
    let result = run(&make(0)).unwrap();
    assert!(result.files()[0].hunks[0].base.owners.is_empty());
    assert!(result.files()[0].reasons.contains(&Reason::BoundaryAnchor));
    assert!(run(&make(100)).is_err());
}

#[test]
fn absent_fingerprints_do_not_fabricate_a_change_reason() {
    let (a, b, mut af, mut bf, c) = simple_inputs();
    for f in af.iter_mut().chain(bf.iter_mut()) {
        for s in &mut f.symbols {
            s.signature_hash = None;
            s.body_hash = None;
        }
    }
    let global = BTreeSet::new();
    let result = classify(
        &c,
        Endpoint {
            revision: &c.revisions().base,
            analyzer_version: "v1",
            sources: &a,
            facts: &af,
            global_inputs: &global,
        },
        Endpoint {
            revision: &c.revisions().head,
            analyzer_version: "v1",
            sources: &b,
            facts: &bf,
            global_inputs: &global,
        },
    )
    .unwrap();
    assert_eq!(result.symbols()[0].kind, ChangeKind::Uncertain);
    assert!(
        !result.symbols()[0]
            .reasons
            .contains(&Reason::FingerprintChanged)
    );
    assert!(result.incomplete());
}
#[test]
fn ambiguous_ownership_is_visible_and_never_chooses_a_tie() {
    use ripple_core::identity::{QualifiedName, SymbolId, SymbolKind};
    let (a, b, mut af, mut bf, c) = simple_inputs();
    for f in af.iter_mut().chain(bf.iter_mut()) {
        let mut duplicate = f.symbols[0].clone();
        duplicate.identity = SymbolId::new(
            ProjectKey::single_project(),
            p("a.ts"),
            QualifiedName::new(["g".into()]).unwrap(),
            SymbolKind::Function,
        );
        f.symbols.push(duplicate);
    }
    let global = BTreeSet::new();
    let result = classify(
        &c,
        Endpoint {
            revision: &c.revisions().base,
            analyzer_version: "v1",
            sources: &a,
            facts: &af,
            global_inputs: &global,
        },
        Endpoint {
            revision: &c.revisions().head,
            analyzer_version: "v1",
            sources: &b,
            facts: &bf,
            global_inputs: &global,
        },
    )
    .unwrap();
    assert!(result.incomplete());
    assert!(
        result.files()[0]
            .reasons
            .contains(&Reason::AmbiguousOwnership)
    );
    assert!(result.files()[0].hunks[0].base.owners.is_empty());
}
#[test]
fn identical_revisions_are_empty_and_cannot_hide_different_sources() {
    let (a, b, af, bf, _) = simple_inputs();
    let global = BTreeSet::new();
    let revision = id('a');
    let c = RevisionChanges::new(
        RevisionPair {
            base: revision.clone(),
            head: revision.clone(),
        },
        vec![],
    )
    .unwrap();
    let run = |new: &SourceSnapshot, facts: &[FileFacts]| {
        classify(
            &c,
            Endpoint {
                revision: &revision,
                analyzer_version: "v1",
                sources: &a,
                facts: &af,
                global_inputs: &global,
            },
            Endpoint {
                revision: &revision,
                analyzer_version: "v1",
                sources: new,
                facts,
                global_inputs: &global,
            },
        )
    };
    let result = run(&a, &af).unwrap();
    assert!(result.files().is_empty());
    assert!(result.symbols().is_empty());
    assert!(run(&b, &bf).is_err());
}
