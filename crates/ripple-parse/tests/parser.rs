use ripple_core::{
    identity::RepoPath,
    syntax::{Language, ParseStatus},
};
use ripple_parse::parser::*;
use std::time::Duration;
fn path(s: &str) -> RepoPath {
    RepoPath::new(s).unwrap()
}
#[test]
fn correct_grammar_handles_ts_tsx_and_empty_sources() {
    for (name, bytes, language) in [
        (
            "clean.ts",
            include_bytes!("../../../fixtures/parser/clean.ts").as_slice(),
            Language::TypeScript,
        ),
        (
            "view.tsx",
            include_bytes!("../../../fixtures/parser/view.tsx").as_slice(),
            Language::Tsx,
        ),
        ("empty.ts", b"".as_slice(), Language::TypeScript),
    ] {
        let parsed = parse(path(name), bytes).unwrap();
        assert_eq!(parsed.status(), ParseStatus::Clean);
        assert_eq!(parsed.language(), language);
        assert!(parsed.diagnostics().is_empty());
        span(parsed.tree().unwrap().root_node())
            .validate_against(parsed.source().text())
            .unwrap();
    }
    assert_eq!(
        parse(
            path("wrong.ts"),
            include_bytes!("../../../fixtures/parser/view.tsx")
        )
        .unwrap()
        .status(),
        ParseStatus::Recovered
    );
}
#[test]
fn malformed_and_missing_syntax_remains_explicit() {
    for text in [
        include_str!("../../../fixtures/parser/malformed.ts"),
        "function f() {",
        "const = ;",
    ] {
        let parsed = parse(path("bad.ts"), text.as_bytes()).unwrap();
        assert_eq!(parsed.status(), ParseStatus::Recovered);
        assert!(!parsed.diagnostics().is_empty());
        for d in parsed.diagnostics() {
            if let Some(s) = d.span {
                s.validate_against(text).unwrap();
            }
        }
    }
    let missing = parse(path("bad.ts"), b"function f() {").unwrap();
    assert!(
        missing
            .diagnostics()
            .iter()
            .any(|d| d.code == "parse_missing")
    );
}
#[test]
fn all_node_spans_preserve_utf8_crlf_and_eof() {
    let text = "// α\r\nexport function f() { return '😀'; }\r\n";
    let parsed = parse(path("unicode.ts"), text.as_bytes()).unwrap();
    let mut stack = vec![parsed.tree().unwrap().root_node()];
    while let Some(node) = stack.pop() {
        span(node).validate_against(text).unwrap();
        for i in 0..node.child_count() {
            stack.push(node.child(i).unwrap());
        }
    }
    assert_eq!(span(parsed.tree().unwrap().root_node()).end().line, 3);
}
#[test]
fn unsupported_encoding_extension_and_resource_limits_are_visible() {
    assert!(
        parse(path("bad.ts"), &[0xff])
            .unwrap_err()
            .to_string()
            .contains("UTF-8")
    );
    assert!(parse(path("bad.js"), b"x").is_err());
    assert!(
        parse_with_limits(
            path("big.ts"),
            b"123",
            ParseLimits {
                max_bytes: 2,
                ..Default::default()
            }
        )
        .is_err()
    );
    let failed = parse_with_limits(
        path("cancel.ts"),
        b"export {};",
        ParseLimits {
            timeout: Duration::ZERO,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(failed.status(), ParseStatus::Failed);
    assert!(failed.tree().is_none());
    assert_eq!(failed.diagnostics()[0].code, "parse_resource_limit");
}

#[test]
fn excessive_recovery_diagnostics_are_bounded_and_explicit() {
    let text = "const x = ;\n".repeat(1100);
    let parsed = parse(path("many-errors.ts"), text.as_bytes()).unwrap();
    assert_eq!(parsed.status(), ParseStatus::Recovered);
    assert_eq!(parsed.diagnostics().len(), 1025);
    assert_eq!(
        parsed
            .diagnostics()
            .iter()
            .filter(|d| d.code == "parse_diagnostics_truncated")
            .count(),
        1
    );
    for diagnostic in parsed.diagnostics() {
        if let Some(location) = diagnostic.span {
            location.validate_against(&text).unwrap();
        }
    }
}

#[test]
fn progress_callback_cancels_a_nonzero_budget_without_publishing_a_tree() {
    let text = "const x = (".repeat(20_000);
    let parsed = parse_with_limits(
        path("cancel.ts"),
        text.as_bytes(),
        ParseLimits {
            timeout: Duration::from_nanos(1),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(parsed.status(), ParseStatus::Failed);
    assert!(parsed.tree().is_none());
    assert_eq!(parsed.diagnostics()[0].code, "parse_resource_limit");
}
