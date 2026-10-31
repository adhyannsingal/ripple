use super::CommandError;
use crate::output::{InspectData, Report, ReportDiagnostic};
use ripple_core::{
    identity::{ProjectKey, RepoPath},
    syntax::{ParseStatus, Severity},
};
use ripple_parse::{
    extract,
    hashes::EXTRACTOR_VERSION,
    parser::parse,
    scanner::{Worktree, classify},
};
use std::path::Path;

/// A CLI path is portable and relative to the explicit project root, not to the host.
pub fn file_argument(value: &str) -> Result<RepoPath, String> {
    let path = RepoPath::new(value).map_err(|e| e.to_string())?;
    if classify(&path).is_none() {
        return Err("inspect requires a .ts or .tsx file relative to --root".into());
    }
    Ok(path)
}

pub fn run(root: &Path, path: RepoPath) -> Result<Report<InspectData>, CommandError> {
    let worktree =
        Worktree::new(root).map_err(|e| CommandError::new("discovery_failed", e.to_string()))?;
    let discovery = worktree
        .discover()
        .map_err(|e| CommandError::new("discovery_failed", e.to_string()))?;
    let file=discovery.files.iter().find(|file|file.path==path).ok_or_else(||CommandError::new(
        "file_not_discovered",format!("{path:?} was not discovered under --root; check that it exists and is not ignored, excluded or a symlink")))?;
    let source = discovery.sources.get(&path).ok_or_else(|| {
        CommandError::new("capture_failed", "Discovered file has no captured source")
    })?;
    // Analyze these captured bytes only; never reopen the selected path after discovery.
    let parsed = parse(path.clone(), source.text().as_bytes())
        .map_err(|e| CommandError::new("parse_failed", e.to_string()))?;
    let facts = extract(&parsed, ProjectKey::single_project())
        .map_err(|e| CommandError::new("extraction_failed", e.to_string()))?;
    let mut diagnostics = discovery
        .diagnostics
        .into_iter()
        .map(|d| ReportDiagnostic {
            path: d.path,
            code: d.code.into(),
            severity: Severity::Warning,
            message: d.message,
            span: None,
        })
        .collect::<Vec<_>>();
    diagnostics.extend(facts.diagnostics.into_iter().map(|d| ReportDiagnostic {
        path: path.clone(),
        code: d.code,
        severity: d.severity,
        message: d.message,
        span: d.span,
    }));
    diagnostics.sort();
    diagnostics.dedup();
    let incomplete_analysis = facts.parse_status != ParseStatus::Clean || !diagnostics.is_empty();
    Ok(Report {
        format_version: 1,
        command: "inspect",
        incomplete_analysis,
        diagnostics,
        data: InspectData {
            analysis_stage: "syntax",
            resolution_available: false,
            analyzer_version: EXTRACTOR_VERSION,
            path: facts.path,
            language: facts.language,
            parse_status: facts.parse_status,
            is_test: file.is_test,
            symbols: facts.symbols,
            imports: facts.imports,
            exports: facts.exports,
            scopes: facts.scopes,
            references: facts.references,
        },
    })
}
