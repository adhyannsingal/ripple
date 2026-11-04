use super::{CommandError, persistent};
use crate::output::{Format, terminal_json};
use ripple_core::{
    graph::NodeId,
    results::{QueryLimits, RevisionSide},
};
use ripple_impact::graph::DependencyGraph;
use std::{fmt::Write, path::Path};
fn label(id: &NodeId) -> String {
    match id {
        NodeId::Symbol(s) => format!(
            "{}::{}",
            s.path(),
            s.qualified_name().components().join(".")
        ),
        _ => id.path().to_string(),
    }
}
pub fn run(
    root: &Path,
    query: &str,
    depth: Option<usize>,
    limit: usize,
    format: Format,
) -> Result<(), CommandError> {
    let current = ripple_index::build::load_current(root, ripple_git::worktree::observe)
        .map_err(persistent::error)?;
    let snapshot = &current.loaded.snapshot;
    let matches = snapshot
        .graph()
        .nodes()
        .iter()
        .filter(|n| match &n.id {
            NodeId::Symbol(s) => {
                query == s.qualified_name().components().join(".")
                    || query == label(&n.id)
                    || query == s.semantic_key()
            }
            _ => false,
        })
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Err(CommandError::new(
            "symbol_not_found",
            "no matching indexed symbol",
        ));
    }
    if matches.len() != 1 {
        return Err(CommandError::new(
            "ambiguous_symbol",
            format!(
                "choose a qualified symbol or semantic key: {:?}",
                matches
                    .iter()
                    .map(|n| (label(&n.id), n.id.semantic_key()))
                    .collect::<Vec<_>>()
            ),
        ));
    }
    let result = DependencyGraph::new(snapshot.graph())
        .query(
            &[matches[0].id.clone()],
            RevisionSide::Worktree,
            QueryLimits {
                depth,
                display_limit: limit,
            },
        )
        .map_err(|e| persistent::error(e.to_string()))?;
    let mut human = format!(
        "Impact: {}\nSymbol-backed targets: {}\nModule-fallback targets: {}\nCombined targets: {}\nPotentially related tests: {} symbol-backed, {} fallback-only, {} combined\nDisplayed: {} / {}\nDepth boundary nodes: {}\n",
        terminal_json(&query).map_err(|e| persistent::error(e.to_string()))?,
        result.impact.evidence.symbol_backed_nodes().len(),
        result.impact.evidence.module_fallback_nodes().len(),
        result.rows.len(),
        result.tests.symbol_backed.len(),
        result.tests.fallback_only.len(),
        result.tests.combined.len(),
        result.displayed().len(),
        result.rows.len(),
        result.impact.truncation.depth_boundary_nodes.len()
    );
    for row in result.displayed() {
        for w in result
            .impact
            .evidence
            .symbol_backed()
            .iter()
            .chain(result.impact.evidence.module_fallback())
            .filter(|w| w.target == row.target)
        {
            writeln!(
                human,
                "{:?} witness: {}",
                w.evidence_kind,
                terminal_json(&label(&w.seed)).map_err(|e| persistent::error(e.to_string()))?
            )
            .map_err(|e| persistent::error(e.to_string()))?;
            for hop in &w.hops {
                writeln!(
                    human,
                    "  <- [{:?} {:?}] {} at {}:{} ({})",
                    hop.confidence,
                    hop.kind,
                    terminal_json(&label(&hop.source))
                        .map_err(|e| persistent::error(e.to_string()))?,
                    terminal_json(&hop.provenance.file)
                        .map_err(|e| persistent::error(e.to_string()))?,
                    hop.provenance.span.start().line,
                    hop.resolution_rule
                )
                .map_err(|e| persistent::error(e.to_string()))?;
            }
        }
    }
    super::diff_render::summaries(&mut human, &result.summaries, limit)
        .map_err(|e| persistent::error(e.to_string()))?;
    let incomplete =
        current.incomplete || !result.impact.truncation.depth_boundary_nodes.is_empty();
    persistent::emit(
        "impact",
        &serde_json::json!({"query":query,"metadata":snapshot.metadata(),"result":result}),
        incomplete,
        format,
        human,
    )
}
