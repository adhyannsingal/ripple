use ripple_core::{identity::RepoPath, syntax::VersionedHash};
use ripple_resolve::{
    config::{CapturedConfigs, ConfigInput, EffectiveConfig},
    modules::ModuleProblem,
    paths::{ConfigLookupProblem, ConfigOutcome, ConfigRoute, ConfiguredResolver},
};
use serde_json::{Value, json};
fn p(path: &str) -> RepoPath {
    RepoPath::new(path).unwrap()
}
fn captured(inputs: &[(&str, &str)]) -> CapturedConfigs {
    CapturedConfigs::new(inputs.iter().map(|(path, text)| ConfigInput {
        path: p(path),
        bytes: text.as_bytes().to_vec(),
    }))
    .unwrap()
}
fn evaluate(inputs: &[(&str, &str)]) -> EffectiveConfig {
    captured(inputs).evaluate(&p("tsconfig.json")).unwrap()
}
fn lookup(config: &EffectiveConfig, specifier: &str) -> ripple_resolve::paths::ConfiguredLookup {
    ConfiguredResolver::new(config, [p("main.ts"), p("src/a.ts"), p("other/a.ts")])
        .unwrap()
        .lookup(&p("main.ts"), specifier)
}
fn has_code(config: &EffectiveConfig, code: &str) -> bool {
    config.diagnostics().iter().any(|d| d.code == code)
}
#[test]
fn independently_expected_aliases_targets_precedence_and_probes() {
    let config = captured(&[
        (
            "apps/tsconfig.json",
            include_str!("../../../fixtures/resolution/config/root.jsonc"),
        ),
        (
            "configs/base.json",
            include_str!("../../../fixtures/resolution/config/base.jsonc"),
        ),
    ])
    .evaluate(&p("apps/tsconfig.json"))
    .unwrap();
    assert!(config.supported(), "{:?}", config.diagnostics());
    assert_eq!(config.base_url(), Some("src"));
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/resolution/config/lookup.json"
    ))
    .unwrap();
    let resolver = ConfiguredResolver::new(
        &config,
        fixture["inventory"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| p(v.as_str().unwrap())),
    )
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let result = resolver.lookup(&p("apps/main.ts"), case["specifier"].as_str().unwrap());
        match result.outcome {
            ConfigOutcome::Found { path, .. } => {
                assert_eq!(path.as_str(), case["target"].as_str().unwrap())
            }
            ref other => panic!("{case}: {other:?}"),
        };
        assert_eq!(
            result.attempts.len(),
            case["probes"].as_u64().unwrap() as usize
        );
        assert!(result.attempts.last().unwrap().present);
        assert!(
            result.attempts[..result.attempts.len() - 1]
                .iter()
                .all(|a| !a.present)
        );
        if let Some(pattern) = case["pattern"].as_str() {
            assert!(
                matches!(&result.routes[0],ConfigRoute::Paths{pattern:actual,..} if actual==pattern)
            );
        }
        assert_eq!(&result.config_hash, config.fingerprint());
    }
    let lookup = resolver.lookup(&p("apps/main.ts"), "@app/util");
    assert_eq!(lookup.attempts[0].path, p("src/missing/util.ts"));
    assert_eq!(lookup.attempts[5].path, p("src/missing/util/index.d.ts"));
}
#[test]
fn fingerprint_matches_independent_python_sha256_recipe() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/resolution/config/fingerprint.json"
    ))
    .unwrap();
    let config = evaluate(&[("tsconfig.json", fixture["raw"].as_str().unwrap())]);
    assert!(config.supported());
    assert_eq!(
        config.inputs()[0].content_hash.as_ref().unwrap().digest,
        fixture["input_sha256"]
    );
    assert_eq!(config.fingerprint().digest, fixture["effective_sha256"]);
    assert_eq!(config.fingerprint().version, 1);
}
#[test]
fn fingerprints_include_every_consumed_byte_missing_probe_and_policy_but_not_unused_inventory() {
    let root = r#"{"extends":"./base"}"#;
    let base = r#"{"compilerOptions":{"baseUrl":"src"}}"#;
    let a = evaluate(&[("tsconfig.json", root), ("base.json", base)]);
    let b = evaluate(&[
        ("base.json", base),
        ("unused.json", "garbage"),
        ("tsconfig.json", root),
    ]);
    assert_eq!(a, b);
    assert_eq!(
        a.inputs()
            .iter()
            .map(|d| d.path.as_str())
            .collect::<Vec<_>>(),
        ["base", "base.json", "tsconfig.json"]
    );
    assert!(a.inputs()[0].content_hash.is_none());
    let comments = evaluate(&[
        ("tsconfig.json", root),
        ("base.json", &format!("{base}\n// trivia")),
    ]);
    assert_eq!(a.base_url(), comments.base_url());
    assert_ne!(a.fingerprint(), comments.fingerprint());
    let exact = evaluate(&[("tsconfig.json", root), ("base", base), ("base.json", base)]);
    assert_ne!(a.fingerprint(), exact.fingerprint());
    assert!(exact.inputs().iter().all(|i| i.path != p("base.json")));
}
#[test]
fn inherited_paths_without_base_url_keep_declaration_origin_and_child_base_url_can_override() {
    let root = r#"{"extends":"./configs/base.json"}"#;
    let base = r#"{"compilerOptions":{"paths":{"@/*":["../src/*"]}}}"#;
    let config = evaluate(&[("tsconfig.json", root), ("configs/base.json", base)]);
    assert!(config.supported());
    assert!(
        matches!(lookup(&config,"@/a").outcome,ConfigOutcome::Found{path,..} if path==p("src/a.ts"))
    );
    let root = r#"{"extends":"./configs/base.json","compilerOptions":{"baseUrl":"other"}}"#;
    let base = r#"{"compilerOptions":{"baseUrl":"src","paths":{"@/*":["*"]}}}"#;
    let config = evaluate(&[("tsconfig.json", root), ("configs/base.json", base)]);
    assert!(
        matches!(lookup(&config,"@/a").outcome,ConfigOutcome::Found{path,..} if path==p("other/a.ts"))
    );
}
#[test]
fn child_paths_replace_the_entire_parent_map_instead_of_merging() {
    let config = evaluate(&[
        (
            "tsconfig.json",
            r#"{"extends":"./base.json","compilerOptions":{"paths":{}}}"#,
        ),
        (
            "base.json",
            r#"{"compilerOptions":{"paths":{"@/*":["./src/*"]}}}"#,
        ),
    ]);
    assert!(config.supported());
    assert!(config.paths().is_empty());
    assert!(matches!(
        lookup(&config, "@/a").outcome,
        ConfigOutcome::Unresolved {
            reason: ConfigLookupProblem::Module(ModuleProblem::NonRelative)
        }
    ));
}
#[test]
fn an_exact_mapping_miss_does_not_try_less_specific_patterns() {
    let config = evaluate(&[(
        "tsconfig.json",
        r#"{"compilerOptions":{"paths":{"@/a":["./absent"],"@/*":["./src/*"]}}}"#,
    )]);
    let result = lookup(&config, "@/a");
    assert!(matches!(
        result.outcome,
        ConfigOutcome::Unresolved {
            reason: ConfigLookupProblem::Module(ModuleProblem::Missing)
        }
    ));
    assert!(result.attempts.iter().all(|a| !a.present));
    assert!(
        result
            .attempts
            .iter()
            .all(|a| !a.path.as_str().starts_with("src/"))
    );
}
#[test]
fn equally_specific_wildcards_are_explicitly_ambiguous() {
    let config = evaluate(&[(
        "tsconfig.json",
        r#"{"compilerOptions":{"paths":{"@/*":["./src/*"],"@/*a":["./other/*"]}}}"#,
    )]);
    assert!(config.supported());
    let result = lookup(&config, "@/a");
    assert_eq!(
        result.outcome,
        ConfigOutcome::Unresolved {
            reason: ConfigLookupProblem::AmbiguousAlias
        }
    );
    assert!(result.attempts.is_empty());
}
#[test]
fn jsonc_preserves_strings_bom_crlf_and_trailing_commas_but_rejects_invalid_json() {
    let config = evaluate(&[(
        "tsconfig.json",
        "\u{feff}{\r\n// line\r\n\"$schema\":\"https://example.invalid/schema/*x*/\", /* block */\"compilerOptions\":{\"paths\":{\"@/*\":[\"./src//*\",],},},}\r\n",
    )]);
    assert!(config.supported(), "{:?}", config.diagnostics());
    assert!(matches!(
        lookup(&config, "@/a").outcome,
        ConfigOutcome::Found { .. }
    ));
    for invalid in [
        "{,}",
        "[, ]",
        "{\"compilerOptions\":,}",
        "/* never closed",
        "{} trailing",
        r#"{"compilerOptions":{"baseUrl":"src","baseUrl":"other"}}"#,
        r#"{"compilerOptions":{"baseUrl":"src","base\u0055rl":"other"}}"#,
    ] {
        let c = evaluate(&[("tsconfig.json", invalid)]);
        assert!(has_code(&c, "malformed_config"), "{invalid}");
        assert!(!c.supported());
    }
}
#[test]
fn absent_configs_remain_visible_and_repair_changes_the_fingerprint() {
    let missing = evaluate(&[]);
    assert!(has_code(&missing, "missing_config"));
    assert_eq!(missing.inputs().len(), 1);
    assert!(missing.inputs()[0].content_hash.is_none());
    let root = r#"{"extends":"./base"}"#;
    let absent = evaluate(&[("tsconfig.json", root)]);
    assert_eq!(absent.inputs().len(), 3);
    let repaired = evaluate(&[("tsconfig.json", root), ("base.json", "{}")]);
    assert!(repaired.supported());
    assert_ne!(absent.fingerprint(), repaired.fingerprint());
    assert!(matches!(
        lookup(&absent, "./src/a").outcome,
        ConfigOutcome::Unresolved {
            reason: ConfigLookupProblem::UnsupportedConfiguration
        }
    ));
}
#[test]
fn inheritance_cycles_and_depth_limits_fail_without_guessing() {
    let c = evaluate(&[
        ("tsconfig.json", r#"{"extends":"./base.json"}"#),
        ("base.json", r#"{"extends":"./tsconfig.json"}"#),
    ]);
    assert!(has_code(&c, "inheritance_cycle"));
    assert_eq!(c.inputs().len(), 2);
    let mut files = vec![ConfigInput {
        path: p("tsconfig.json"),
        bytes: br#"{"extends":"./c0.json"}"#.to_vec(),
    }];
    for i in 0..33 {
        files.push(ConfigInput {
            path: p(&format!("c{i}.json")),
            bytes: format!("{{\"extends\":\"./c{}.json\"}}", i + 1).into_bytes(),
        });
    }
    let c = CapturedConfigs::new(files)
        .unwrap()
        .evaluate(&p("tsconfig.json"))
        .unwrap();
    assert!(has_code(&c, "config_depth_limit"));
    assert!(!c.supported());
}
#[test]
fn package_array_absolute_and_escaping_extends_are_diagnosed() {
    for value in [
        json!("@tsconfig/node"),
        json!(["./a.json", "./b.json"]),
        json!("/tmp/config.json"),
        json!("../../outside"),
        json!(123),
    ] {
        let text = json!({"extends":value}).to_string();
        let c = evaluate(&[("tsconfig.json", &text)]);
        assert!(has_code(&c, "unsupported_extends"), "{text}");
        assert_eq!(c.inputs().len(), 1);
    }
}
#[test]
fn unsupported_modes_references_and_resolution_options_cannot_yield_successful_lookups() {
    for text in [
        r#"{"compilerOptions":{"moduleResolution":"nodenext"}}"#,
        r#"{"compilerOptions":{"moduleResolution":"bundler"}}"#,
        r#"{"compilerOptions":{"moduleSuffixes":[".native",""]}}"#,
        r#"{"compilerOptions":{"rootDirs":["src","generated"]}}"#,
        r#"{"references":[{"path":"../other"}]}"#,
        r#"{"include":["src"]}"#,
    ] {
        let c = evaluate(&[("tsconfig.json", text)]);
        assert!(!c.supported(), "{text}");
        assert!(matches!(
            lookup(&c, "./src/a").outcome,
            ConfigOutcome::Unresolved {
                reason: ConfigLookupProblem::UnsupportedConfiguration
            }
        ));
    }
}
#[test]
fn supported_child_option_overrides_an_unsupported_inherited_value() {
    let c = evaluate(&[
        (
            "tsconfig.json",
            r#"{"extends":"./base.json","compilerOptions":{"moduleResolution":"node10"}}"#,
        ),
        (
            "base.json",
            r#"{"compilerOptions":{"moduleResolution":"bundler"}}"#,
        ),
    ]);
    assert!(c.supported(), "{:?}", c.diagnostics());
}
#[test]
fn invalid_base_urls_alias_patterns_and_targets_are_not_silently_accepted() {
    for options in [
        json!({"baseUrl":"../outside"}),
        json!({"baseUrl":"${configDir}/src"}),
        json!({"baseUrl":"/absolute"}),
        json!({"baseUrl":42}),
        json!({"baseUrl":"src\\nested"}),
        json!({"paths":{"@/**":["./src/*"]}}),
        json!({"paths":{"@/*":[]}}),
        json!({"paths":{"@/*":[null]}}),
        json!({"paths":{"@/*":["src/*"]}}),
        json!({"paths":{"exact":["./src/*"]}}),
        json!({"paths":{"@/*":["../outside/*"]}}),
    ] {
        let text = json!({"compilerOptions":options}).to_string();
        let c = evaluate(&[("tsconfig.json", &text)]);
        assert!(!c.supported(), "{text}");
    }
}
#[test]
fn wildcard_substitution_cannot_escape_the_repository() {
    let c = evaluate(&[(
        "tsconfig.json",
        r#"{"compilerOptions":{"paths":{"@/*":["./src/*"]}}}"#,
    )]);
    let result = lookup(&c, "@/../../outside");
    assert!(matches!(
        result.outcome,
        ConfigOutcome::Unresolved {
            reason: ConfigLookupProblem::Module(ModuleProblem::OutsideRoot)
        }
    ));
    assert!(result.attempts.is_empty());
}
#[test]
fn unknown_importers_invalid_specifiers_and_unmapped_packages_remain_unresolved() {
    let c = evaluate(&[("tsconfig.json", "{}")]);
    let r = ConfiguredResolver::new(&c, [p("main.ts")]).unwrap();
    assert!(matches!(
        r.lookup(&p("absent.ts"), "./a").outcome,
        ConfigOutcome::Unresolved {
            reason: ConfigLookupProblem::Module(ModuleProblem::UnknownImporter)
        }
    ));
    for invalid in ["/a", "./a?raw", "./a\\b", "./a\n"] {
        assert!(matches!(
            r.lookup(&p("main.ts"), invalid).outcome,
            ConfigOutcome::Unresolved {
                reason: ConfigLookupProblem::Module(ModuleProblem::InvalidSpecifier)
            }
        ));
    }
    assert!(matches!(
        r.lookup(&p("main.ts"), "package").outcome,
        ConfigOutcome::Unresolved {
            reason: ConfigLookupProblem::Module(ModuleProblem::NonRelative)
        }
    ));
    assert!(matches!(
        r.lookup(&p("main.ts"), &"x".repeat(4097)).outcome,
        ConfigOutcome::Unresolved {
            reason: ConfigLookupProblem::Module(ModuleProblem::SpecifierTooLong)
        }
    ));
}
#[test]
fn inventory_and_json_resource_bounds_are_explicit() {
    assert!(
        CapturedConfigs::new([ConfigInput {
            path: p("a.json"),
            bytes: vec![b' '; 1024 * 1024 + 1]
        }])
        .is_err()
    );
    assert!(
        CapturedConfigs::new([
            ConfigInput {
                path: p("a.json"),
                bytes: vec![]
            },
            ConfigInput {
                path: p("A.json"),
                bytes: vec![]
            }
        ])
        .is_err()
    );
    assert!(
        CapturedConfigs::new((0..129).map(|i| ConfigInput {
            path: p(&format!("{i}.json")),
            bytes: vec![]
        }))
        .is_err()
    );
    let c = evaluate(&[(
        "tsconfig.json",
        &format!(
            "{{\"compilerOptions\":{{\"lib\":[{}]}}}}",
            vec!["0"; 4097].join(",")
        ),
    )]);
    assert!(has_code(&c, "malformed_config"));
    let c = evaluate(&[(
        "tsconfig.json",
        &format!(
            "{{\"compilerOptions\":{{\"paths\":{{\"@/*\":[{}]}}}}}}",
            vec!["\"./src/*\""; 17].join(",")
        ),
    )]);
    assert!(has_code(&c, "invalid_paths_mapping"));
}
#[test]
fn captured_config_bytes_can_be_used_to_build_a_persistent_manifest() {
    let set = captured(&[("tsconfig.json", "{}")]);
    let config = set.evaluate(&p("tsconfig.json")).unwrap();
    let dep = &config.inputs()[0];
    let bytes = &set.get(&dep.path).unwrap().bytes;
    assert_eq!(dep.byte_len, Some(bytes.len() as u64));
    assert_eq!(
        dep.content_hash.as_ref().unwrap(),
        &VersionedHash {
            version: 1,
            digest: "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a".into()
        }
    );
    let encoded = serde_json::to_value(&config).unwrap();
    assert_eq!(encoded["root"], "tsconfig.json");
    assert!(encoded["diagnostics"].as_array().unwrap().is_empty());
}
#[test]
fn source_substitution_directory_aliases_and_root_base_url_preserve_lookup_policy() {
    let c = evaluate(&[(
        "tsconfig.json",
        r#"{"compilerOptions":{"baseUrl":".","moduleResolution":"Node10","module":"ESNext","paths":{"@js":["./src/a.js"],"@dir":["./src/"]}}}"#,
    )]);
    assert!(c.supported(), "{:?}", c.diagnostics());
    assert_eq!(c.base_url(), Some(""));
    let r = ConfiguredResolver::new(
        &c,
        [p("main.ts"), p("src/a.ts"), p("src.ts"), p("src/index.ts")],
    )
    .unwrap();
    assert!(
        matches!(r.lookup(&p("main.ts"),"@js").outcome,ConfigOutcome::Found{path,..} if path==p("src/a.ts"))
    );
    assert!(
        matches!(r.lookup(&p("main.ts"),"@dir").outcome,ConfigOutcome::Found{path,..} if path==p("src/index.ts"))
    );
    assert!(
        matches!(r.lookup(&p("main.ts"),"src/a").outcome,ConfigOutcome::Found{path,..} if path==p("src/a.ts"))
    );
}
#[test]
fn deep_json_and_aggregate_or_path_inventory_exhaustion_are_bounded() {
    let nested = format!(
        "{{\"compilerOptions\":{{\"lib\":{}0{}}}}}",
        "[".repeat(200),
        "]".repeat(200)
    );
    let c = evaluate(&[("tsconfig.json", &nested)]);
    assert!(has_code(&c, "malformed_config"));
    assert!(
        CapturedConfigs::new((0..17).map(|i| ConfigInput {
            path: p(&format!("{i}.json")),
            bytes: vec![b' '; 1024 * 1024]
        }))
        .is_err()
    );
    assert!(
        CapturedConfigs::new([ConfigInput {
            path: p(&"a".repeat(4097)),
            bytes: vec![]
        }])
        .is_err()
    );
    assert!(captured(&[]).evaluate(&p(&"a".repeat(4097))).is_err());
}
#[test]
fn config_directory_macros_and_invalid_utf8_cannot_be_mistaken_for_literal_paths() {
    let c = evaluate(&[(
        "tsconfig.json",
        r#"{"compilerOptions":{"baseUrl":"${configDir}/src"}}"#,
    )]);
    assert!(has_code(&c, "invalid_base_url"));
    let c = evaluate(&[(
        "tsconfig.json",
        r#"{"compilerOptions":{"paths":{"@/*":["./${configDir}/*"]}}}"#,
    )]);
    assert!(has_code(&c, "invalid_paths_mapping"));
    let c = CapturedConfigs::new([ConfigInput {
        path: p("tsconfig.json"),
        bytes: vec![255],
    }])
    .unwrap()
    .evaluate(&p("tsconfig.json"))
    .unwrap();
    assert!(has_code(&c, "malformed_config"));
}
