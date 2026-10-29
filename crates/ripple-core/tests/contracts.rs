use ripple_core::{
    identity::{ProjectKey, QualifiedName, RepoPath, SymbolId, SymbolKind},
    source::{SourceFile, SourcePosition, SourceSnapshot, SourceSpan},
    syntax::*,
};

fn path(value: &str) -> RepoPath {
    RepoPath::new(value).unwrap()
}
fn id(name: &[&str], kind: SymbolKind) -> SymbolId {
    SymbolId::new(
        ProjectKey::single_project(),
        path("src/checkout.ts"),
        QualifiedName::new(name.iter().map(|s| s.to_string())).unwrap(),
        kind,
    )
}
fn span(text: &str, start: usize, end: usize) -> SourceSpan {
    SourceSpan::from_offsets(text, start, end).unwrap()
}

fn shadowing_fixture() -> (SourceFile, FileFacts) {
    let text = "import { login } from './auth';\nfunction checkout(login) { login(); }";
    let source = SourceFile::new(path("src/checkout.ts"), text.into());
    let mut facts = FileFacts::empty(source.path().clone(), Language::TypeScript);
    let full = source.full_span();
    let function = span(text, text.find("function").unwrap(), text.len());
    let parameter = text.find("(login)").unwrap() + 1;
    let call = text.rfind("login()").unwrap();
    let imported = text.find("login").unwrap();
    facts.symbols.push(SymbolFact {
        identity: id(&["checkout"], SymbolKind::Function),
        span: function,
        signature_span: None,
        body_span: None,
        signature_hash: None,
        body_hash: None,
        exported: false,
    });
    facts.imports.push(ImportFact {
        specifier: "./auth".into(),
        span: span(text, 0, 30),
        bindings: vec![ImportBinding {
            imported: ImportedName::Named("login".into()),
            local: "login".into(),
            type_only: false,
            span: span(text, imported, imported + 5),
        }],
    });
    facts.scopes = vec![
        ScopeFact {
            id: ScopeId(0),
            parent: None,
            span: full,
            bindings: vec![BindingFact {
                name: "login".into(),
                kind: BindingKind::Import,
                span: span(text, imported, imported + 5),
            }],
        },
        ScopeFact {
            id: ScopeId(1),
            parent: Some(ScopeId(0)),
            span: function,
            bindings: vec![BindingFact {
                name: "login".into(),
                kind: BindingKind::Parameter,
                span: span(text, parameter, parameter + 5),
            }],
        },
    ];
    facts.references.push(ReferenceFact {
        text: "login".into(),
        kind: ReferenceKind::Call,
        span: span(text, call, call + 7),
        owner: ReferenceOwner::Symbol(id(&["checkout"], SymbolKind::Function)),
        scope: Some(ScopeId(1)),
    });
    (source, facts)
}

#[test]
fn paths_normalize_portably_and_reject_outside_roots() {
    assert_eq!(path(r".\src\auth.ts").as_str(), "src/auth.ts");
    assert_eq!(path("src//./auth.ts").as_str(), "src/auth.ts");
    for invalid in [
        "",
        ".",
        "..",
        "src/../auth.ts",
        "/tmp/a.ts",
        r"C:\src\a.ts",
        r"\\server\a.ts",
        "src/\na.ts",
    ] {
        assert!(RepoPath::new(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn deserialization_cannot_bypass_path_or_name_validation() {
    assert!(serde_json::from_str::<RepoPath>(r#""../escape.ts""#).is_err());
    assert!(serde_json::from_str::<QualifiedName>(r#"[""]"#).is_err());
    assert!(serde_json::from_str::<ProjectKey>(r#""""#).is_err());
}

#[test]
fn symbol_identity_is_unchanged_when_source_lines_shift() {
    let first = SourceFile::new(path("src/checkout.ts"), "function checkout() {}".into());
    let second = SourceFile::new(first.path().clone(), "\n\nfunction checkout() {}".into());
    let identity = id(&["checkout"], SymbolKind::Function);
    let a = SymbolFact {
        identity: identity.clone(),
        span: first.full_span(),
        signature_span: None,
        body_span: None,
        signature_hash: None,
        body_hash: None,
        exported: false,
    };
    let b = SymbolFact {
        identity,
        span: span(second.text(), 2, second.text().len()),
        ..a.clone()
    };
    assert_eq!(a.identity.semantic_key(), b.identity.semantic_key());
    assert_ne!(a.span, b.span);
}

#[test]
fn methods_and_quoted_names_cannot_collide_in_semantic_keys() {
    let auth = id(&["Auth", "login"], SymbolKind::Method);
    let user = id(&["User", "login"], SymbolKind::Method);
    let quoted = id(&["Auth.login"], SymbolKind::Method);
    assert_ne!(auth, user);
    assert_eq!(
        auth.qualified_name().display_name(),
        quoted.qualified_name().display_name()
    );
    assert_ne!(auth.semantic_key(), quoted.semantic_key());
    assert_ne!(
        auth.semantic_key(),
        auth.clone()
            .with_disambiguator("overload:abc".into())
            .unwrap()
            .semantic_key()
    );
    assert_eq!(
        auth,
        serde_json::from_str(&serde_json::to_string(&auth).unwrap()).unwrap()
    );
}

#[test]
fn spans_preserve_utf8_byte_crlf_and_eof_semantics() {
    let source = "α\r\nβ😀\n";
    assert_eq!(
        SourcePosition::at(source, 4).unwrap(),
        SourcePosition {
            byte: 4,
            line: 2,
            column: 1
        }
    );
    assert_eq!(
        SourcePosition::at(source, 10).unwrap(),
        SourcePosition {
            byte: 10,
            line: 2,
            column: 7
        }
    );
    assert_eq!(
        SourcePosition::at(source, 11).unwrap(),
        SourcePosition {
            byte: 11,
            line: 3,
            column: 1
        }
    );
    for invalid in [1, 5, 7, 12] {
        assert!(SourcePosition::at(source, invalid).is_err());
    }
    let beta = span(source, 4, 6);
    assert!(beta.contains_byte(4));
    assert!(!beta.contains_byte(6));
    assert!(!span(source, 11, 11).contains_byte(11));
    assert!(SourceSpan::from_offsets(source, 6, 4).is_err());
    span("", 0, 0).validate_against("").unwrap();
}

#[test]
fn serialized_spans_reject_invalid_coordinates_and_validate_against_source() {
    assert!(
        serde_json::from_str::<SourceSpan>(
            r#"{"start":{"byte":0,"line":0,"column":1},"end":{"byte":1,"line":1,"column":2}}"#
        )
        .is_err()
    );
    let wrong = SourceSpan::new(
        SourcePosition {
            byte: 0,
            line: 1,
            column: 2,
        },
        SourcePosition {
            byte: 1,
            line: 1,
            column: 3,
        },
    )
    .unwrap();
    assert!(wrong.validate_against("x").is_err());
}

#[test]
fn capture_is_ordered_immutable_and_rejects_duplicate_normalized_paths() {
    let a = SourceFile::new(path("a.ts"), "a".into());
    let b = SourceFile::new(path("b.ts"), "b".into());
    let snapshot = SourceSnapshot::new([b.clone(), a.clone()]).unwrap();
    assert_eq!(
        snapshot
            .files()
            .map(|f| f.path().as_str())
            .collect::<Vec<_>>(),
        ["a.ts", "b.ts"]
    );
    assert_eq!(snapshot.get(&path("a.ts")).unwrap().text(), "a");
    assert!(SourceSnapshot::new([a.clone(), a]).is_err());
}

#[test]
fn syntax_facts_retain_shadowing_without_inventing_resolution() {
    let (source, facts) = shadowing_fixture();
    facts.validate(&source).unwrap();
    assert_eq!(facts.scopes[0].bindings[0].kind, BindingKind::Import);
    assert_eq!(facts.scopes[1].bindings[0].kind, BindingKind::Parameter);
    assert_eq!(facts.references[0].scope, Some(ScopeId(1)));
    let round_trip: FileFacts =
        serde_json::from_str(&serde_json::to_string(&facts).unwrap()).unwrap();
    assert_eq!(facts, round_trip);
    round_trip.validate(&source).unwrap();
}

#[test]
fn missing_owners_duplicate_symbols_and_cross_file_ids_are_rejected() {
    let (source, mut facts) = shadowing_fixture();
    facts.references[0].owner = ReferenceOwner::Symbol(id(&["unknown"], SymbolKind::Function));
    assert!(facts.validate(&source).is_err());
    let (_, mut facts) = shadowing_fixture();
    facts.symbols.push(facts.symbols[0].clone());
    assert!(facts.validate(&source).is_err());
    let (_, mut facts) = shadowing_fixture();
    facts.path = path("elsewhere.ts");
    assert!(facts.validate(&source).is_err());
}

#[test]
fn lexical_cycles_missing_scopes_and_out_of_owner_references_are_rejected() {
    let (source, mut facts) = shadowing_fixture();
    facts.scopes[1].span = facts.scopes[0].span;
    facts.scopes[0].parent = Some(ScopeId(1));
    assert!(
        facts
            .validate(&source)
            .unwrap_err()
            .to_string()
            .contains("cyclic")
    );
    let (_, mut facts) = shadowing_fixture();
    facts.references[0].scope = Some(ScopeId(99));
    assert!(facts.validate(&source).is_err());
    let (_, mut facts) = shadowing_fixture();
    facts.references[0].span = span(source.text(), 0, 5);
    assert!(facts.validate(&source).is_err());
}

#[test]
fn unsupported_and_failed_syntax_is_explicit_not_silent() {
    let source = SourceFile::new(path("src/checkout.ts"), "@???".into());
    let mut facts = FileFacts::empty(source.path().clone(), Language::TypeScript);
    facts.parse_status = ParseStatus::Failed;
    assert!(facts.validate(&source).is_err());
    facts.diagnostics.push(Diagnostic {
        code: "parse_error".into(),
        severity: Severity::Error,
        message: "Cannot extract supported facts from malformed source".into(),
        span: Some(source.full_span()),
    });
    facts.validate(&source).unwrap();
    let (other_source, mut other) = shadowing_fixture();
    other.parse_status = ParseStatus::Failed;
    other.diagnostics.push(facts.diagnostics[0].clone());
    assert!(other.validate(&other_source).is_err());
}

#[test]
fn invalid_hashes_and_recovered_parse_without_diagnostics_are_rejected() {
    let (source, mut facts) = shadowing_fixture();
    for (version, digest) in [
        (0, "a".repeat(64)),
        (1, "A".repeat(64)),
        (1, "a".repeat(63)),
    ] {
        facts.symbols[0].body_hash = Some(VersionedHash { version, digest });
        assert!(facts.validate(&source).is_err());
    }
    facts.symbols[0].body_hash = Some(VersionedHash {
        version: 1,
        digest: "a".repeat(64),
    });
    facts.validate(&source).unwrap();
    facts.parse_status = ParseStatus::Recovered;
    assert!(facts.validate(&source).is_err());
}
