use ripple_core::{
    identity::{ProjectKey, RepoPath, SymbolKind},
    source::SourceSpan,
    syntax::{FileFacts, ParseStatus, ReferenceOwner, SymbolFact},
};
use ripple_parse::{
    parser::{ParseLimits, parse, parse_with_limits},
    symbols::{extract, owner_for_span},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/extraction")
}
fn facts(file: &str, text: &str) -> FileFacts {
    let parsed = parse(RepoPath::new(file).unwrap(), text.as_bytes()).unwrap();
    let facts = extract(&parsed, ProjectKey::single_project()).unwrap();
    facts.validate(parsed.source()).unwrap();
    facts
}
fn named<'a>(facts: &'a FileFacts, name: &str) -> &'a SymbolFact {
    facts
        .symbols
        .iter()
        .find(|s| s.identity.qualified_name().display_name() == name)
        .unwrap()
}
fn summary(facts: &FileFacts) -> Vec<Value> {
    let mut result=facts.symbols.iter().map(|s| json!({"name":s.identity.qualified_name().components(),"kind":s.identity.kind().as_str(),"exported":s.exported,"role":serde_json::to_value(&s.identity).unwrap()["disambiguator"]})).collect::<Vec<_>>();
    result.sort_by_key(|v| v.to_string());
    result
}
#[test]
fn independent_symbol_goldens_cover_supported_declarations_and_diagnostics() {
    let expected: Value =
        serde_json::from_str(&fs::read_to_string(fixtures().join("expected.json")).unwrap())
            .unwrap();
    for case in expected["cases"].as_array().unwrap() {
        let file = case["file"].as_str().unwrap();
        let text = fs::read_to_string(fixtures().join(file)).unwrap();
        let actual = facts(file, &text);
        assert_eq!(
            actual.parse_status,
            ParseStatus::Clean,
            "{file}: {:?}",
            actual.diagnostics
        );
        let mut symbols = case["symbols"].as_array().unwrap().clone();
        symbols.sort_by_key(|v| v.to_string());
        assert_eq!(summary(&actual), symbols, "{file}");
        let codes = actual
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            codes,
            case["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect(),
            "{file}"
        );
        for symbol in &actual.symbols {
            symbol.signature_hash.as_ref().unwrap().validate().unwrap();
            symbol.body_hash.as_ref().unwrap().validate().unwrap();
            assert!(symbol.span.encloses(symbol.signature_span.unwrap()));
            assert!(symbol.span.encloses(symbol.body_span.unwrap()));
        }
        assert!(
            actual.imports.is_empty()
                && actual.exports.is_empty()
                && actual.references.is_empty()
                && actual.scopes.is_empty()
        );
        assert_eq!(
            actual,
            serde_json::from_value::<FileFacts>(serde_json::to_value(&actual).unwrap()).unwrap()
        );
        assert_eq!(actual, facts(file, &text));
    }
}
#[test]
fn independent_hash_change_matrix_preserves_structure_and_meaningful_text() {
    let expected: Value =
        serde_json::from_str(&fs::read_to_string(fixtures().join("hash-pairs.json")).unwrap())
            .unwrap();
    for pair in expected["pairs"].as_array().unwrap() {
        let file = pair["file"].as_str().unwrap();
        let before = facts(file, pair["before"].as_str().unwrap());
        let after = facts(file, pair["after"].as_str().unwrap());
        assert_eq!(before.parse_status, ParseStatus::Clean);
        assert_eq!(after.parse_status, ParseStatus::Clean);
        let (a, b) = (named(&before, "f"), named(&after, "f"));
        assert_eq!(a.identity, b.identity);
        assert_eq!(
            a.signature_hash != b.signature_hash,
            pair["signature_changed"].as_bool().unwrap(),
            "{} signature",
            pair["name"]
        );
        assert_eq!(
            a.body_hash != b.body_hash,
            pair["body_changed"].as_bool().unwrap(),
            "{} body",
            pair["name"]
        );
    }
}
#[test]
fn method_body_changes_do_not_change_class_shape_but_member_changes_do() {
    let a = facts(
        "class.ts",
        "export class C { method(x: number) {return 1;} }",
    );
    let b = facts(
        "class.ts",
        "export class C { method(x: number) {return 2;} }",
    );
    assert_eq!(named(&a, "C").signature_hash, named(&b, "C").signature_hash);
    assert_eq!(named(&a, "C").body_hash, named(&b, "C").body_hash);
    assert_eq!(
        named(&a, "C.method").signature_hash,
        named(&b, "C.method").signature_hash
    );
    assert_ne!(
        named(&a, "C.method").body_hash,
        named(&b, "C.method").body_hash
    );
    for text in [
        "export class C { method(x: string) {return 1;} }",
        "export class C { method(x: number) {return 1;} added() {} }",
        "export class C extends Base { method(x: number) {return 1;} }",
    ] {
        let changed = facts("class.ts", text);
        assert_ne!(
            named(&a, "C").signature_hash,
            named(&changed, "C").signature_hash
        );
    }
}
#[test]
fn overloads_share_a_stable_identity_and_hash_all_headers() {
    let text = fs::read_to_string(fixtures().join("overloads.ts")).unwrap();
    let original = facts("overloads.ts", &text);
    assert_eq!(
        original
            .symbols
            .iter()
            .filter(|s| s.identity.qualified_name().display_name() == "convert")
            .count(),
        1
    );
    let changed = facts(
        "overloads.ts",
        &text.replace(
            "convert(value: string): string",
            "convert(value: boolean): boolean",
        ),
    );
    assert_eq!(
        named(&original, "convert").identity,
        named(&changed, "convert").identity
    );
    assert_ne!(
        named(&original, "convert").signature_hash,
        named(&changed, "convert").signature_hash
    );
    assert_eq!(
        named(&original, "convert").body_hash,
        named(&changed, "convert").body_hash
    );
    assert!(
        named(&original, "convert").span.start().byte
            < named(&original, "convert").body_span.unwrap().start().byte
    );
    assert_eq!(
        named(&original, "Box.compute").identity.kind(),
        SymbolKind::Method
    );
}
#[test]
fn line_shifts_unicode_and_closest_supported_ownership_remain_stable() {
    let text = "export class α { run() { const callback=()=>inner(); return callback(); } }\r\nregister(()=>external());";
    let before = facts("nested.ts", text);
    let after = facts("nested.ts", &format!("// shifted\r\n\r\n{text}"));
    for (a, b) in before.symbols.iter().zip(&after.symbols) {
        assert_eq!(a.identity, b.identity);
        assert_eq!(a.signature_hash, b.signature_hash);
        assert_eq!(a.body_hash, b.body_hash);
        assert_ne!(a.span, b.span);
    }
    let call = text.find("inner()").unwrap();
    let owner = owner_for_span(
        &before,
        SourceSpan::from_offsets(text, call, call + 7).unwrap(),
    );
    assert_eq!(
        owner,
        ReferenceOwner::Symbol(named(&before, "α.run").identity.clone())
    );
    let call = text.find("external()").unwrap();
    assert_eq!(
        owner_for_span(
            &before,
            SourceSpan::from_offsets(text, call, call + 10).unwrap()
        ),
        ReferenceOwner::File
    );
}
#[test]
fn export_shape_changes_only_signature_hash_and_reexports_do_not_mark_locals() {
    let a = facts(
        "exports.ts",
        "function f(){return 1;} export { f as first };",
    );
    let b = facts(
        "exports.ts",
        "function f(){return 1;} export { f as second };",
    );
    assert!(named(&a, "f").exported);
    assert_ne!(named(&a, "f").signature_hash, named(&b, "f").signature_hash);
    assert_eq!(named(&a, "f").body_hash, named(&b, "f").body_hash);
    let external = facts("exports.ts", "function f(){} export { f } from './other';");
    assert!(!named(&external, "f").exported);
    let default = facts("exports.ts", "function f(){} export default f;");
    assert!(named(&default, "f").exported);
    let type_only = facts("exports.ts", "class C {} export type { C };");
    assert!(!named(&type_only, "C").exported);
}
#[test]
fn unsupported_bindings_ambient_groups_and_parse_failure_never_invent_targets() {
    for text in [
        "let f=()=>1;",
        "var f=function(){};",
        "function f(x:number):number;",
        "function f(x:number):number; const unrelated=1; function f(x:number){return x;}",
    ] {
        let actual = facts("unsupported.ts", text);
        assert!(actual.symbols.is_empty(), "{text}");
        assert!(!actual.diagnostics.is_empty());
    }
    let parsed = parse_with_limits(
        RepoPath::new("failed.ts").unwrap(),
        b"function f(){}",
        ParseLimits {
            timeout: Duration::ZERO,
            ..Default::default()
        },
    )
    .unwrap();
    let failed = extract(&parsed, ProjectKey::single_project()).unwrap();
    assert_eq!(failed.parse_status, ParseStatus::Failed);
    assert!(failed.symbols.is_empty());
    assert!(!failed.diagnostics.is_empty());
    let recovered = facts("recovered.ts", "function good(){}\nconst = ;");
    assert_eq!(recovered.parse_status, ParseStatus::Recovered);
    assert_eq!(recovered.symbols.len(), 1);
    assert_eq!(
        named(&recovered, "good").identity.kind(),
        SymbolKind::Function
    );
}

#[test]
fn unsupported_export_spelling_and_directives_withhold_complete_signatures() {
    for text in [
        r#"function f(){} export { f as "\u0061" };"#,
        "/** @jsx customFactory */\nfunction f(){}",
        r"function f(){} export function \u0067(){}",
        "function f(){} export default factory();",
    ] {
        let actual = facts("unsupported.ts", text);
        let symbol = named(&actual, "f");
        assert!(symbol.signature_hash.is_none());
        assert!(symbol.body_hash.is_some());
        assert!(!actual.diagnostics.is_empty());
    }
    let recovered = facts("recovered.ts", "function good(){}\nconst = ;");
    assert!(named(&recovered, "good").signature_hash.is_none());
    assert!(named(&recovered, "good").body_hash.is_some());
}

#[test]
fn unsupported_member_bodies_remain_observable_in_the_class_fingerprint() {
    for (before, after) in [
        (
            "class C { [key](){return 1;} }",
            "class C { [key](){return 2;} }",
        ),
        (
            "class C { same(){return 1;} same(){return 2;} }",
            "class C { same(){return 3;} same(){return 2;} }",
        ),
        ("class C { field=()=>1; }", "class C { field=()=>2; }"),
        (
            "class C { static { init(1); } }",
            "class C { static { init(2); } }",
        ),
    ] {
        let a = facts("members.ts", before);
        let b = facts("members.ts", after);
        assert_eq!(a.symbols.len(), 1);
        assert_eq!(b.symbols.len(), 1);
        assert!(!a.diagnostics.is_empty());
        assert_ne!(
            named(&a, "C").body_hash,
            named(&b, "C").body_hash,
            "{before}"
        );
        assert_ne!(
            named(&a, "C").signature_hash,
            named(&b, "C").signature_hash,
            "{before}"
        );
    }
}

#[test]
fn parenthesized_defaults_keep_identity_and_export_evidence() {
    for (before, after) in [
        (
            "export default (()=>1);",
            "export default (/* note */ ()=>1);",
        ),
        (
            "export default (class { run(){} });",
            "export default (/* note */ class { run(){} });",
        ),
        (
            "function f(){} export default (f);",
            "function f(){} export default (/* note */ f);",
        ),
    ] {
        let a = facts("default.ts", before);
        let b = facts("default.ts", after);
        assert!(!a.symbols.is_empty());
        assert!(a.diagnostics.is_empty() && b.diagnostics.is_empty());
        assert_eq!(summary(&a), summary(&b));
        for (a, b) in a.symbols.iter().zip(&b.symbols) {
            assert_eq!(a.identity, b.identity);
            assert_eq!(a.signature_hash, b.signature_hash);
            assert_eq!(a.body_hash, b.body_hash);
        }
    }
}

#[test]
fn unsupported_names_are_visible_and_hashes_do_not_depend_on_repository_path() {
    let actual = facts("names.ts", r#"class C { 1(){} "\u006d"(){} [key](){} }"#);
    assert_eq!(actual.symbols.len(), 1);
    assert!(
        actual
            .diagnostics
            .iter()
            .any(|d| d.code == "unsupported_symbol_name")
    );
    let escaped = facts("names.ts", r#"function \u0066() {}"#);
    assert!(escaped.symbols.is_empty());
    assert!(
        escaped
            .diagnostics
            .iter()
            .any(|d| d.code == "unsupported_symbol_name")
    );
    let a = facts("src/a.ts", "function f(){return 7;}");
    let b = facts("other/b.ts", "function f(){return 7;}");
    assert_ne!(named(&a, "f").identity, named(&b, "f").identity);
    assert_eq!(named(&a, "f").signature_hash, named(&b, "f").signature_hash);
    assert_eq!(named(&a, "f").body_hash, named(&b, "f").body_hash);
}

#[test]
fn commonjs_and_namespace_exports_do_not_receive_supported_es_export_fingerprints() {
    for text in [
        "function f(){} export = f;",
        "function f(){} export as namespace Library;",
    ] {
        let actual = facts("exports.ts", text);
        assert_eq!(actual.parse_status, ParseStatus::Clean);
        assert!(!named(&actual, "f").exported);
        assert!(named(&actual, "f").signature_hash.is_none());
        assert!(
            actual
                .diagnostics
                .iter()
                .any(|d| d.code == "unsupported_export_form")
        );
    }
}
