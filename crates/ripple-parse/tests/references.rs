use ripple_core::{
    identity::{ProjectKey, RepoPath},
    syntax::{
        BindingFact, BindingKind, FileFacts, ParseStatus, ReferenceFact, ReferenceKind,
        ReferenceOwner, ScopeId,
    },
};
use ripple_parse::{
    extract,
    parser::{ParseLimits, parse, parse_with_limits},
    symbols::owner_for_span,
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path, time::Duration};
fn facts(file: &str, source: &str) -> FileFacts {
    let parsed = parse(RepoPath::new(file).unwrap(), source.as_bytes()).unwrap();
    let facts = extract(&parsed, ProjectKey::single_project()).unwrap();
    facts.validate(parsed.source()).unwrap();
    facts
}
fn sorted(mut values: Vec<Value>) -> Vec<Value> {
    values.sort_by_key(Value::to_string);
    values
}
fn nearest<'a>(
    facts: &'a FileFacts,
    mut scope: Option<ScopeId>,
    name: &str,
) -> Option<&'a BindingFact> {
    while let Some(id) = scope {
        let current = facts.scopes.iter().find(|s| s.id == id).unwrap();
        if let Some(binding) = current.bindings.iter().find(|b| b.name == name) {
            return Some(binding);
        }
        scope = current.parent;
    }
    None
}
fn calls<'a>(facts: &'a FileFacts, name: &str) -> Vec<&'a ReferenceFact> {
    let mut refs = facts
        .references
        .iter()
        .filter(|r| r.kind == ReferenceKind::Call && r.text == name)
        .collect::<Vec<_>>();
    refs.sort_by_key(|r| r.span.start().byte);
    refs
}
#[test]
fn independent_goldens_cover_imports_exports_ownership_and_dynamic_boundaries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/references");
    let expected: Value =
        serde_json::from_str(&fs::read_to_string(root.join("expected.json")).unwrap()).unwrap();
    for case in expected["cases"].as_array().unwrap() {
        let file = case["file"].as_str().unwrap();
        let text = fs::read_to_string(root.join(file)).unwrap();
        let actual = facts(file, &text);
        assert_eq!(
            actual.parse_status,
            ParseStatus::Clean,
            "{file}: {:?}",
            actual.diagnostics
        );
        let imports=actual.imports.iter().map(|i|json!({"specifier":i.specifier,"bindings":sorted(i.bindings.iter().map(|b|json!({"imported":b.imported,"local":b.local,"type_only":b.type_only})).collect())})).collect();
        let expected_imports = case["imports"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                let mut v = v.clone();
                v["bindings"] = json!(sorted(v["bindings"].as_array().unwrap().clone()));
                v
            })
            .collect();
        assert_eq!(sorted(imports), sorted(expected_imports), "{file} imports");
        assert_eq!(
            sorted(
                actual
                    .exports
                    .iter()
                    .map(|e| serde_json::to_value(&e.kind).unwrap())
                    .collect()
            ),
            sorted(case["exports"].as_array().unwrap().clone()),
            "{file} exports"
        );
        let references=actual.references.iter().map(|r|json!({"text":r.text,"kind":r.kind,"owner":match &r.owner{ReferenceOwner::File=>"FILE".into(),ReferenceOwner::Symbol(id)=>id.qualified_name().display_name()}})).collect();
        assert_eq!(
            sorted(references),
            sorted(case["references"].as_array().unwrap().clone()),
            "{file} references"
        );
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
            "{file} diagnostics"
        );
        for reference in &actual.references {
            assert_eq!(
                &text[reference.span.start().byte..reference.span.end().byte],
                reference.text
            );
            assert_eq!(reference.owner, owner_for_span(&actual, reference.span));
            assert!(reference.scope.is_some());
        }
        assert_eq!(actual, facts(file, &text), "deterministic {file}");
        assert_eq!(
            actual,
            serde_json::from_value::<FileFacts>(serde_json::to_value(&actual).unwrap()).unwrap()
        );
    }
}
#[test]
fn nearest_bindings_separate_parameter_callback_block_and_module_scopes() {
    let text = include_str!("../../../fixtures/references/shadowing.ts");
    let actual = facts("shadowing.ts", text);
    let calls = calls(&actual, "login");
    assert_eq!(calls.len(), 5);
    assert_eq!(
        calls
            .iter()
            .map(|r| nearest(&actual, r.scope, "login").unwrap().kind)
            .collect::<Vec<_>>(),
        vec![
            BindingKind::Parameter,
            BindingKind::Parameter,
            BindingKind::Local,
            BindingKind::Parameter,
            BindingKind::Import
        ]
    );
    assert_ne!(
        nearest(&actual, calls[0].scope, "login").unwrap().span,
        nearest(&actual, calls[1].scope, "login").unwrap().span
    );
}
#[test]
fn body_var_hoisting_does_not_shadow_parameter_defaults() {
    let actual = facts(
        "defaults.ts",
        "import { login } from './auth'; function outer(value=login()){ { var login=()=>{}; } login(); } login();",
    );
    let refs = calls(&actual, "login");
    assert_eq!(refs.len(), 3);
    assert_eq!(
        refs.iter()
            .map(|r| nearest(&actual, r.scope, "login").unwrap().kind)
            .collect::<Vec<_>>(),
        vec![BindingKind::Import, BindingKind::Local, BindingKind::Import]
    );
}
#[test]
fn catch_loop_and_destructuring_bindings_do_not_leak() {
    let actual = facts(
        "patterns.ts",
        "import { login } from './auth'; try {} catch ({login}) {login();} for (const login of values) {login();} login(); const { key: alias = seed, ...rest } = source;",
    );
    let refs = calls(&actual, "login");
    assert_eq!(refs.len(), 3);
    assert_eq!(
        refs.iter()
            .map(|r| nearest(&actual, r.scope, "login").unwrap().kind)
            .collect::<Vec<_>>(),
        vec![
            BindingKind::Parameter,
            BindingKind::Local,
            BindingKind::Import
        ]
    );
    let names = actual
        .scopes
        .iter()
        .flat_map(|s| s.bindings.iter().map(|b| b.name.as_str()))
        .collect::<BTreeSet<_>>();
    assert!(names.contains("alias") && names.contains("rest"));
    assert!(!names.contains("key") && !names.contains("seed"));
    assert!(
        actual
            .references
            .iter()
            .any(|r| r.text == "seed" && r.kind == ReferenceKind::Read)
    );
}
#[test]
fn named_expression_and_block_declarations_shadow_without_new_nested_symbols() {
    let actual = facts(
        "nested.ts",
        "import { login } from './auth'; const execute=function login(){ login(); }; function outer(){ login(); { login(); let login=()=>{}; } function inner(login:any){login();} } login();",
    );
    let refs = calls(&actual, "login");
    assert_eq!(refs.len(), 5);
    assert_eq!(
        refs.iter()
            .map(|r| nearest(&actual, r.scope, "login").unwrap().kind)
            .collect::<Vec<_>>(),
        vec![
            BindingKind::Local,
            BindingKind::Import,
            BindingKind::Local,
            BindingKind::Parameter,
            BindingKind::Import
        ]
    );
    assert_eq!(actual.symbols.len(), 2);
    assert!(
        matches!(&refs[3].owner,ReferenceOwner::Symbol(id) if id.qualified_name().display_name()=="outer")
    );
}
#[test]
fn types_are_not_runtime_reads_and_type_only_bindings_remain_marked() {
    let actual = facts(
        "types.ts",
        "import type { Type } from './types'; import { type Shape, run } from './api'; type Alias=typeof run; interface I {x: Type}; function f(value: Type): Shape {const x=value as Type; return run<Shape>(x satisfies Shape);}",
    );
    assert!(
        actual
            .references
            .iter()
            .all(|r| !matches!(r.text.as_str(), "Type" | "Shape" | "Alias" | "I"))
    );
    assert_eq!(
        actual
            .imports
            .iter()
            .flat_map(|i| i.bindings.iter())
            .filter(|b| b.type_only)
            .count(),
        2
    );
    assert_eq!(calls(&actual, "run").len(), 1);
    assert!(
        actual
            .references
            .iter()
            .filter(|r| r.text == "run")
            .all(|r| r.kind == ReferenceKind::Call)
    );
}
#[test]
fn optional_wrapped_constructor_and_receiver_calls_never_become_direct_calls() {
    let actual = facts(
        "calls.ts",
        "function f(fn:any, obj:any){ fn?.(); (fn as any)(); new fn(); obj.run(); obj[key](); (fn)(); }",
    );
    assert_eq!(calls(&actual, "fn").len(), 1);
    assert_eq!(
        actual
            .references
            .iter()
            .filter(|r| r.kind == ReferenceKind::ComputedCall)
            .count(),
        5
    );
    assert!(
        actual
            .diagnostics
            .iter()
            .any(|d| d.code == "unsupported_call_target")
    );
}
#[test]
fn method_ownership_and_module_callbacks_match_supported_symbol_intervals() {
    let actual = facts(
        "owners.ts",
        "function f(x=load()){ return ()=>load(); } class C { method(){ return ()=>load(); } } describe('x',()=>load());",
    );
    let refs = calls(&actual, "load");
    assert_eq!(refs.len(), 4);
    let owners = refs
        .iter()
        .map(|r| match &r.owner {
            ReferenceOwner::File => "FILE".into(),
            ReferenceOwner::Symbol(id) => id.qualified_name().display_name(),
        })
        .collect::<Vec<_>>();
    assert_eq!(owners, vec!["f", "f", "C.method", "FILE"]);
    for r in &actual.references {
        assert_eq!(r.owner, owner_for_span(&actual, r.span));
    }
}
#[test]
fn default_exports_and_type_reexports_have_explicit_local_names() {
    for text in [
        "export default (()=>1);",
        "export default function(){}",
        "export default class {}",
    ] {
        let actual = facts("default.ts", text);
        assert_eq!(actual.exports.len(), 1);
        assert_eq!(
            serde_json::to_value(&actual.exports[0].kind).unwrap(),
            json!({"kind":"LOCAL","local":"default","exported":{"kind":"DEFAULT"},"type_only":false})
        );
    }
    let actual = facts(
        "exports.ts",
        "export const {value: renamed, short} = data; export interface Shape {} export type Alias=string;",
    );
    let locals = actual
        .exports
        .iter()
        .map(|e| {
            serde_json::to_value(&e.kind).unwrap()["local"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        locals,
        BTreeSet::from([
            "renamed".into(),
            "short".into(),
            "Shape".into(),
            "Alias".into()
        ])
    );
}
#[test]
fn unsupported_imports_and_namespace_exports_are_diagnosed_without_invented_targets() {
    let actual = facts(
        "unsupported.ts",
        r#"import { login } from './a\u002fb'; import service = require('./auth'); export * as ns from './auth'; export default factory(); login(); service();"#,
    );
    assert!(actual.imports.is_empty());
    assert!(actual.exports.is_empty());
    let codes = actual
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect::<BTreeSet<_>>();
    for code in [
        "unsupported_module_literal",
        "unsupported_import_equals",
        "unsupported_namespace_export",
        "unsupported_export_expression",
    ] {
        assert!(codes.contains(code), "{code}: {codes:?}");
    }
    for name in ["login", "service"] {
        let refs = calls(&actual, name);
        assert_eq!(refs.len(), 1);
        assert_eq!(
            nearest(&actual, refs[0].scope, name).unwrap().kind,
            BindingKind::Import
        );
    }
}
#[test]
fn unicode_crlf_spans_and_line_shifts_preserve_identity_and_reference_owners() {
    let text =
        "import { π } from './math';\r\nexport function café(値: number){ return π(値); }\r\n";
    let a = facts("unicode.ts", text);
    let b = facts("unicode.ts", &format!("\r\n{text}"));
    assert_eq!(a.symbols[0].identity, b.symbols[0].identity);
    assert_eq!(
        a.references
            .iter()
            .map(|r| (&r.text, &r.owner))
            .collect::<Vec<_>>(),
        b.references
            .iter()
            .map(|r| (&r.text, &r.owner))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        a.references[0].span.start().line + 1,
        b.references[0].span.start().line
    );
}
#[test]
fn failed_and_recovered_parses_never_claim_complete_facts() {
    let parsed = parse_with_limits(
        RepoPath::new("cancel.ts").unwrap(),
        b"import x from './x'; x();",
        ParseLimits {
            timeout: Duration::ZERO,
            ..Default::default()
        },
    )
    .unwrap();
    let actual = extract(&parsed, ProjectKey::single_project()).unwrap();
    assert_eq!(actual.parse_status, ParseStatus::Failed);
    assert!(actual.imports.is_empty() && actual.references.is_empty() && actual.scopes.is_empty());
    let actual = facts("bad.ts", "import { from ; export function f( { run(;");
    assert_eq!(actual.parse_status, ParseStatus::Recovered);
    assert!(!actual.diagnostics.is_empty());
}
#[test]
fn resource_limits_withhold_supplemental_facts_and_report_truncation() {
    let text = format!("{}run();{}", "{".repeat(260), "}".repeat(260));
    let actual = facts("deep.ts", &text);
    assert!(actual.scopes.is_empty() && actual.references.is_empty() && actual.imports.is_empty());
    assert!(
        actual
            .diagnostics
            .iter()
            .any(|d| d.code == "reference_resource_limit")
    );
    let text = "obj.run();".repeat(1100);
    let actual = facts("many.ts", &text);
    assert!(actual.diagnostics.len() <= 1026);
    assert!(
        actual
            .diagnostics
            .iter()
            .any(|d| d.code == "reference_diagnostics_truncated")
    );
    assert_eq!(
        actual
            .references
            .iter()
            .filter(|r| r.kind == ReferenceKind::ComputedCall)
            .count(),
        1100
    );
}
#[test]
fn static_block_vars_stay_inside_their_runtime_environment() {
    let actual = facts(
        "static.ts",
        "import { login } from './auth'; class C {static {var login=()=>{}; login();}} login();",
    );
    let refs = calls(&actual, "login");
    assert_eq!(refs.len(), 2);
    assert_eq!(
        nearest(&actual, refs[0].scope, "login").unwrap().kind,
        BindingKind::Local
    );
    assert_eq!(
        nearest(&actual, refs[1].scope, "login").unwrap().kind,
        BindingKind::Import
    );
}
#[test]
fn type_star_reexport_is_visible_as_recovered_in_the_pinned_grammar() {
    let actual = facts("type-star.ts", "export type * from './types';");
    assert_eq!(actual.parse_status, ParseStatus::Recovered);
    assert!(actual.exports.is_empty());
    assert!(
        actual
            .diagnostics
            .iter()
            .any(|d| d.code == "incomplete_export")
    );
}
#[test]
fn ordinary_functions_bind_arguments_and_arrows_inherit_it() {
    let actual = facts(
        "arguments.ts",
        "function outer(){ use(arguments); const callback=()=>use(arguments); } const callback=()=>use(arguments);",
    );
    let mut refs = actual
        .references
        .iter()
        .filter(|r| r.text == "arguments")
        .collect::<Vec<_>>();
    refs.sort_by_key(|r| r.span.start().byte);
    assert_eq!(refs.len(), 3);
    assert_eq!(
        nearest(&actual, refs[0].scope, "arguments").unwrap().kind,
        BindingKind::Local
    );
    assert_eq!(
        nearest(&actual, refs[0].scope, "arguments"),
        nearest(&actual, refs[1].scope, "arguments")
    );
    assert!(nearest(&actual, refs[2].scope, "arguments").is_none());
}
#[test]
fn generated_source_intervals_match_independent_owner_lookup() {
    let mut text = String::new();
    for n in 0..100 {
        text.push_str(&format!(
            "function f{n}(x=load()){{ return ()=>load(); }} class C{n}{{ m(){{load();}} }}\n"
        ));
    }
    text.push_str("load();");
    let actual = facts("generated.ts", &text);
    assert_eq!(calls(&actual, "load").len(), 301);
    for reference in &actual.references {
        assert_eq!(reference.owner, owner_for_span(&actual, reference.span));
    }
}
