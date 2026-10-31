mod support;
use support::ripple;

#[test]
fn help_identifies_implemented_and_future_capabilities() {
    let output = ripple(&["--help"]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Usage: ripple"));
    assert!(text.contains("inspect"));
    assert!(text.contains("not implemented yet"));
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
fn registered_future_commands_fail_without_success_output() {
    for args in [
        &["index", "."][..],
        &["impact", "caller"][..],
        &["diff", "main..HEAD"][..],
        &["test", "main..HEAD"][..],
        &["status"][..],
    ] {
        let output = ripple(args);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("command_not_implemented"));
    }
}
#[test]
fn missing_unknown_commands_and_bad_usage_exit_two() {
    for args in [
        &[][..],
        &["unknown"][..],
        &["inspect"][..],
        &["inspect", "file.ts", "--format", "xml"][..],
    ] {
        let output = ripple(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}
#[test]
fn command_help_is_available_for_inspect_and_future_placeholders() {
    for name in ["inspect", "index", "impact", "diff", "test", "status"] {
        let output = ripple(&[name, "--help"]);
        assert_eq!(output.status.code(), Some(0), "{name}");
        assert!(output.stderr.is_empty());
        assert!(String::from_utf8_lossy(&output.stdout).contains("Usage: ripple"));
    }
}
