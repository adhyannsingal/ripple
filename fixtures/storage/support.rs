//! Independent core-record fixture, not a production analyzer-generated snapshot.
use ripple_core::{graph::*, identity::*, index::*, revision::*, source::*, syntax::*};
pub fn versions() -> IndexVersions {
    IndexVersions {
        schema_version: SCHEMA_VERSION,
        hash_version: HASH_VERSION,
        analyzer_version: "storage-fixture-v1".into(),
    }
}
pub fn fixture(
    generation: u64,
    value: &str,
    hash: impl Fn(&[u8]) -> VersionedHash,
) -> (IndexSnapshot, Vec<(RepoPath, Vec<u8>)>) {
    let recipe: serde_json::Value = serde_json::from_str(include_str!("basic.json")).unwrap();
    let library = RepoPath::new(recipe["library_path"].as_str().unwrap()).unwrap();
    let test_path = RepoPath::new(recipe["test_path"].as_str().unwrap()).unwrap();
    let source = SourceFile::new(
        library.clone(),
        recipe["library_source"]
            .as_str()
            .unwrap()
            .replace("$VALUE", value),
    );
    let test = SourceFile::new(
        test_path.clone(),
        recipe["test_source"].as_str().unwrap().into(),
    );
    let span = |source: &SourceFile, a, b| SourceSpan::from_offsets(source.text(), a, b).unwrap();
    let project = ProjectKey::single_project();
    let id = SymbolId::new(
        project.clone(),
        library.clone(),
        QualifiedName::new(["f".into()]).unwrap(),
        SymbolKind::Function,
    );
    let library_node = GraphNode::file(project.clone(), library.clone(), false);
    let test_node = GraphNode::file(project.clone(), test_path.clone(), true);
    let symbol = GraphNode::symbol(id.clone(), source.full_span());
    let known = test.text().find("f();").unwrap();
    let unknown = test.text().find("unknown()").unwrap();
    let location = |a, b| SourceLocation {
        file: test_path.clone(),
        span: span(&test, a, b),
    };
    let boundary = GraphNode {
        id: NodeId::Opaque {
            project: project.clone(),
            path: test_path.clone(),
            reference_key: "unknown:0".into(),
        },
        kind: NodeKind::OpaqueBoundary,
        span: Some(span(&test, unknown, unknown + 9)),
    };
    let graph = GraphSnapshot::new(
        vec![
            library_node.clone(),
            test_node.clone(),
            symbol.clone(),
            boundary.clone(),
        ],
        vec![
            Edge {
                source: library_node.id.clone(),
                target: symbol.id.clone(),
                kind: EdgeKind::Defines,
                confidence: Confidence::Resolved,
                provenance: SourceLocation {
                    file: library.clone(),
                    span: source.full_span(),
                },
                resolution_rule: "fixture_declaration".into(),
            },
            Edge {
                source: test_node.id.clone(),
                target: library_node.id.clone(),
                kind: EdgeKind::Imports,
                confidence: Confidence::Resolved,
                provenance: location(0, test.text().find('\n').unwrap()),
                resolution_rule: "fixture_import".into(),
            },
            Edge {
                source: test_node.id.clone(),
                target: symbol.id.clone(),
                kind: EdgeKind::Calls,
                confidence: serde_json::from_value(recipe["call_confidence"].clone()).unwrap(),
                provenance: location(known, known + 3),
                resolution_rule: "fixture_candidate".into(),
            },
        ],
        vec![UnresolvedReference {
            source: test_node.id.clone(),
            boundary: boundary.id,
            text: "unknown".into(),
            reference_kind: ReferenceKind::ComputedCall,
            provenance: location(unknown, unknown + 9),
            reason: recipe["unresolved_reason"].as_str().unwrap().into(),
        }],
    )
    .unwrap();
    let mut facts = FileFacts::empty(library.clone(), Language::TypeScript);
    facts.symbols.push(SymbolFact {
        identity: id,
        span: source.full_span(),
        signature_span: None,
        body_span: None,
        signature_hash: None,
        body_hash: None,
        exported: true,
    });
    let mut tests = FileFacts::empty(test_path.clone(), Language::TypeScript);
    for (text, start, end, kind) in [
        ("f", known, known + 3, ReferenceKind::Call),
        ("unknown", unknown, unknown + 9, ReferenceKind::ComputedCall),
    ] {
        tests.references.push(ReferenceFact {
            text: text.into(),
            kind,
            span: span(&test, start, end),
            owner: ReferenceOwner::File,
            scope: None,
        });
    }
    tests.diagnostics.push(Diagnostic {
        code: "fixture_uncertainty".into(),
        severity: Severity::Warning,
        message: "candidate call and dynamic boundary".into(),
        span: Some(span(&test, unknown, unknown + 9)),
    });
    let mut inputs = vec![
        (
            library,
            source.text().as_bytes().to_vec(),
            InputRole::Source,
        ),
        (
            test_path,
            test.text().as_bytes().to_vec(),
            InputRole::Source,
        ),
    ];
    for (path, bytes, role) in [
        (
            "tsconfig.json",
            recipe["config"].as_str().unwrap().as_bytes().to_vec(),
            InputRole::Configuration,
        ),
        (
            ".gitignore",
            recipe["ignore"].as_str().unwrap().as_bytes().to_vec(),
            InputRole::IgnorePolicy,
        ),
        ("package.json", b"{}".to_vec(), InputRole::PackageManifest),
        ("fixture.lock", vec![0, 255, 1], InputRole::Lockfile),
    ] {
        inputs.push((RepoPath::new(path).unwrap(), bytes, role));
    }
    let manifest = InputManifest::new(
        inputs
            .iter()
            .map(|(path, bytes, role)| CapturedInput {
                path: path.clone(),
                role: *role,
                byte_len: bytes.len() as u64,
                content_hash: hash(bytes),
            })
            .collect(),
    )
    .unwrap();
    let snapshot = SnapshotData {
        metadata: IndexMetadata {
            project,
            origin: SnapshotOrigin::Worktree {
                head: None,
                dirty: true,
            },
            versions: versions(),
            effective_config_hash: Some(hash(recipe["config"].as_str().unwrap().as_bytes())),
            generation,
            created_at: "2026-11-01T00:00:00Z".into(),
        },
        inputs: manifest,
        files: vec![facts, tests],
        graph,
    }
    .validate(&SourceSnapshot::new([source, test]).unwrap())
    .unwrap();
    (
        snapshot,
        inputs.into_iter().map(|(p, b, _)| (p, b)).collect(),
    )
}
