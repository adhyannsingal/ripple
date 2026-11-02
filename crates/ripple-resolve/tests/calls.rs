use ripple_core::{
    graph::{Confidence, EdgeKind},
    identity::{ProjectKey, RepoPath},
    source::{SourceFile, SourceSnapshot},
    syntax::FileFacts,
};
use ripple_resolve::{
    config::{CapturedConfigs, ConfigInput},
    symbols::{GraphAnalysis, resolve_graph},
};
use std::collections::BTreeSet;
fn p(s: &str) -> RepoPath {
    RepoPath::new(s).unwrap()
}
fn graph(inputs: &[(&str, &str)]) -> GraphAnalysis {
    let mut sources = Vec::new();
    let mut facts = Vec::<FileFacts>::new();
    for (path, text) in inputs {
        sources.push(SourceFile::new(p(path), (*text).into()));
        let parsed = ripple_parse::parser::parse(p(path), text.as_bytes()).unwrap();
        facts.push(ripple_parse::extract(&parsed, ProjectKey::single_project()).unwrap());
    }
    let config = CapturedConfigs::new([ConfigInput {
        path: p("tsconfig.json"),
        bytes: br#"{"compilerOptions":{"paths":{"@/*":["./*"]}}}"#.to_vec(),
    }])
    .unwrap()
    .evaluate(&p("tsconfig.json"))
    .unwrap();
    resolve_graph(
        &ProjectKey::single_project(),
        &SourceSnapshot::new(sources).unwrap(),
        &facts,
        &config,
        &inputs
            .iter()
            .filter(|(p, _)| p.ends_with(".test.ts"))
            .map(|(s, _)| p(s))
            .collect::<BTreeSet<_>>(),
    )
    .unwrap()
}
#[test]
fn imported_barrels_local_calls_and_exact_file_owned_test_calls_have_provenance() {
    let a = graph(&[
        ("lib.ts", "export function f() {}"),
        ("barrel.ts", "export { f as renamed } from './lib';"),
        (
            "caller.ts",
            "import { renamed as run } from '@/barrel'; export function caller() { run(); }",
        ),
        (
            "caller.test.ts",
            "import { caller } from './caller'; caller();",
        ),
    ]);
    assert_eq!(
        a.graph
            .edges()
            .iter()
            .filter(|e| e.kind == EdgeKind::Calls)
            .count(),
        2
    );
    assert!(
        a.graph
            .edges()
            .iter()
            .all(|e| e.confidence == Confidence::Resolved)
    );
    assert!(
        a.graph
            .edges()
            .iter()
            .any(|e| e.kind == EdgeKind::Calls && e.source.path() == &p("caller.test.ts"))
    );
    assert!(
        a.graph
            .edges()
            .iter()
            .any(|e| e.kind == EdgeKind::Reexports)
    );
    assert!(a.graph.unresolved().is_empty());
}
#[test]
fn parameters_blocks_and_destructuring_shadow_imported_targets() {
    for body in [
        "function caller(f: unknown) { f(); }",
        "function caller() { const f = 3; f(); }",
        "function caller({ f }: any) { f(); }",
    ] {
        let text = format!("import {{ f }} from './lib'; {body}");
        let a = graph(&[("lib.ts", "export function f() {}"), ("caller.ts", &text)]);
        assert!(!a.graph.edges().iter().any(|e| e.kind == EdgeKind::Calls));
        assert!(!a.graph.unresolved().is_empty());
    }
}
#[test]
fn receiver_dynamic_constructor_and_optional_calls_never_use_global_name_matching() {
    let a = graph(&[(
        "a.ts",
        "class C { save() {} } function save() {} const obj = {}; obj.save(); new C(); save?.(); import('./other');",
    )]);
    assert!(!a.graph.edges().iter().any(|e| e.kind == EdgeKind::Calls));
    assert!(a.graph.unresolved().len() >= 4);
}
#[test]
fn type_only_imports_never_become_runtime_edges() {
    let a = graph(&[
        ("lib.ts", "export function f() {}"),
        ("a.ts", "import type { f } from './lib'; f();"),
    ]);
    assert!(!a.graph.edges().iter().any(|e| e.kind == EdgeKind::Calls));
    assert!(
        a.graph
            .unresolved()
            .iter()
            .any(|u| u.reason.contains("type-only"))
    );
}
#[test]
fn recursion_and_value_reads_resolve_against_declarations() {
    let a = graph(&[("a.ts", "function f() { f(); } const alias = f;")]);
    assert!(
        a.graph
            .edges()
            .iter()
            .any(|e| e.kind == EdgeKind::Calls && e.source == e.target)
    );
    assert!(
        a.graph
            .edges()
            .iter()
            .any(|e| e.kind == EdgeKind::References)
    );
}
#[test]
fn missing_modules_and_calls_retain_distinct_boundaries_and_roundtrip() {
    let a = graph(&[("a.ts", "import { f } from './absent'; f();")]);
    assert!(a.graph.unresolved().len() >= 2);
    let bytes = serde_json::to_vec(&a).unwrap();
    let b: GraphAnalysis = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(a, b);
    assert!(b.incomplete);
}
#[test]
fn same_programs_in_different_inventory_order_are_equal() {
    let a = ("a.ts", "export function f() {}");
    let b = ("b.ts", "import {f} from './a'; f();");
    assert_eq!(graph(&[a, b]), graph(&[b, a]));
}
#[test]
fn default_import_calls_and_const_arrow_calls_resolve_without_receiver_guesses() {
    let a = graph(&[
        ("lib.ts", "export default function f() {}"),
        (
            "a.ts",
            "import run from './lib'; const arrow = () => run(); arrow();",
        ),
    ]);
    assert_eq!(
        a.graph
            .edges()
            .iter()
            .filter(|e| e.kind == EdgeKind::Calls)
            .count(),
        2
    );
    assert!(a.graph.unresolved().is_empty());
}
#[test]
fn an_empty_module_specifier_is_an_explicit_boundary_not_a_graph_failure() {
    let a = graph(&[("a.ts", "import ''; export function f() {}")]);
    assert_eq!(a.graph.unresolved().len(), 1);
    assert!(a.modules.dependencies[0].lookup.specifier.is_empty());
    assert!(!a.graph.unresolved()[0].text.is_empty());
}

#[test]
fn whitespace_only_module_requests_also_retain_a_valid_boundary() {
    let a = graph(&[("a.ts", "import ' '; export function f() {}")]);
    assert_eq!(a.graph.unresolved().len(), 1);
    assert_eq!(a.modules.dependencies[0].lookup.specifier, " ");
}
