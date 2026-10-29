mod support;

use support::ripple;

#[test]
fn help_identifies_bootstrap_capabilities() {
    let output = ripple(&["--help"]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Usage: ripple"));
    assert!(text.contains("analysis commands are not implemented yet"));
    assert!(output.stderr.is_empty());
}

#[test]
fn version_matches_the_package() {
    let output = ripple(&["--version"]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        concat!("ripple ", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn unimplemented_analysis_does_not_claim_success() {
    for args in [&["index", "."][..], &["status"][..], &[][..]] {
        let output = ripple(args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}
