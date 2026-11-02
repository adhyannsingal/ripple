use ripple_core::{
    identity::{ProjectKey, RepoPath},
    source::{SourceFile, SourceSnapshot},
    syntax::{ExportName, FileFacts},
};
use ripple_resolve::{
    config::{CapturedConfigs, ConfigInput, EffectiveConfig},
    reexports::{ExportBinding, ExportProblem, ModuleAnalysis, resolve_modules},
};
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};
fn p(path: &str) -> RepoPath {
    RepoPath::new(path).unwrap()
}
fn config(text: &str) -> EffectiveConfig {
    CapturedConfigs::new([ConfigInput {
        path: p("tsconfig.json"),
        bytes: text.as_bytes().to_vec(),
    }])
    .unwrap()
    .evaluate(&p("tsconfig.json"))
    .unwrap()
}
fn default_config() -> EffectiveConfig {
    config(r#"{"compilerOptions":{"paths":{"@/*":["./*"]}}}"#)
}
fn captured(inputs: &[(String, String)]) -> (SourceSnapshot, Vec<FileFacts>) {
    let mut sources = Vec::new();
    let mut facts = Vec::new();
    for (path, text) in inputs {
        let parsed = ripple_parse::parser::parse(p(path), text.as_bytes()).unwrap();
        facts.push(ripple_parse::extract(&parsed, ProjectKey::single_project()).unwrap());
        sources.push(SourceFile::new(p(path), text.clone()));
    }
    (SourceSnapshot::new(sources).unwrap(), facts)
}
fn analyze(inputs: &[(&str, &str)]) -> ModuleAnalysis {
    let data = inputs
        .iter()
        .map(|(p, t)| ((*p).into(), (*t).into()))
        .collect::<Vec<_>>();
    let (sources, facts) = captured(&data);
    resolve_modules(
        &ProjectKey::single_project(),
        &sources,
        &facts,
        &default_config(),
    )
    .unwrap()
}
fn export<'a>(a: &'a ModuleAnalysis, file: &str, name: &str) -> &'a ExportBinding {
    &a.exports
        .iter()
        .find(|e| {
            e.key.file == p(file)
                && e.key.name
                    == if name == "default" {
                        ExportName::Default
                    } else {
                        ExportName::Named(name.into())
                    }
        })
        .unwrap()
        .binding
}
fn problem(binding: &ExportBinding, problem: ExportProblem) {
    assert!(
        matches!(binding,ExportBinding::Unresolved{reasons,..} if reasons.contains(&problem)),
        "{binding:?}"
    );
}
fn symbol(binding: &ExportBinding, file: &str, name: &str, type_only: bool) {
    match binding {
        ExportBinding::Symbol {
            identity,
            type_only: flag,
            ..
        } => {
            assert_eq!(identity.path(), &p(file));
            assert_eq!(identity.qualified_name().components(), [name.to_string()]);
            assert_eq!(*flag, type_only);
        }
        other => panic!("{other:?}"),
    }
}
fn check_witnesses(a: &ModuleAnalysis, facts: &[FileFacts]) {
    for binding in a
        .exports
        .iter()
        .map(|e| &e.binding)
        .chain(a.imports.iter().map(|i| &i.binding))
    {
        if let ExportBinding::Symbol {
            identity,
            declaration,
            witness,
            ..
        } = binding
        {
            assert_eq!(declaration.file, *identity.path());
            for (i, step) in witness.iter().enumerate() {
                assert!(a.transfers.contains(step));
                assert_eq!(step.location.file, step.from.file);
                if let Some(next) = witness.get(i + 1) {
                    assert_eq!(step.to, next.from);
                }
                let file = facts.iter().find(|f| f.path == step.from.file).unwrap();
                assert!(file.exports.iter().any(|e| e.span == step.location.span));
            }
            if let Some(last) = witness.last() {
                assert_eq!(last.to.file, *identity.path());
            }
            assert!(facts.iter().any(|f| {
                f.path == *identity.path()
                    && f.symbols
                        .iter()
                        .any(|s| &s.identity == identity && s.span == declaration.span)
            }));
        }
    }
}
fn fixtures() -> Vec<(String, String)> {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/resolution/reexports/project");
    let mut inputs = fs::read_dir(root)
        .unwrap()
        .map(|e| {
            let path = e.unwrap().path();
            (
                path.file_name().unwrap().to_str().unwrap().to_string(),
                fs::read_to_string(path).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    inputs.sort();
    inputs
}
#[test]
fn independent_alias_barrel_collision_default_type_and_cycle_fixture() {
    let data = fixtures();
    let (sources, facts) = captured(&data);
    let a = resolve_modules(
        &ProjectKey::single_project(),
        &sources,
        &facts,
        &default_config(),
    )
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../fixtures/resolution/reexports/expected.json"
    ))
    .unwrap();
    let actual = a
        .imports
        .iter()
        .filter(|i| i.file == p("client.ts"))
        .map(|i| (i.local.as_str(), i))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual.len(), expected.as_array().unwrap().len());
    for item in expected.as_array().unwrap() {
        let i = actual[item["local"].as_str().unwrap()];
        if let Some(file) = item["file"].as_str() {
            match &i.binding {
                ExportBinding::Symbol { identity, .. } => {
                    assert_eq!(identity.path(), &p(file));
                    assert_eq!(
                        identity.qualified_name().components(),
                        [item["name"].as_str().unwrap()]
                    );
                    assert_eq!(i.type_only, item["type_only"].as_bool().unwrap());
                }
                other => panic!("{item}: {other:?}"),
            }
        } else {
            let encoded = serde_json::to_value(&i.binding).unwrap();
            let reasons = encoded["Unresolved"]["reasons"].as_array().unwrap();
            assert!(reasons.contains(&item["problem"]), "{item}: {encoded}");
        }
    }
    check_witnesses(&a, &facts);
    assert!(a.incomplete);
    assert!(
        a.dependencies
            .iter()
            .any(|d| d.lookup.specifier == "@/origin")
    );
    assert!(
        a.dependencies
            .iter()
            .any(|d| d.lookup.specifier == "./absent"
                && d.lookup.attempts.iter().all(|p| !p.present))
    );
}
#[test]
fn anchored_cycles_terminate_and_empty_cycles_stay_unresolved() {
    let a = analyze(&[
        ("origin.ts", "export function f() {}"),
        ("a.ts", "export * from './b'; export { f } from './origin';"),
        ("b.ts", "export * from './a';"),
        ("x.ts", "export { x } from './y';"),
        ("y.ts", "export { x } from './x';"),
    ]);
    symbol(export(&a, "b.ts", "f"), "origin.ts", "f", false);
    problem(export(&a, "x.ts", "x"), ExportProblem::NoSupportedOrigin);
    problem(export(&a, "y.ts", "x"), ExportProblem::NoSupportedOrigin);
}
#[test]
fn an_empty_cyclic_branch_cannot_leave_a_false_unique_star_winner() {
    let a = analyze(&[
        ("origin.ts", "export function f() {}"),
        ("a.ts", "export { f } from './b';"),
        ("b.ts", "export { f } from './a';"),
        ("join.ts", "export * from './origin'; export * from './a';"),
    ]);
    problem(export(&a, "join.ts", "f"), ExportProblem::NoSupportedOrigin);
}
#[test]
fn unknown_star_taint_propagates_but_explicit_names_override_it() {
    let a = analyze(&[
        ("origin.ts", "export function f() {}"),
        (
            "a.ts",
            "export * from './origin'; export * from './missing'; export { f as exact } from './origin';",
        ),
        ("b.ts", "export * from './a';"),
    ]);
    problem(export(&a, "b.ts", "f"), ExportProblem::UnknownStarSurface);
    symbol(export(&a, "b.ts", "exact"), "origin.ts", "f", false);
    assert!(
        a.surfaces
            .iter()
            .filter(|s| s.file != p("origin.ts"))
            .all(|s| s.unknown_star_surface)
    );
}
#[test]
fn stars_exclude_default_and_explicit_default_aliases_reach_the_declaration() {
    let a = analyze(&[
        ("origin.ts", "export default function f() {}"),
        ("star.ts", "export * from './origin';"),
        (
            "named.ts",
            "export { default as named, default } from './origin';",
        ),
        (
            "client.ts",
            "import bad from './star'; import good, { named } from './named';",
        ),
    ]);
    assert!(!a.exports.iter().any(|e| e.key.file == p("star.ts")));
    problem(
        &a.imports.iter().find(|i| i.local == "bad").unwrap().binding,
        ExportProblem::MissingExport,
    );
    symbol(export(&a, "named.ts", "named"), "origin.ts", "f", false);
    symbol(export(&a, "named.ts", "default"), "origin.ts", "f", false);
}
#[test]
fn same_origin_diamonds_are_unique_and_distinct_origins_remain_ambiguous() {
    let a = analyze(&[
        ("origin.ts", "export function f() {}"),
        ("left.ts", "export { f } from './origin';"),
        ("right.ts", "export { f } from './origin';"),
        (
            "join.ts",
            "export * from './left'; export * from './right';",
        ),
        ("other.ts", "export function f() {}"),
        (
            "collision.ts",
            "export * from './join'; export * from './other';",
        ),
    ]);
    symbol(export(&a, "join.ts", "f"), "origin.ts", "f", false);
    match export(&a, "collision.ts", "f") {
        ExportBinding::Unresolved { candidates, .. } => assert_eq!(candidates.len(), 2),
        other => panic!("{other:?}"),
    };
}
#[test]
fn type_only_forwarding_and_same_origin_runtime_paths_remain_distinct() {
    let a = analyze(&[
        ("origin.ts", "export function f() {}"),
        (
            "types.ts",
            "import type { f } from './origin'; export { f }; export type Shape = { x: number };",
        ),
        ("runtime.ts", "export { f } from './origin';"),
        (
            "join.ts",
            "export * from './types'; export * from './runtime';",
        ),
        (
            "client.ts",
            "import { f } from './types'; import type { f as only } from './runtime';",
        ),
    ]);
    symbol(export(&a, "types.ts", "f"), "origin.ts", "f", true);
    symbol(export(&a, "join.ts", "f"), "origin.ts", "f", false);
    assert!(
        a.imports
            .iter()
            .filter(|i| i.file == p("client.ts"))
            .all(|i| i.type_only)
    );
    let shape = a
        .exports
        .iter()
        .find(|e| e.key.file == p("types.ts") && e.key.name == ExportName::Named("Shape".into()))
        .unwrap();
    assert_eq!(shape.syntactic_type_only, Some(true));
    problem(&shape.binding, ExportProblem::UnsupportedLocal);
}
#[test]
fn duplicate_explicit_and_shadowed_or_namespace_forwarding_never_guess() {
    let a = analyze(&[
        ("origin.ts", "export function f() {}"),
        (
            "duplicate.ts",
            "export { f } from './origin'; export { f } from './origin';",
        ),
        (
            "shadow.ts",
            "import { f } from './origin'; const f = () => 1; export { f };",
        ),
        (
            "namespace.ts",
            "import * as ns from './origin'; export { ns };",
        ),
    ]);
    problem(
        export(&a, "duplicate.ts", "f"),
        ExportProblem::DuplicateExplicit,
    );
    assert!(matches!(
        export(&a, "shadow.ts", "f"),
        ExportBinding::Unresolved { .. }
    ));
    problem(
        export(&a, "namespace.ts", "ns"),
        ExportProblem::UnsupportedNamespace,
    );
}
#[test]
fn deletion_and_missing_provider_repairs_change_outputs_and_external_fingerprints() {
    let before = analyze(&[
        ("client.ts", "import { f } from './barrel';"),
        ("barrel.ts", "export { f } from './provider';"),
    ]);
    problem(&before.imports[0].binding, ExportProblem::ModuleUnavailable);
    let after = analyze(&[
        ("client.ts", "import { f } from './barrel';"),
        ("barrel.ts", "export { f } from './provider';"),
        ("provider.ts", "export function f() {}"),
    ]);
    symbol(&after.imports[0].binding, "provider.ts", "f", false);
    assert_ne!(
        before
            .surfaces
            .iter()
            .find(|s| s.file == p("barrel.ts"))
            .unwrap()
            .fingerprint,
        after
            .surfaces
            .iter()
            .find(|s| s.file == p("barrel.ts"))
            .unwrap()
            .fingerprint
    );
    assert_eq!(
        before,
        analyze(&[
            ("barrel.ts", "export { f } from './provider';"),
            ("client.ts", "import { f } from './barrel';")
        ])
    );
}
#[test]
fn opaque_recovered_and_unsupported_config_sources_do_not_create_proven_exports() {
    let a = analyze(&[
        (
            "origin.ts",
            "export function f() {}\nexport function broken(",
        ),
        ("barrel.ts", "export * from './origin';"),
        ("client.ts", "import { f } from './barrel';"),
    ]);
    assert!(a.incomplete);
    assert!(matches!(
        a.imports[0].binding,
        ExportBinding::Unresolved { .. }
    ));
    let data = vec![
        ("origin.ts".into(), "export function f() {}".into()),
        ("client.ts".into(), "import { f } from './origin';".into()),
    ];
    let (sources, facts) = captured(&data);
    let bad = config(r#"{"compilerOptions":{"moduleResolution":"nodenext"}}"#);
    let a = resolve_modules(&ProjectKey::single_project(), &sources, &facts, &bad).unwrap();
    assert!(!a.config_diagnostics.is_empty());
    assert!(
        a.exports
            .iter()
            .all(|e| matches!(e.binding, ExportBinding::Unresolved { .. }))
    );
    assert!(
        a.imports
            .iter()
            .all(|i| matches!(i.binding, ExportBinding::Unresolved { .. }))
    );
}
#[test]
fn module_artifacts_roundtrip_and_require_revalidation_after_tampering() {
    let data = fixtures();
    let (sources, facts) = captured(&data);
    let cfg = default_config();
    let a = resolve_modules(&ProjectKey::single_project(), &sources, &facts, &cfg).unwrap();
    let bytes = serde_json::to_vec(&a).unwrap();
    let mut loaded: ModuleAnalysis = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(a, loaded);
    loaded
        .validate(&ProjectKey::single_project(), &sources, &facts, &cfg)
        .unwrap();
    loaded.surfaces[0].fingerprint.digest = "0".repeat(64);
    assert!(
        loaded
            .validate(&ProjectKey::single_project(), &sources, &facts, &cfg)
            .is_err()
    );
}
#[test]
fn input_order_and_cold_repetition_are_deterministic() {
    let mut data = fixtures();
    let (sources, mut facts) = captured(&data);
    let cfg = default_config();
    let a = resolve_modules(&ProjectKey::single_project(), &sources, &facts, &cfg).unwrap();
    data.reverse();
    facts.reverse();
    let (other, _) = captured(&data);
    let b = resolve_modules(&ProjectKey::single_project(), &other, &facts, &cfg).unwrap();
    assert_eq!(a, b);
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
}
#[test]
fn external_fingerprints_track_signature_and_ambiguity_without_treating_body_hash_as_api_proof() {
    let base = [
        (
            "origin.ts",
            "export function f(x: number): number { return 1; }",
        ),
        ("barrel.ts", "export { f } from './origin';"),
    ];
    let a = analyze(&base);
    let body = analyze(&[
        (
            "origin.ts",
            "export function f(x: number): number { return 2; }",
        ),
        base[1],
    ]);
    assert_eq!(a.surfaces, body.surfaces);
    let signature = analyze(&[
        (
            "origin.ts",
            "export function f(x: string): number { return 2; }",
        ),
        base[1],
    ]);
    assert_ne!(a.surfaces, signature.surfaces);
}
#[test]
fn configured_alias_failure_and_negative_probes_survive_serialization() {
    let a = analyze(&[
        ("barrel.ts", "export { f } from '@/missing';"),
        ("client.ts", "import { f } from './barrel';"),
    ]);
    let lookup = &a
        .dependencies
        .iter()
        .find(|d| d.lookup.specifier == "@/missing")
        .unwrap()
        .lookup;
    assert_eq!(lookup.attempts.len(), 6);
    assert!(lookup.attempts.iter().all(|a| !a.present));
    let text = serde_json::to_string(&a).unwrap();
    assert!(text.contains("@/missing"));
    assert!(text.contains("config_hash"));
}
#[test]
fn invalid_fact_project_or_incomplete_inventory_is_rejected() {
    let data = vec![("origin.ts".into(), "export function f() {}".into())];
    let (sources, facts) = captured(&data);
    assert!(
        resolve_modules(
            &ProjectKey::new("other").unwrap(),
            &sources,
            &facts,
            &default_config()
        )
        .is_err()
    );
    assert!(
        resolve_modules(
            &ProjectKey::single_project(),
            &sources,
            &[],
            &default_config()
        )
        .is_err()
    );
}
#[test]
fn long_barrel_work_exhaustion_is_an_error_not_a_partial_unique_result() {
    let mut data = (0..520)
        .map(|i| (format!("m{i}.ts"), format!("export * from './m{}';", i + 1)))
        .collect::<Vec<_>>();
    data.push(("m520.ts".into(), "export function f() {}".into()));
    let (sources, facts) = captured(&data);
    let result = resolve_modules(
        &ProjectKey::single_project(),
        &sources,
        &facts,
        &default_config(),
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().0.contains("512"));
}
#[test]
fn a_second_unknown_branch_still_blocks_an_otherwise_explicit_provider_name() {
    let a = analyze(&[
        ("origin.ts", "export function f() {}"),
        (
            "safe.ts",
            "export { f as exact } from './origin'; export * from './missing';",
        ),
        ("other.ts", "export * from './anotherMissing';"),
        (
            "join.ts",
            "export * from './safe'; export * from './other';",
        ),
    ]);
    symbol(export(&a, "safe.ts", "exact"), "origin.ts", "f", false);
    problem(
        export(&a, "join.ts", "exact"),
        ExportProblem::UnknownStarSurface,
    );
}
#[test]
fn local_export_alias_witnesses_point_to_the_actual_declaration_span() {
    let inputs = vec![
        (
            "origin.ts".into(),
            "function f() {}\nexport { f as alias };".into(),
        ),
        (
            "barrel.ts".into(),
            "export { alias as result } from './origin';".into(),
        ),
    ];
    let (sources, facts) = captured(&inputs);
    let a = resolve_modules(
        &ProjectKey::single_project(),
        &sources,
        &facts,
        &default_config(),
    )
    .unwrap();
    symbol(export(&a, "barrel.ts", "result"), "origin.ts", "f", false);
    check_witnesses(&a, &facts);
}
#[test]
fn anonymous_default_identity_survives_named_and_default_barrels() {
    let a = analyze(&[
        ("origin.ts", "export default () => 1;"),
        (
            "barrel.ts",
            "export { default, default as named } from './origin';",
        ),
        ("client.ts", "import f, { named } from './barrel';"),
    ]);
    for i in &a.imports {
        if let ExportBinding::Symbol { identity, .. } = &i.binding {
            assert_eq!(identity.path(), &p("origin.ts"));
            assert_eq!(identity.disambiguator(), "default-export");
        } else {
            panic!("{:?}", i.binding);
        }
    }
}
