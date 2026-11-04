//! CLI composition over pinned Git trees; never loads or publishes the worktree index.
use super::{CommandError, diff_render, persistent};
use crate::output::{Format, terminal_json};
use ripple_core::{results::QueryLimits, revision::ComparisonSpec};
use ripple_impact::{
    changes::ChangeKind,
    revision_impact::{self, RevisionImpact, RevisionView},
};
use ripple_index::snapshots::{self, RevisionSnapshot};
use std::{
    fmt::{self, Write},
    path::Path,
};

fn view(snapshot: &RevisionSnapshot) -> RevisionView<'_> {
    RevisionView {
        index: snapshot.index(),
        sources: snapshot.sources(),
        global_inputs: snapshot.global_inputs(),
        existing_paths: snapshot.existing_paths(),
        incomplete: snapshot.incomplete(),
        diagnostics: snapshot.diagnostics(),
    }
}
fn human(result: &RevisionImpact, limit: usize, incomplete: bool) -> Result<String, fmt::Error> {
    let changes = result.changes();
    let mut out = format!(
        "Diff: {}..{}\nChanged files: {}\nClassified symbols: {} (including unchanged hunk owners)\nGlobal input invalidation: {}\nIncomplete analysis: {}\n",
        changes.revisions().base.as_str(),
        changes.revisions().head.as_str(),
        changes.files().len(),
        changes.symbols().len(),
        changes.global_invalidation(),
        incomplete
    );
    for symbol in changes.symbols().iter().take(limit) {
        writeln!(
            out,
            "{} {}",
            terminal_json(&format!(
                "{}::{}",
                symbol.identity.path(),
                symbol.identity.qualified_name().components().join(".")
            ))?,
            terminal_json(&symbol.kind)?
        )?;
    }
    writeln!(
        out,
        "Classifications omitted from human display: {}",
        changes.symbols().len().saturating_sub(limit)
    )?;
    for file in changes.files().iter().take(limit) {
        writeln!(
            out,
            "File {:?}: {} -> {}; fallback reasons: {}",
            file.change.kind,
            terminal_json(&file.change.old_path)?,
            terminal_json(&file.change.new_path)?,
            terminal_json(&file.reasons)?
        )?;
    }
    writeln!(
        out,
        "File records omitted from human display: {}",
        changes.files().len().saturating_sub(limit)
    )?;
    let tests = result.tests();
    writeln!(
        out,
        "Symbol-backed targets: {}\nModule-fallback targets: {}\nCombined targets: {}\nDisplayed targets: {} / {}\nPotentially related test files: {} symbol-backed, {} fallback-only, {} combined\nHead indexed test files: {}\nSelected tests: {} active at head, {} absent at head, {} present but unindexed at head\nTest sets are potentially related; they are not a sufficiency guarantee.",
        result.evidence().symbol_backed_nodes().len(),
        result.evidence().module_fallback_nodes().len(),
        result.rows().len(),
        result.displayed().len(),
        result.rows().len(),
        tests.symbol_backed.len(),
        tests.fallback_only.len(),
        tests.combined.len(),
        tests.total_head_indexed_test_files,
        tests.active_head.len(),
        tests.base_only.len(),
        tests.present_unindexed_head.len()
    )?;
    for (name, side) in [("base", result.base()), ("head", result.head())] {
        writeln!(
            out,
            "{name}: {} seeds, {} graph-wide unresolved records, {} depth boundary nodes, {} unindexed changes, {} global policy file candidates",
            side.seeds.len(),
            side.query.impact.unresolved.len(),
            side.query.impact.truncation.depth_boundary_nodes.len(),
            side.unindexed_changes.len(),
            side.global_fallback.len()
        )?;
        for diagnostic in side.snapshot_diagnostics.iter().take(limit) {
            writeln!(out, "  diagnostic: {}", terminal_json(diagnostic)?)?;
        }
        writeln!(
            out,
            "  Snapshot diagnostics: {} ({} omitted from human display)",
            side.snapshot_diagnostics.len(),
            side.snapshot_diagnostics.len().saturating_sub(limit)
        )?;
        diff_render::summaries(&mut out, &side.query.summaries, limit)?;
    }
    for row in result.displayed() {
        writeln!(
            out,
            "Target {} distance={} confidence={:?}",
            terminal_json(&diff_render::label(&row.target))?,
            row.distance,
            row.confidence
        )?;
        for witness in result
            .evidence()
            .symbol_backed()
            .iter()
            .chain(result.evidence().module_fallback())
            .filter(|w| w.target == row.target)
        {
            diff_render::witness(&mut out, witness)?;
        }
    }
    writeln!(
        out,
        "Target rows omitted from human display: {}. JSON retains complete evidence, classifications and test sets within the requested depth.",
        result.display_omitted()
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
    let result = revision_impact::analyze(
        &trees.changes,
        view(&base),
        view(&head),
        QueryLimits {
            depth,
            display_limit: limit,
        },
    )
    .map_err(|e| CommandError::new("revision_analysis_failed", e.to_string()))?;
    let diagnostics = trees
        .diagnostics
        .iter()
        .map(|d| serde_json::json!({"path":d.path,"reason":d.reason}))
        .collect::<Vec<_>>();
    let incomplete = result.incomplete() || !diagnostics.is_empty();
    let mut human = human(&result, limit, incomplete)
        .map_err(|e| CommandError::new("output_failed", e.to_string()))?;
    for d in trees.diagnostics.iter().take(limit) {
        writeln!(
            human,
            "Git diagnostic: {} {}",
            terminal_json(&d.path)
                .map_err(|e| CommandError::new("output_failed", e.to_string()))?,
            d.reason
        )
        .map_err(|e| CommandError::new("output_failed", e.to_string()))?;
    }
    writeln!(
        human,
        "Git diagnostics: {} ({} omitted from human display)",
        diagnostics.len(),
        diagnostics.len().saturating_sub(limit)
    )
    .map_err(|e| CommandError::new("output_failed", e.to_string()))?;
    persistent::emit(
        "diff",
        &serde_json::json!({
            "comparison": {"requested":comparison, "endpoints":trees.changes.revisions(), "semantics":"explicit_tree_endpoints"},
            "versions":base.index().metadata().versions,
            "effective_config_hashes":{"base":base.index().metadata().effective_config_hash,"head":head.index().metadata().effective_config_hash},
            "git_diagnostics":diagnostics,
            "counts":{"changed_files":result.changes().files().len(),"classified_symbols":result.changes().symbols().len(),"changed_symbols":result.changes().symbols().iter().filter(|s| s.kind != ChangeKind::Unchanged).count()},
            "result":result
        }),
        incomplete,
        format,
        human,
    )
}
