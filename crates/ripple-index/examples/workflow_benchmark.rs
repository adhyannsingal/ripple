//! Evaluation-only stage timings; production capture source is compiled verbatim, not reimplemented.
use ripple_index::{build, storage};
#[path = "../src/build_capture.rs"]
mod build_capture;
use ripple_core::{
    graph::NodeId,
    identity::ProjectKey,
    results::{QueryLimits, RevisionSide},
};
use ripple_impact::graph::DependencyGraph;
use std::{path::Path, time::Instant};
fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}
fn run(root: &Path) -> Result<(), String> {
    let canonical_root = std::fs::canonicalize(root).map_err(|e| e.to_string())?;
    let root = canonical_root.as_path();
    let start = Instant::now();
    let loaded = storage::Store::open(root.join(".ripple"), build::versions())
        .map_err(|e| e.to_string())?
        .load()
        .map_err(|e| e.to_string())?
        .ok_or("no index")?;
    let storage_ms = ms(start);
    let start = Instant::now();
    let a = build_capture::capture(root, ripple_git::worktree::observe)?;
    let b = build_capture::capture(root, ripple_git::worktree::observe)?;
    if !a.matches(&b)
        || loaded.snapshot.inputs() != &a.manifest
        || loaded.snapshot.metadata().origin != a.origin
        || loaded.snapshot.metadata().effective_config_hash.as_ref() != Some(a.config.fingerprint())
    {
        return Err("stale benchmark state".into());
    }
    std::hint::black_box(&a.inputs);
    let verification_ms = ms(start);
    let start = Instant::now();
    let modules = ripple_resolve::reexports::resolve_modules(
        &ProjectKey::single_project(),
        &loaded.sources,
        loaded.snapshot.files(),
        &a.config,
    )
    .map_err(|e| e.to_string())?;
    std::hint::black_box(modules);
    let module_policy_ms = ms(start);
    let seed = loaded
        .snapshot
        .graph()
        .nodes()
        .iter()
        .find_map(|n| {
            matches!(&n.id,NodeId::Symbol(s) if s.qualified_name().display_name()=="f0")
                .then_some(n.id.clone())
        })
        .ok_or("no f0")?;
    let start = Instant::now();
    let graph = DependencyGraph::new(loaded.snapshot.graph());
    let adjacency_ms = ms(start);
    let start = Instant::now();
    let query = graph
        .query(
            std::slice::from_ref(&seed),
            RevisionSide::Worktree,
            QueryLimits::default(),
        )
        .map_err(|e| e.to_string())?;
    let traversal_ms = ms(start);
    let start = Instant::now();
    let current = build::load_current(root, ripple_git::worktree::observe)?;
    let production_verified_load_ms = ms(start);
    let reference = DependencyGraph::new(current.loaded.snapshot.graph())
        .query(&[seed], RevisionSide::Worktree, QueryLimits::default())
        .map_err(|e| e.to_string())?;
    if query != reference {
        return Err("phase query/production query mismatch".into());
    }
    println!(
        "{}",
        serde_json::json!({"storage_load_ms":storage_ms,"capture_verification_ms":verification_ms,"module_policy_ms":module_policy_ms,"adjacency_ms":adjacency_ms,"pure_traversal_ms":traversal_ms,"production_verified_load_ms":production_verified_load_ms,"targets":query.rows.len(),"canonical_query_equal":true,"versions":build::versions()})
    );
    Ok(())
}
fn main() {
    let result = std::env::args_os()
        .nth(1)
        .ok_or("root required".into())
        .and_then(|root| run(Path::new(&root)));
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
