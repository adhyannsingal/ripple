//! Conservative test-impact composition over immutable endpoints; never executes tests.
use super::{CommandError, diff_render, persistent};
use crate::output::{Format, terminal_json};
use ripple_core::{results::QueryLimits, revision::ComparisonSpec};
use ripple_impact::{
    revision_impact::{self, RevisionView},
    tests::{self, TestImpact, TestPresence},
};
use ripple_index::snapshots;
use serde_json::{Value, json};
use std::{
    fmt::{self, Write},
    path::Path,
};

fn view(snapshot: &snapshots::RevisionSnapshot) -> RevisionView<'_> {
    RevisionView {
        index: snapshot.index(),
        sources: snapshot.sources(),
        global_inputs: snapshot.global_inputs(),
        existing_paths: snapshot.existing_paths(),
        incomplete: snapshot.incomplete(),
        diagnostics: snapshot.diagnostics(),
    }
}
fn ratio(selected: usize, total: usize) -> Value {
    json!({"numerator":selected,"denominator":total,"fraction":if total==0 {None} else {Some(selected as f64 / total as f64)}})
}
fn human(selection: &TestImpact, limit: usize) -> Result<String, fmt::Error> {
    let active = &selection.active_head;
    let total = selection.total_head_test_files;
    let mut out = format!(
        "Potentially related test files (conservative combined selection)\nSupported indexed head test files: {total}\nSymbol-backed: {}\nFallback-only: {}\nCombined active at head: {}\nAbsent at head: {}\nPresent but unindexed at head: {}\nIncomplete analysis: {}\nDepth truncated: {}\n",
        active.symbol_backed.len(),
        active.fallback_only.len(),
        active.combined.len(),
        selection.base_only.combined.len(),
        selection.present_unindexed_head.combined.len(),
        selection.completeness.analysis_incomplete,
        selection.completeness.depth_truncated
    );
    for (name, selected) in [
        ("symbol-backed", active.symbol_backed.len()),
        ("combined", active.combined.len()),
    ] {
        if total == 0 {
            writeln!(
                out,
                "Selection ratio {name}: {selected}/{total} (undefined: no supported head tests)"
            )?;
        } else {
            writeln!(
                out,
                "Selection ratio {name}: {selected}/{total} ({:.2}%)",
                100.0 * selected as f64 / total as f64
            )?;
        }
    }
    writeln!(
        out,
        "Ratios describe supported head tests within the requested depth; base-only/unindexed evidence is excluded. Module fallback may equal file-import reachability; no precision improvement is implied."
    )?;
    for (presence, label) in [
        (TestPresence::ActiveHead, "Active head"),
        (TestPresence::AbsentHead, "Absent at head"),
        (
            TestPresence::PresentUnindexedHead,
            "Present but unindexed at head",
        ),
    ] {
        let files = selection
            .files
            .iter()
            .filter(|f| f.presence == presence)
            .collect::<Vec<_>>();
        for file in files.iter().take(limit) {
            let track = if file.symbol_backed.is_empty() {
                "fallback-only"
            } else if file.module_fallback.is_empty() {
                "symbol-backed"
            } else {
                "symbol-backed and module-fallback"
            };
            writeln!(
                out,
                "{label} {} [{track}]",
                terminal_json(file.file.path())?
            )?;
            for witness in file
                .symbol_backed
                .iter()
                .chain(&file.module_fallback)
                .take(limit)
            {
                diff_render::witness(&mut out, witness)?;
            }
            writeln!(
                out,
                "  Witnesses omitted from human display: {}",
                (file.symbol_backed.len() + file.module_fallback.len()).saturating_sub(limit)
            )?;
        }
        writeln!(
            out,
            "{label} paths omitted from human display: {}",
            files.len().saturating_sub(limit)
        )?;
    }
    for reason in selection.completeness.reasons.iter().take(limit) {
        writeln!(out, "Uncertainty: {}", terminal_json(reason)?)?;
    }
    writeln!(
        out,
        "Uncertainty reasons omitted from human display: {}",
        selection.completeness.reasons.len().saturating_sub(limit)
    )?;
    writeln!(
        out,
        "JSON retains complete selected paths and witnesses within requested depth. A depth boundary can omit related tests.\nThese tests are potentially related, not a sufficiency guarantee. No tests were executed."
    )?;
    Ok(out)
}
pub fn run(
    root: &Path,
    comparison: &str,
    depth: Option<usize>,
    limit: usize,
    format: Format,
) -> Result<(), CommandError> {
    let spec = ComparisonSpec::parse(comparison)
        .map_err(|e| CommandError::new("invalid_comparison", e.to_string()))?;
    let repo = ripple_git::revisions::Repository::open(root)
        .map_err(|e| CommandError::new("git_unavailable", e.to_string()))?;
    let trees = repo
        .compare(&spec)
        .map_err(|e| CommandError::new("revision_unavailable", e.to_string()))?;
    let base = snapshots::build(&trees.base)
        .map_err(|e| CommandError::new("revision_build_failed", format!("base: {e}")))?;
    let head = snapshots::build(&trees.head)
        .map_err(|e| CommandError::new("revision_build_failed", format!("head: {e}")))?;
    let analysis = revision_impact::analyze(
        &trees.changes,
        view(&base),
        view(&head),
        QueryLimits {
            depth,
            display_limit: limit,
        },
    )
    .map_err(|e| CommandError::new("revision_analysis_failed", e.to_string()))?;
    let mut selection = tests::select(&analysis);
    let diagnostics = trees
        .diagnostics
        .iter()
        .map(|d| json!({"path":d.path,"reason":d.reason}))
        .collect::<Vec<_>>();
    if !diagnostics.is_empty() {
        selection.completeness.analysis_incomplete = true;
        selection
            .completeness
            .reasons
            .push("Git comparison diagnostics; see git_diagnostics".into());
    }
    let active = &selection.active_head;
    let counts = json!({"total_head_test_files":selection.total_head_test_files,"symbol_backed_selected":active.symbol_backed.len(),"module_fallback_selected":active.module_fallback.len(),"fallback_only_selected":active.fallback_only.len(),"combined_selected":active.combined.len(),"base_only":selection.base_only.combined.len(),"present_unindexed_head":selection.present_unindexed_head.combined.len()});
    let ratios = json!({"symbol_backed":ratio(active.symbol_backed.len(),selection.total_head_test_files),"combined":ratio(active.combined.len(),selection.total_head_test_files),"module_fallback":ratio(active.module_fallback.len(),selection.total_head_test_files)});
    let mut text =
        human(&selection, limit).map_err(|e| CommandError::new("output_failed", e.to_string()))?;
    writeln!(
        text,
        "Comparison: {}..{}\nGit diagnostics: {} (complete in JSON)",
        trees.changes.revisions().base.as_str(),
        trees.changes.revisions().head.as_str(),
        diagnostics.len()
    )
    .map_err(|e| CommandError::new("output_failed", e.to_string()))?;
    for (label, side) in [("base", analysis.base()), ("head", analysis.head())] {
        writeln!(text,"{label}: {} graph-wide unresolved records (context, not reachable paths), {} snapshot diagnostics, {} depth boundary nodes",side.query.impact.unresolved.len(),side.snapshot_diagnostics.len(),side.query.impact.truncation.depth_boundary_nodes.len()).map_err(|e|CommandError::new("output_failed",e.to_string()))?;
    }
    persistent::emit(
        "test",
        &json!({"comparison":{"requested":comparison,"endpoints":trees.changes.revisions(),"semantics":"explicit_tree_endpoints"},"versions":base.index().metadata().versions,"counts":counts,"selection_ratios":ratios,"sufficiency_guaranteed":false,"selection":selection,"git_diagnostics":diagnostics,"analysis":analysis}),
        selection.completeness.analysis_incomplete,
        format,
        text,
    )
}
