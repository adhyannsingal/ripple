use ripple_core::{
    canonical::CanonicalState, graph::*, identity::*, index::*, results::*, revision::*, source::*,
    syntax::*,
};

const FUNCTION: &str = "export function gone() { return 1; }\n";
const TEST: &str = "import { gone } from '../src/gone';\ngone();\n";
fn path(s: &str) -> RepoPath {
    RepoPath::new(s).unwrap()
}
fn digest(s: &str) -> VersionedHash {
    VersionedHash {
        version: 1,
        digest: s.repeat(64),
    }
}
fn commit(s: &str) -> CommitId {
    CommitId::new(s.repeat(40)).unwrap()
}
fn versions() -> IndexVersions {
    IndexVersions {
        schema_version: SCHEMA_VERSION,
        hash_version: HASH_VERSION,
        analyzer_version: "test-analyzer-v1".into(),
    }
}
fn location(file: &str, text: &str, a: usize, b: usize) -> SourceLocation {
    SourceLocation {
        file: path(file),
        span: SourceSpan::from_offsets(text, a, b).unwrap(),
    }
}
fn metadata(origin: SnapshotOrigin) -> IndexMetadata {
    IndexMetadata {
        project: ProjectKey::single_project(),
        origin,
        versions: versions(),
        effective_config_hash: Some(digest("c")),
        generation: 1,
        created_at: "2026-10-29T00:00:00Z".into(),
    }
}
fn file_input(p: &str, text: &str, letter: &str) -> CapturedInput {
    CapturedInput {
        path: path(p),
        role: InputRole::Source,
        byte_len: text.len() as u64,
        content_hash: digest(letter),
    }
}

fn fixture(origin: SnapshotOrigin) -> (SnapshotData, SourceSnapshot, Witness) {
    let project = ProjectKey::single_project();
    let source = SourceFile::new(path("src/gone.ts"), FUNCTION.into());
    let test = SourceFile::new(path("tests/gone.test.ts"), TEST.into());
    let f = GraphNode::file(project.clone(), source.path().clone(), false);
    let t = GraphNode::file(project.clone(), test.path().clone(), true);
    let symbol_id = SymbolId::new(
        project,
        source.path().clone(),
        QualifiedName::new(["gone".into()]).unwrap(),
        SymbolKind::Function,
    );
    let symbol = GraphNode::symbol(symbol_id.clone(), source.full_span());
    let definition = Edge {
        source: f.id.clone(),
        target: symbol.id.clone(),
        kind: EdgeKind::Defines,
        confidence: Confidence::Resolved,
        provenance: location("src/gone.ts", FUNCTION, 0, FUNCTION.len()),
        resolution_rule: "function_declaration".into(),
    };
    let call_start = TEST.rfind("gone()").unwrap();
    let call = Edge {
        source: t.id.clone(),
        target: symbol.id.clone(),
        kind: EdgeKind::Calls,
        confidence: Confidence::Resolved,
        provenance: location("tests/gone.test.ts", TEST, call_start, call_start + 6),
        resolution_rule: "named_import_direct_call".into(),
    };
    let imports = Edge {
        source: t.id.clone(),
        target: f.id.clone(),
        kind: EdgeKind::Imports,
        confidence: Confidence::Resolved,
        provenance: location("tests/gone.test.ts", TEST, 0, TEST.find('\n').unwrap()),
        resolution_rule: "relative_import".into(),
    };
    let mut facts = FileFacts::empty(source.path().clone(), Language::TypeScript);
    facts.symbols.push(SymbolFact {
        identity: symbol_id,
        span: source.full_span(),
        signature_span: None,
        body_span: None,
        signature_hash: None,
        body_hash: None,
        exported: true,
    });
    let mut test_facts = FileFacts::empty(test.path().clone(), Language::TypeScript);
    test_facts.references.push(ReferenceFact {
        text: "gone".into(),
        kind: ReferenceKind::Call,
        span: call.provenance.span,
        owner: ReferenceOwner::File,
        scope: None,
    });
    let graph = GraphSnapshot::new(
        vec![f, t.clone(), symbol.clone()],
        vec![definition, imports, call.clone()],
        vec![],
    )
    .unwrap();
    let manifest = InputManifest::new(vec![
        file_input("src/gone.ts", FUNCTION, "a"),
        file_input("tests/gone.test.ts", TEST, "b"),
        CapturedInput {
            path: path("tsconfig.json"),
            role: InputRole::Configuration,
            byte_len: 2,
            content_hash: digest("c"),
        },
    ])
    .unwrap();
    let data = SnapshotData {
        metadata: metadata(origin),
        inputs: manifest,
        files: vec![test_facts, facts],
        graph,
    };
    let sources = SourceSnapshot::new([test, source]).unwrap();
    let witness = Witness {
        seed: symbol.id,
        target: t.id,
        revision: RevisionSide::Base,
        evidence_kind: EvidenceKind::SymbolBacked,
        hops: vec![call],
    };
    (data, sources, witness)
}
fn head_without_export() -> (SnapshotData, SourceSnapshot) {
    let source = SourceFile::new(path("tests/gone.test.ts"), TEST.into());
    let file = GraphNode::file(ProjectKey::single_project(), source.path().clone(), true);
    let call_start = TEST.rfind("gone()").unwrap();
    let provenance = location("tests/gone.test.ts", TEST, call_start, call_start + 6);
    let boundary = GraphNode {
        id: NodeId::Opaque {
            project: ProjectKey::single_project(),
            path: source.path().clone(),
            reference_key: "call:gone:0".into(),
        },
        kind: NodeKind::OpaqueBoundary,
        span: Some(provenance.span),
    };
    let unresolved = UnresolvedReference {
        source: file.id.clone(),
        boundary: boundary.id.clone(),
        text: "gone".into(),
        reference_kind: ReferenceKind::Call,
        provenance,
        reason: "provider was deleted".into(),
    };
    let graph = GraphSnapshot::new(vec![file, boundary], vec![], vec![unresolved]).unwrap();
    let data = SnapshotData {
        metadata: metadata(SnapshotOrigin::Commit {
            revision: commit("b"),
        }),
        inputs: InputManifest::new(vec![file_input("tests/gone.test.ts", TEST, "b")]).unwrap(),
        files: vec![FileFacts::empty(
            source.path().clone(),
            Language::TypeScript,
        )],
        graph,
    };
    (data, SourceSnapshot::new([source]).unwrap())
}

#[test]
fn deleted_symbols_remain_explainable_only_in_their_base_graph() {
    let (base, sources, witness) = fixture(SnapshotOrigin::Commit {
        revision: commit("a"),
    });
    let base = base.validate(&sources).unwrap();
    let (head, sources) = head_without_export();
    let head = head.validate(&sources).unwrap();
    let revisions = RevisionPair {
        base: commit("a"),
        head: commit("b"),
    };
    let graphs = RevisionGraphs::new(revisions.clone(), &base, &head).unwrap();
    graphs.validate_witness(&witness).unwrap();
    assert!(
        graphs
            .validate_witness(&Witness {
                revision: RevisionSide::Head,
                ..witness.clone()
            })
            .is_err()
    );
    assert!(
        graphs
            .validate_witness(&Witness {
                revision: RevisionSide::Worktree,
                ..witness
            })
            .is_err()
    );
    assert_eq!(graphs.revisions(), &revisions);
    assert_eq!(head.graph().unresolved()[0].reason, "provider was deleted");
    assert!(
        RevisionGraphs::new(
            RevisionPair {
                base: commit("c"),
                head: commit("b")
            },
            &base,
            &head
        )
        .is_err()
    );
}

#[test]
fn worktree_generations_cannot_masquerade_as_immutable_commits() {
    let origin = SnapshotOrigin::Worktree {
        head: Some(commit("a")),
        dirty: true,
    };
    let (data, sources, mut witness) = fixture(origin);
    let worktree = data.validate(&sources).unwrap();
    witness.revision = RevisionSide::Worktree;
    worktree.validate_worktree_witness(&witness).unwrap();
    let (data, sources, _) = fixture(SnapshotOrigin::Commit {
        revision: commit("a"),
    });
    let base = data.validate(&sources).unwrap();
    assert!(base.validate_worktree_witness(&witness).is_err());
    assert!(
        RevisionGraphs::new(
            RevisionPair {
                base: commit("a"),
                head: commit("a")
            },
            &base,
            &worktree
        )
        .is_err()
    );
}

#[test]
fn equality_ignores_generation_origin_and_order_but_freshness_does_not() {
    let (mut data, sources, _) = fixture(SnapshotOrigin::Commit {
        revision: commit("a"),
    });
    for code in ["z_note", "a_note"] {
        data.files[0].diagnostics.push(Diagnostic {
            code: code.into(),
            severity: Severity::Warning,
            message: code.into(),
            span: None,
        });
    }
    let first = data.clone().validate(&sources).unwrap();
    data.metadata.generation = 42;
    data.metadata.created_at = "2026-10-30T00:00:00Z".into();
    data.metadata.origin = SnapshotOrigin::Worktree {
        head: Some(commit("b")),
        dirty: true,
    };
    data.files[0].diagnostics.reverse();
    data.files.reverse();
    data.graph = GraphSnapshot::new(
        data.graph.nodes().iter().rev().cloned().collect(),
        data.graph.edges().iter().rev().cloned().collect(),
        vec![],
    )
    .unwrap();
    let second = data.validate(&sources).unwrap();
    assert_ne!(first, second);
    assert_eq!(
        CanonicalState::from_snapshot(&first),
        CanonicalState::from_snapshot(&second)
    );
    assert!(
        first
            .check_observation(second.inputs(), &second.metadata().origin)
            .is_err()
    );
    let canonical = CanonicalState::from_snapshot(&first);
    assert_eq!(
        canonical,
        serde_json::from_str(&serde_json::to_string(&canonical).unwrap()).unwrap()
    );
}

#[test]
fn serialization_preserves_validated_semantic_state() {
    let (data, sources, _) = fixture(SnapshotOrigin::Worktree {
        head: None,
        dirty: true,
    });
    let snapshot = data.validate(&sources).unwrap();
    let restored: SnapshotData =
        serde_json::from_str(&serde_json::to_string(&snapshot).unwrap()).unwrap();
    assert_eq!(snapshot, restored.validate(&sources).unwrap());
}

#[test]
fn canonical_state_retains_confidence_diagnostics_config_and_input_hashes() {
    let (data, sources, _) = fixture(SnapshotOrigin::Commit {
        revision: commit("a"),
    });
    let baseline = CanonicalState::from_snapshot(&data.clone().validate(&sources).unwrap());
    let mut changed = data.clone();
    let mut edges = changed.graph.edges().to_vec();
    edges
        .iter_mut()
        .find(|e| e.kind == EdgeKind::Calls)
        .unwrap()
        .confidence = Confidence::Candidate;
    changed.graph = GraphSnapshot::new(changed.graph.nodes().to_vec(), edges, vec![]).unwrap();
    assert_ne!(
        baseline,
        CanonicalState::from_snapshot(&changed.validate(&sources).unwrap())
    );
    let mut changed = data.clone();
    changed.files[0].diagnostics.push(Diagnostic {
        code: "unsupported".into(),
        severity: Severity::Warning,
        message: "unmodelled syntax".into(),
        span: None,
    });
    assert_ne!(
        baseline,
        CanonicalState::from_snapshot(&changed.validate(&sources).unwrap())
    );
    let mut changed = data.clone();
    let mut edges = changed.graph.edges().to_vec();
    let call = edges
        .iter_mut()
        .find(|e| e.kind == EdgeKind::Calls)
        .unwrap();
    let start = call.provenance.span.start().byte;
    call.provenance.span = SourceSpan::from_offsets(TEST, start, start + 7).unwrap();
    changed.graph = GraphSnapshot::new(changed.graph.nodes().to_vec(), edges, vec![]).unwrap();
    assert_ne!(
        baseline,
        CanonicalState::from_snapshot(&changed.validate(&sources).unwrap())
    );
    let mut changed = data.clone();
    changed.files[0].parse_status = ParseStatus::Recovered;
    changed.files[0].diagnostics.push(Diagnostic {
        code: "parse_recovery".into(),
        severity: Severity::Warning,
        message: "partial facts".into(),
        span: None,
    });
    assert_ne!(
        baseline,
        CanonicalState::from_snapshot(&changed.validate(&sources).unwrap())
    );
    let mut changed = data.clone();
    changed.metadata.effective_config_hash = Some(digest("d"));
    assert_ne!(
        baseline,
        CanonicalState::from_snapshot(&changed.validate(&sources).unwrap())
    );
    let mut changed = data.clone();
    let mut entries = changed.inputs.entries().to_vec();
    entries[0].content_hash = digest("e");
    changed.inputs = InputManifest::new(entries).unwrap();
    assert_ne!(
        baseline,
        CanonicalState::from_snapshot(&changed.validate(&sources).unwrap())
    );
    let (mut changed, sources) = head_without_export();
    let first = CanonicalState::from_snapshot(&changed.clone().validate(&sources).unwrap());
    let mut unknown = changed.graph.unresolved().to_vec();
    unknown[0].reason = "different missing-target reason".into();
    changed.graph = GraphSnapshot::new(changed.graph.nodes().to_vec(), vec![], unknown).unwrap();
    assert_ne!(
        first,
        CanonicalState::from_snapshot(&changed.validate(&sources).unwrap())
    );
}

#[test]
fn observations_detect_edit_add_delete_config_and_head_changes() {
    let (data, sources, _) = fixture(SnapshotOrigin::Worktree {
        head: Some(commit("a")),
        dirty: false,
    });
    let snapshot = data.validate(&sources).unwrap();
    snapshot
        .check_observation(snapshot.inputs(), &snapshot.metadata().origin)
        .unwrap();
    let original = snapshot.inputs().entries().to_vec();
    let mut edit = original.clone();
    edit[0].content_hash = digest("d");
    let mut add = original.clone();
    add.push(file_input("src/new.ts", "x", "e"));
    let mut delete = original.clone();
    delete.remove(0);
    let mut config = original;
    config
        .iter_mut()
        .find(|i| i.role == InputRole::Configuration)
        .unwrap()
        .content_hash = digest("f");
    for inputs in [edit, add, delete, config] {
        assert!(
            snapshot
                .check_observation(
                    &InputManifest::new(inputs).unwrap(),
                    &snapshot.metadata().origin
                )
                .is_err()
        );
    }
    assert!(
        snapshot
            .check_observation(
                snapshot.inputs(),
                &SnapshotOrigin::Worktree {
                    head: Some(commit("b")),
                    dirty: false
                }
            )
            .is_err()
    );
}

#[test]
fn version_compatibility_uses_the_callers_expected_analyzer_not_the_cache() {
    let (data, sources, _) = fixture(SnapshotOrigin::Commit {
        revision: commit("a"),
    });
    let snapshot = data.validate(&sources).unwrap();
    snapshot.ensure_compatible(&versions()).unwrap();
    let mut expected = versions();
    expected.analyzer_version = "test-analyzer-v2".into();
    assert!(snapshot.ensure_compatible(&expected).is_err());
    let mut expected = versions();
    expected.schema_version += 1;
    assert!(snapshot.ensure_compatible(&expected).is_err());
    let mut expected = versions();
    expected.hash_version += 1;
    assert!(snapshot.ensure_compatible(&expected).is_err());
}

#[test]
fn persisted_rows_must_match_captured_sources_and_graph_membership() {
    let (data, sources, _) = fixture(SnapshotOrigin::Commit {
        revision: commit("a"),
    });
    let mut wrong = data.clone();
    wrong.files.pop();
    assert!(wrong.validate(&sources).is_err());
    let mut wrong = data.clone();
    wrong.files.push(wrong.files[0].clone());
    assert!(wrong.validate(&sources).is_err());
    let mut wrong = data.clone();
    wrong.metadata.project = ProjectKey::new("other").unwrap();
    assert!(wrong.validate(&sources).is_err());
    let changed_sources = SourceSnapshot::new([
        SourceFile::new(path("src/gone.ts"), format!("{FUNCTION}\n")),
        SourceFile::new(path("tests/gone.test.ts"), TEST.into()),
    ])
    .unwrap();
    assert!(data.clone().validate(&changed_sources).is_err());
    let mut wrong = data.clone();
    let mut edges = wrong.graph.edges().to_vec();
    let e = edges
        .iter_mut()
        .find(|e| e.kind == EdgeKind::Imports)
        .unwrap();
    let mut a = e.provenance.span.start();
    let mut b = e.provenance.span.end();
    a.line = 2;
    b.line = 2;
    e.provenance.span = SourceSpan::new(a, b).unwrap();
    wrong.graph = GraphSnapshot::new(wrong.graph.nodes().to_vec(), edges, vec![]).unwrap();
    assert!(wrong.validate(&sources).is_err());
    let mut wrong = data;
    wrong.metadata.versions.hash_version = 2;
    assert!(wrong.validate(&sources).is_err());
}

#[test]
fn manifests_validate_hashes_and_reject_duplicate_or_retyped_paths() {
    let a = file_input("src/a.ts", "x", "a");
    let mut retyped = a.clone();
    retyped.role = InputRole::Configuration;
    assert!(InputManifest::new(vec![a.clone(), retyped]).is_err());
    let mut invalid = a;
    invalid.content_hash.digest = "not-a-digest".into();
    assert!(InputManifest::new(vec![invalid]).is_err());
    assert!(serde_json::from_value::<InputManifest>(serde_json::json!([{"path":"../escape","role":"source","byte_len":1,"content_hash":{"version":1,"digest":"a"}}])).is_err());
}

#[test]
fn git_endpoints_are_explicit_and_commit_ids_are_full_and_normalized() {
    assert_eq!(CommitId::new("A".repeat(40)).unwrap(), commit("a"));
    assert!(CommitId::new("f".repeat(64)).is_ok());
    for invalid in ["HEAD", "abc", "--help", "zzzz"] {
        assert!(CommitId::new(invalid).is_err());
    }
    let spec = ComparisonSpec::parse("feature/topic~1..HEAD").unwrap();
    assert_eq!(spec.base_ref, "feature/topic~1");
    assert_eq!(spec.head_ref, "HEAD");
    for invalid in [
        "main...HEAD",
        "main",
        "..HEAD",
        "main..",
        "a..b..c",
        "--help..HEAD",
        "main..HEAD\n",
    ] {
        assert!(ComparisonSpec::parse(invalid).is_err());
    }
}

#[test]
fn file_changes_keep_both_ranges_and_rename_hints_without_symbol_rename_claims() {
    let revisions = RevisionPair {
        base: commit("a"),
        head: commit("b"),
    };
    let inserted = DiffHunk {
        old: LineRange { start: 0, count: 0 },
        new: LineRange { start: 1, count: 2 },
    };
    let change = FileChange {
        old_path: None,
        new_path: Some(path("src/new.ts")),
        kind: FileChangeKind::Added,
        rename_similarity: None,
        hunks: vec![inserted],
    };
    let changes = RevisionChanges::new(revisions.clone(), vec![change.clone()]).unwrap();
    assert_eq!(changes.files()[0].hunks[0].old.count, 0);
    assert_eq!(
        changes,
        serde_json::from_str(&serde_json::to_string(&changes).unwrap()).unwrap()
    );
    let renamed = FileChange {
        old_path: Some(path("old.ts")),
        new_path: Some(path("new.ts")),
        kind: FileChangeKind::Renamed,
        rename_similarity: Some(60),
        hunks: vec![],
    };
    assert!(RevisionChanges::new(revisions.clone(), vec![renamed.clone()]).is_ok());
    assert!(RevisionChanges::new(revisions.clone(), vec![change.clone(), change]).is_err());
    let bad = FileChange {
        rename_similarity: Some(101),
        ..renamed
    };
    assert!(RevisionChanges::new(revisions, vec![bad]).is_err());
    assert!(LineRange { start: 0, count: 1 }.validate().is_err());
    assert!(
        LineRange {
            start: u32::MAX,
            count: 1
        }
        .validate()
        .is_err()
    );
}

#[test]
fn unknown_canonical_formats_cannot_be_used_for_equality() {
    let (data, sources, _) = fixture(SnapshotOrigin::Commit {
        revision: commit("a"),
    });
    let state = CanonicalState::from_snapshot(&data.validate(&sources).unwrap());
    let mut encoded = serde_json::to_value(state).unwrap();
    encoded["format_version"] = serde_json::json!(999);
    assert!(serde_json::from_value::<CanonicalState>(encoded).is_err());
}
