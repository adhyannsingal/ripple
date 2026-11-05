//! Evaluation-only helper: compare persisted incremental and independent cold analysis.
use ripple_core::revision::SnapshotOrigin;
use ripple_index::{build, fingerprints::CanonicalAnalysis, incremental, storage::Store};
use sha2::{Digest, Sha256};
use std::{path::Path, time::Instant};
fn origin(_: &Path) -> Result<SnapshotOrigin, String> {
    // Inputs are materialized immutable blobs in disposable directories, not a user worktree.
    Ok(SnapshotOrigin::Worktree {
        head: None,
        dirty: true,
    })
}
fn canonical(root: &Path) -> Result<CanonicalAnalysis, String> {
    let loaded = Store::open(root.join(".ripple"), build::versions())
        .map_err(|e| e.to_string())?
        .load()
        .map_err(|e| e.to_string())?
        .ok_or("missing evaluated generation")?;
    CanonicalAnalysis::new(
        &loaded.snapshot,
        loaded.analysis.as_ref().ok_or("missing analysis")?,
    )
}
fn run() -> Result<(), String> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("usage: incremental_evaluate INCREMENTAL_ROOT FRESH_COLD_ROOT".into());
    }
    let root = Path::new(&args[0]);
    let cold = Path::new(&args[1]);
    if root.canonicalize().map_err(|e| e.to_string())?
        == cold.canonicalize().map_err(|e| e.to_string())?
        || cold.join(".ripple").exists()
    {
        return Err("cold root must be independent and have no prior index".into());
    }
    let start = Instant::now();
    let updated = incremental::refresh(root, origin)?;
    let incremental_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let baseline = build::cold_index(cold, origin)?;
    let cold_ms = start.elapsed().as_secs_f64() * 1000.0;
    let actual = canonical(root)?;
    let expected = canonical(cold)?;
    if actual != expected {
        return Err("incremental/cold full canonical mismatch".into());
    }
    if actual != canonical(root)? {
        return Err("persisted canonical round-trip mismatch".into());
    }
    let digest = Sha256::digest(serde_json::to_vec(&actual).map_err(|e| e.to_string())?)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    println!(
        "{}",
        serde_json::json!({"canonical_equal":true,"canonical_sha256":digest,
        "versions":build::versions(),"counts":updated.counts,"incomplete":updated.incomplete,
        "discovery_diagnostics":updated.discovery_diagnostics,"config_diagnostics":updated.config_diagnostics,
        "update":updated.update,"cold_update":baseline.update,"incremental_ms":incremental_ms,"cold_ms":cold_ms})
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
