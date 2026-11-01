use ripple_core::{
    identity::{ProjectKey, RepoPath},
    source::{SourceFile, SourceSnapshot},
    syntax::FileFacts,
};
use ripple_resolve::{
    ModuleResolution,
    exports::BindingOutcome,
    modules::{ModuleOutcome, ModuleProblem, ModuleResolver},
    resolve,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/resolution/modules")
}
fn p(path: &str) -> RepoPath {
    RepoPath::new(path).unwrap()
}
fn captured(inputs: &[(&str, &str)]) -> (SourceSnapshot, Vec<FileFacts>) {
    let mut files = Vec::new();
    let mut facts = Vec::new();
    for (path, text) in inputs {
        let source = SourceFile::new(p(path), (*text).into());
        let parsed = ripple_parse::parser::parse(p(path), text.as_bytes()).unwrap();
        facts.push(ripple_parse::extract(&parsed, ProjectKey::single_project()).unwrap());
        files.push(source);
    }
    (SourceSnapshot::new(files).unwrap(), facts)
}
fn result(inputs: &[(&str, &str)]) -> ModuleResolution {
    let (sources, facts) = captured(inputs);
    resolve(&ProjectKey::single_project(), &sources, &facts).unwrap()
}
fn sorted(mut values: Vec<Value>) -> Vec<Value> {
    values.sort_by_key(Value::to_string);
    values
}
#[test]
fn independent_lookup_fixture_defines_supported_precedence_and_failures() {
    let fixture: Value =
        serde_json::from_str(&fs::read_to_string(fixtures().join("lookup.json")).unwrap()).unwrap();
    let resolver = ModuleResolver::new(
        fixture["inventory"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| p(v.as_str().unwrap())),
    )
    .unwrap();
    let importer = p(fixture["importer"].as_str().unwrap());
    for case in fixture["cases"].as_array().unwrap() {
        let specifier = case["specifier"].as_str().unwrap();
        let lookup = resolver.lookup(&importer, specifier);
        let encoded = serde_json::to_value(&lookup.outcome).unwrap();
        if case.get("target").is_some() {
            assert_eq!(encoded["path"], case["target"], "{specifier}");
            assert_eq!(encoded["rule"], case["rule"], "{specifier}");
            assert!(lookup.attempts.last().unwrap().present);
        } else {
            assert_eq!(encoded["reason"], case["reason"], "{specifier}");
            assert!(lookup.attempts.iter().all(|a| !a.present));
        }
        assert_eq!(lookup, resolver.lookup(&importer, specifier));
    }
    let folder = resolver.lookup(&importer, "./folder");
    assert_eq!(
        folder
            .attempts
            .iter()
            .map(|a| a.path.as_str())
            .collect::<Vec<_>>(),
        vec![
            "src/folder.ts",
            "src/folder.tsx",
            "src/folder.d.ts",
            "src/folder/index.ts",
            "src/folder/index.tsx"
        ]
    );
}
#[test]
fn independent_direct_export_and_import_alias_fixture_never_guesses_opaque_targets() {
    let fixture: Value =
        serde_json::from_str(&fs::read_to_string(fixtures().join("bindings.json")).unwrap())
            .unwrap();
    let texts = fixture["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            let path = v.as_str().unwrap();
            (
                path.to_string(),
                fs::read_to_string(fixtures().join("project").join(path)).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let input = texts
        .iter()
        .map(|(p, t)| (p.as_str(), t.as_str()))
        .collect::<Vec<_>>();
    let (sources, mut facts) = captured(&input);
    let actual = resolve(&ProjectKey::single_project(), &sources, &facts).unwrap();
    let bindings=actual.imports.iter().filter(|i|i.file.as_str()=="client.ts").flat_map(|i|&i.bindings).map(|b|{
        match &b.binding {
            BindingOutcome::Symbol{identity}=>json!({"local":b.local,"type_only":b.type_only,"target":{"file":identity.path(),"name":identity.qualified_name(),"kind":identity.kind(),"role":identity.disambiguator()}}),
            BindingOutcome::Unresolved{..}=>json!({"local":b.local,"type_only":b.type_only,"reason":serde_json::to_value(&b.binding).unwrap()["reason"]}),
        }
    }).collect();
    assert_eq!(
        sorted(bindings),
        sorted(fixture["bindings"].as_array().unwrap().clone())
    );
    assert!(actual.incomplete_resolution);
    assert!(!actual.configuration_evaluated);
    assert_eq!(actual.deferred_exports.len(), 1);
    assert!(
        actual
            .dependencies
            .iter()
            .any(|d| d.lookup.specifier == "./absent" && d.lookup.attempts.len() == 6)
    );
    assert!(actual.diagnostics.iter().any(|d| d.reason == "NonRelative"));
    facts.reverse();
    assert_eq!(
        actual,
        resolve(&ProjectKey::single_project(), &sources, &facts).unwrap()
    );
    assert_eq!(
        actual,
        serde_json::from_value::<ModuleResolution>(serde_json::to_value(&actual).unwrap()).unwrap()
    );
}
#[test]
fn missing_higher_priority_candidates_and_later_repairs_are_recorded() {
    let importer = p("src/main.ts");
    let first = ModuleResolver::new([importer.clone(), p("src/a.tsx")])
        .unwrap()
        .lookup(&importer, "./a");
    assert_eq!(
        first
            .attempts
            .iter()
            .map(|a| (a.path.as_str(), a.present))
            .collect::<Vec<_>>(),
        vec![("src/a.ts", false), ("src/a.tsx", true)]
    );
    let repaired = ModuleResolver::new([importer.clone(), p("src/a.tsx"), p("src/a.ts")])
        .unwrap()
        .lookup(&importer, "./a");
    assert!(matches!(repaired.outcome,ModuleOutcome::Found{path,..} if path==p("src/a.ts")));
    let absent = ModuleResolver::new([importer.clone()])
        .unwrap()
        .lookup(&importer, "./a");
    assert_eq!(absent.attempts.len(), 6);
}
#[test]
fn explicit_ts_and_directory_slashes_do_not_switch_to_another_file_kind() {
    let main = p("main.ts");
    let r =
        ModuleResolver::new([main.clone(), p("a.tsx"), p("dir.ts"), p("dir/index.ts")]).unwrap();
    assert!(matches!(
        r.lookup(&main, "./a.ts").outcome,
        ModuleOutcome::Unresolved {
            reason: ModuleProblem::Missing
        }
    ));
    assert!(
        matches!(r.lookup(&main,"./dir/").outcome,ModuleOutcome::Found{path,..} if path==p("dir/index.ts"))
    );
}
#[test]
fn malformed_and_unknown_inventory_inputs_fail_explicitly() {
    assert!(ModuleResolver::new([p("a.ts"), p("A.ts")]).is_err());
    assert!(ModuleResolver::new([p("a.js")]).is_err());
    let r = ModuleResolver::new([p("main.ts")]).unwrap();
    assert_eq!(
        r.lookup(&p("other.ts"), "./a").outcome,
        ModuleOutcome::Unresolved {
            reason: ModuleProblem::UnknownImporter
        }
    );
    assert_eq!(
        r.lookup(&p("main.ts"), &format!("./{}", "a".repeat(4096)))
            .outcome,
        ModuleOutcome::Unresolved {
            reason: ModuleProblem::SpecifierTooLong
        }
    );
    let (sources, mut facts) = captured(&[("a.ts", "export function a(){}")]);
    assert!(resolve(&ProjectKey::single_project(), &sources, &[]).is_err());
    facts.push(facts[0].clone());
    assert!(resolve(&ProjectKey::single_project(), &sources, &facts).is_err());
    facts.pop();
    facts[0].symbols[0].span =
        ripple_core::source::SourceSpan::from_offsets("longer source text", 0, 1).unwrap();
    assert!(resolve(&ProjectKey::single_project(), &sources, &facts).is_err());
}
#[test]
fn type_only_exposure_is_never_promoted_to_a_runtime_binding() {
    let out = result(&[
        ("a.ts", "function f(){} export type {f};"),
        ("b.ts", "import {f} from './a';"),
    ]);
    let binding = &out.imports[0].bindings[0];
    assert!(binding.type_only);
    assert!(matches!(binding.binding, BindingOutcome::Symbol { .. }));
}
#[test]
fn imported_alias_forwarding_and_star_exports_are_deferred_with_lookup_evidence() {
    let out = result(&[
        ("a.ts", "export function f(){}"),
        ("barrel.ts", "export * from './a';"),
        ("main.ts", "import {f} from './barrel';"),
    ]);
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.code == "reexport_deferred")
    );
    assert_eq!(out.dependencies.len(), 2);
    assert!(!out.deferred_exports.is_empty());
}
#[test]
fn recovered_and_symbol_only_provider_facts_cannot_prove_bindings() {
    let (sources, mut facts) = captured(&[
        ("a.ts", "export function f(){}"),
        ("main.ts", "import {f} from './a';"),
    ]);
    facts[0].scopes.clear();
    let out = resolve(&ProjectKey::single_project(), &sources, &facts).unwrap();
    assert!(
        out.imports[0]
            .bindings
            .iter()
            .all(|b| matches!(b.binding, BindingOutcome::Unresolved { .. }))
    );
    let broken = result(&[
        ("a.ts", "export function f( {"),
        ("main.ts", "import {f} from './a';"),
    ]);
    assert!(broken.incomplete_resolution);
}
#[test]
fn adding_a_captured_provider_repairs_a_previously_missing_import_in_a_cold_rerun() {
    let before = result(&[("client.ts", "import {run} from './new';")]);
    assert!(matches!(
        before.imports[0].lookup.outcome,
        ModuleOutcome::Unresolved {
            reason: ModuleProblem::Missing
        }
    ));
    assert_eq!(before.dependencies[0].lookup.attempts.len(), 6);
    let after = result(&[
        ("client.ts", "import {run} from './new';"),
        ("new.ts", "export function run(){}"),
    ]);
    assert!(
        matches!(&after.imports[0].bindings[0].binding,BindingOutcome::Symbol{identity} if identity.path()==&p("new.ts"))
    );
    assert!(!after.incomplete_resolution);
    assert!(!after.configuration_evaluated);
}
#[test]
fn wrong_project_or_cross_file_symbol_facts_are_rejected_before_resolution() {
    let (sources, facts) = captured(&[("a.ts", "export function f(){}")]);
    assert!(resolve(&ProjectKey::new("other").unwrap(), &sources, &facts).is_err());
    let mut cross = facts.clone();
    cross[0].path = p("outside.ts");
    assert!(resolve(&ProjectKey::single_project(), &sources, &cross).is_err());
}
