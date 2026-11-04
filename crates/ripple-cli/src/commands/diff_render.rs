//! Terminal-safe witness and traversal summaries; reporting never prunes traversal.
use crate::output::terminal_json;
use ripple_core::{graph::NodeId, results::Witness};
use ripple_impact::traversal::{HUB_THRESHOLD, TraversalSummary};
use std::fmt::{self, Write};
pub(super) fn label(id: &NodeId) -> String {
    match id {
        NodeId::Symbol(s) => format!(
            "{}::{}",
            s.path(),
            s.qualified_name().components().join(".")
        ),
        _ => id.path().to_string(),
    }
}
pub(super) fn witness(out: &mut String, witness: &Witness) -> Result<(), fmt::Error> {
    writeln!(
        out,
        "  {:?} {:?} witness: {} -> {} distance={} confidence={:?}",
        witness.revision,
        witness.evidence_kind,
        terminal_json(&label(&witness.seed))?,
        terminal_json(&label(&witness.target))?,
        witness.distance(),
        witness.confidence()
    )?;
    for hop in &witness.hops {
        writeln!(
            out,
            "    <- [{:?} {:?}] {} at {}:{} ({})",
            hop.confidence,
            hop.kind,
            terminal_json(&label(&hop.source))?,
            terminal_json(&hop.provenance.file)?,
            hop.provenance.span.start().line,
            terminal_json(&hop.resolution_rule)?
        )?;
    }
    Ok(())
}
pub(super) fn summaries(
    out: &mut String,
    summaries: &[TraversalSummary],
    limit: usize,
) -> Result<(), fmt::Error> {
    for summary in summaries.iter().take(limit) {
        writeln!(
            out,
            "  {:?} {:?} seed {}: {} visited nodes; nodes by depth {}",
            summary.revision,
            summary.evidence_kind,
            terminal_json(&label(&summary.seed))?,
            summary.visited_nodes,
            terminal_json(&summary.nodes_by_depth)?
        )?;
        writeln!(
            out,
            "    High fan-out nodes (threshold {HUB_THRESHOLD} distinct direct dependents): {}",
            summary.hubs.len()
        )?;
        for hub in summary.hubs.iter().take(limit) {
            writeln!(
                out,
                "    {}: {} direct dependents",
                terminal_json(&label(&hub.node))?,
                hub.direct_dependents
            )?;
        }
        writeln!(
            out,
            "    Hubs omitted from human display: {}",
            summary.hubs.len().saturating_sub(limit)
        )?;
    }
    writeln!(
        out,
        "  Traversal summaries omitted from human display: {}",
        summaries.len().saturating_sub(limit)
    )
}
