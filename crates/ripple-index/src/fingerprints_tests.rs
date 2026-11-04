use crate::{
    build,
    fingerprints::CanonicalAnalysis,
    storage::{LoadedGeneration, Store},
};
use ripple_core::{
    canonical::CanonicalState,
    graph::{EdgeKind, GraphSnapshot},
    index::SnapshotData,
    revision::SnapshotOrigin,
};
use std::{fs, path::Path};
fn origin(_: &Path) -> Result<SnapshotOrigin, String> {
    Ok(SnapshotOrigin::Worktree {
        head: None,
        dirty: true,
    })
}
fn fixture() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    for (p, bytes) in [
        ("lib.ts", include_str!("../../../fixtures/canonical/lib.ts")),
        (
            "app.test.ts",
            include_str!("../../../fixtures/canonical/app.test.ts"),
        ),
    ] {
        fs::write(d.path().join(p), bytes).unwrap();
    }
    build::cold_index(d.path(), origin).unwrap();
    d
}
fn load(d: &Path) -> LoadedGeneration {
    Store::open(d.join(".ripple"), build::versions())
        .unwrap()
        .load()
        .unwrap()
        .unwrap()
}
fn canonical(g: &LoadedGeneration) -> CanonicalAnalysis {
    CanonicalAnalysis::new(&g.snapshot, g.analysis.as_ref().unwrap()).unwrap()
}
#[test]
fn canonical_expected_records_and_atomic_cache_roundtrip() {
    let d = fixture();
    let a = load(d.path());
    let cache = a.analysis.as_ref().unwrap();
    let actual = serde_json::json!({"paths":cache.files.iter().map(|f|f.path.as_str()).collect::<Vec<_>>(),"symbols":cache.files.iter().flat_map(|f|f.symbols.iter().map(|s|s.identity.qualified_name().display_name())).collect::<Vec<_>>(),"lookups":cache.modules.dependencies.iter().map(|d|d.lookup.specifier.as_str()).collect::<Vec<_>>(),"missing_candidates":cache.modules.dependencies.iter().filter(|d|d.lookup.specifier=="./missing").flat_map(|d|d.lookup.attempts.iter().map(|a|a.path.as_str())).collect::<Vec<_>>()});
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/canonical/expected.json")).unwrap();
    assert_eq!(actual, expected);
    let b = load(d.path());
    assert_eq!(canonical(&a), canonical(&b));
    assert_eq!(a.analysis, b.analysis);
    build::cold_index(d.path(), origin).unwrap();
    let c = load(d.path());
    assert_ne!(
        a.snapshot.metadata().generation,
        c.snapshot.metadata().generation
    );
    assert_eq!(canonical(&a), canonical(&c));
    let mut reordered = cache.clone();
    reordered.modules.dependencies.reverse();
    reordered.modules.exports.reverse();
    reordered.modules.surfaces.reverse();
    assert_eq!(
        canonical(&a),
        CanonicalAnalysis::new(&a.snapshot, &reordered).unwrap()
    );
}
#[test]
fn body_edits_change_content_but_not_supported_external_fingerprints() {
    let d = fixture();
    let a = load(d.path());
    let text = fs::read_to_string(d.path().join("lib.ts"))
        .unwrap()
        .replace("return 1", "return 9");
    fs::write(d.path().join("lib.ts"), text).unwrap();
    build::cold_index(d.path(), origin).unwrap();
    let b = load(d.path());
    let old = &a.analysis.as_ref().unwrap().files[1];
    let new = &b.analysis.as_ref().unwrap().files[1];
    assert_ne!(old.content, new.content);
    assert_eq!(old.symbols, new.symbols);
    assert_eq!(old.module, new.module);
    assert_ne!(canonical(&a), canonical(&b));
}
#[test]
fn equality_detects_stale_full_analysis_and_preserves_ordered_lookup_evidence() {
    let d = fixture();
    let g = load(d.path());
    let baseline = canonical(&g);
    let cache = g.analysis.as_ref().unwrap();
    let mut changed = cache.clone();
    changed
        .modules
        .dependencies
        .iter_mut()
        .find(|d| d.lookup.specifier == "./missing")
        .unwrap()
        .lookup
        .attempts
        .reverse();
    assert_ne!(
        baseline,
        CanonicalAnalysis::new(&g.snapshot, &changed).unwrap()
    );
    let mut changed = cache.clone();
    changed
        .discovery_diagnostics
        .push("ignored construct".into());
    assert_ne!(
        baseline,
        CanonicalAnalysis::new(&g.snapshot, &changed).unwrap()
    );
    let mut changed = cache.clone();
    changed
        .modules
        .config_diagnostics
        .push(ripple_resolve::config::ConfigDiagnostic {
            file: ripple_core::identity::RepoPath::new("tsconfig.json").unwrap(),
            code: "unsupported".into(),
            detail: "unknown mode".into(),
        });
    assert_ne!(
        baseline,
        CanonicalAnalysis::new(&g.snapshot, &changed).unwrap()
    );
    for kind in 0..6 {
        let mut data =
            serde_json::from_value::<SnapshotData>(serde_json::to_value(&g.snapshot).unwrap())
                .unwrap();
        match kind {
            0 => {
                let edges = data
                    .graph
                    .edges()
                    .iter()
                    .filter(|e| e.kind != EdgeKind::Calls)
                    .cloned()
                    .collect();
                data.graph = GraphSnapshot::new(
                    data.graph.nodes().to_vec(),
                    edges,
                    data.graph.unresolved().to_vec(),
                )
                .unwrap();
            }
            1 => {
                data.graph = GraphSnapshot::new(
                    data.graph
                        .nodes()
                        .iter()
                        .filter(|n| !matches!(n.id, ripple_core::graph::NodeId::Opaque { .. }))
                        .cloned()
                        .collect(),
                    data.graph.edges().to_vec(),
                    vec![],
                )
                .unwrap();
            }
            2 => {
                let mut edges = data.graph.edges().to_vec();
                edges
                    .iter_mut()
                    .find(|e| e.kind == EdgeKind::Calls)
                    .unwrap()
                    .confidence = ripple_core::graph::Confidence::Candidate;
                data.graph = GraphSnapshot::new(
                    data.graph.nodes().to_vec(),
                    edges,
                    data.graph.unresolved().to_vec(),
                )
                .unwrap();
            }
            3 => {
                data.files[0]
                    .diagnostics
                    .push(ripple_core::syntax::Diagnostic {
                        code: "stale".into(),
                        severity: ripple_core::syntax::Severity::Warning,
                        message: "stale fact".into(),
                        span: None,
                    });
            }
            4 => {
                data.metadata.effective_config_hash.as_mut().unwrap().digest = "0".repeat(64);
            }
            _ => {
                let mut edges = data.graph.edges().to_vec();
                edges
                    .iter_mut()
                    .find(|e| e.kind == EdgeKind::Calls)
                    .unwrap()
                    .resolution_rule = "different_rule".into();
                data.graph = GraphSnapshot::new(
                    data.graph.nodes().to_vec(),
                    edges,
                    data.graph.unresolved().to_vec(),
                )
                .unwrap();
            }
        }
        let modified = data.validate(&g.sources).unwrap();
        let old = CanonicalState::from_snapshot(&g.snapshot);
        let new = CanonicalState::from_snapshot(&modified);
        assert_ne!(old, new, "fault {kind}");
        assert!(!old.differences(&new).is_empty());
    }
}
#[test]
fn cache_rejection_and_corruption_never_publish_or_silently_reuse() {
    let d = fixture();
    let g = load(d.path());
    let mut cache = g.analysis.clone().unwrap();
    cache.files[0].content.digest = "0".repeat(64);
    let mut store = Store::open(d.path().join(".ripple"), build::versions()).unwrap();
    let mut data: SnapshotData =
        serde_json::from_value::<SnapshotData>(serde_json::to_value(&g.snapshot).unwrap()).unwrap();
    data.metadata.generation = 2;
    let snapshot = data.validate(&g.sources).unwrap();
    assert!(
        store
            .publish_analysis(&snapshot, &g.inputs, Some(&cache))
            .is_err()
    );
    assert_eq!(
        store
            .load()
            .unwrap()
            .unwrap()
            .snapshot
            .metadata()
            .generation,
        1
    );
    drop(store);
    let c = rusqlite::Connection::open(d.path().join(".ripple/index.db")).unwrap();
    c.execute("DELETE FROM analysis_artifacts", []).unwrap();
    drop(c);
    assert!(
        Store::open(d.path().join(".ripple"), build::versions())
            .unwrap()
            .load()
            .is_err()
    );
    let d = fixture();
    let c = rusqlite::Connection::open(d.path().join(".ripple/index.db")).unwrap();
    c.pragma_update(None, "user_version", 1).unwrap();
    drop(c);
    assert!(Store::open(d.path().join(".ripple"), build::versions()).is_err());
}
