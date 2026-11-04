//! Generation-bound derived analysis. Not semantic proof or permission for early cutoff.
use ripple_core::{
    canonical::CanonicalState,
    identity::{RepoPath, SymbolId},
    index::{IndexSnapshot, InputRole},
    syntax::VersionedHash,
};
use ripple_resolve::reexports::ModuleAnalysis;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const CACHE_POLICY: &str = "ripple-analysis-cache-v1";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolFingerprint {
    pub identity: SymbolId,
    /// Missing/incomplete hashes never establish an unchanged external contract.
    pub external: Option<VersionedHash>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileFingerprint {
    pub path: RepoPath,
    pub content: VersionedHash,
    pub symbols: Vec<SymbolFingerprint>,
    pub module: VersionedHash,
    pub unknown_exports: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisCache {
    pub policy: String,
    pub files: Vec<FileFingerprint>,
    /// Includes export origins, forwarding witnesses, config probes and failed lookup attempts.
    pub modules: ModuleAnalysis,
    pub discovery_diagnostics: Vec<String>,
    pub missing_inputs: BTreeSet<RepoPath>,
}
impl AnalysisCache {
    pub fn new(
        snapshot: &IndexSnapshot,
        modules: ModuleAnalysis,
        mut discovery_diagnostics: Vec<String>,
        missing_inputs: BTreeSet<RepoPath>,
    ) -> Result<Self, String> {
        if modules.version != ripple_resolve::reexports::MODULES_VERSION
            || modules.config_version != ripple_resolve::config::CONFIG_VERSION
        {
            return Err("analysis cache module/config policy mismatch".into());
        }
        let inputs = snapshot
            .inputs()
            .entries()
            .iter()
            .map(|i| (&i.path, i))
            .collect::<BTreeMap<_, _>>();
        let surface_map = modules
            .surfaces
            .iter()
            .map(|s| (&s.file, s))
            .collect::<BTreeMap<_, _>>();
        if missing_inputs.iter().any(|p| inputs.contains_key(p)) {
            return Err("analysis cache marks a captured input missing".into());
        }
        if modules.project != snapshot.metadata().project
            || Some(&modules.config_hash) != snapshot.metadata().effective_config_hash.as_ref()
        {
            return Err("analysis cache project/config mismatch".into());
        }
        let paths = snapshot
            .files()
            .iter()
            .map(|f| f.path.clone())
            .collect::<BTreeSet<_>>();
        let surfaces = modules
            .surfaces
            .iter()
            .map(|s| s.file.clone())
            .collect::<BTreeSet<_>>();
        if surfaces != paths || modules.surfaces.len() != paths.len() {
            return Err("analysis cache module membership mismatch".into());
        }
        let mut files = Vec::new();
        for file in snapshot.files() {
            let input = inputs
                .get(&file.path)
                .filter(|i| i.role == InputRole::Source)
                .ok_or("analysis cache missing source digest")?;
            let surface = surface_map
                .get(&file.path)
                .ok_or("analysis cache missing module surface")?;
            let mut symbols = file
                .symbols
                .iter()
                .map(|s| SymbolFingerprint {
                    identity: s.identity.clone(),
                    external: s.signature_hash.clone(),
                })
                .collect::<Vec<_>>();
            symbols.sort_by(|a, b| a.identity.cmp(&b.identity));
            files.push(FileFingerprint {
                path: file.path.clone(),
                content: input.content_hash.clone(),
                symbols,
                module: surface.fingerprint.clone(),
                unknown_exports: surface.unknown_star_surface,
            });
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        discovery_diagnostics.sort();
        discovery_diagnostics.dedup();
        Ok(Self {
            policy: CACHE_POLICY.into(),
            files,
            modules,
            discovery_diagnostics,
            missing_inputs,
        })
    }
    pub fn validate(&self, snapshot: &IndexSnapshot) -> Result<(), String> {
        let expected = Self::new(
            snapshot,
            self.modules.clone(),
            self.discovery_diagnostics.clone(),
            self.missing_inputs.clone(),
        )?;
        if self != &expected {
            return Err("analysis cache fingerprint/policy mismatch".into());
        }
        Ok(())
    }
}
/// Full equality includes resolver/config/discovery evidence, while snapshot equality
/// already excludes storage IDs, origins, timestamps and generation counters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CanonicalAnalysis {
    policy: &'static str,
    snapshot: CanonicalState,
    modules: serde_json::Value,
    discovery_diagnostics: Vec<String>,
    missing_inputs: BTreeSet<RepoPath>,
}
impl CanonicalAnalysis {
    pub fn new(snapshot: &IndexSnapshot, cache: &AnalysisCache) -> Result<Self, String> {
        cache.validate(snapshot)?;
        let mut modules = serde_json::to_value(&cache.modules).map_err(|e| e.to_string())?;
        // Only record collections are sets. Lookup routes/attempts, aliases and
        // witness hops keep their precedence/path order, including inside records.
        for field in [
            "config_inputs",
            "config_diagnostics",
            "exports",
            "imports",
            "dependencies",
            "transfers",
            "surfaces",
        ] {
            let array = modules[field]
                .as_array_mut()
                .ok_or("missing module record collection")?;
            let mut keyed = array
                .drain(..)
                .map(|v| serde_json::to_string(&v).map(|k| (k, v)))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            keyed.sort_by(|a, b| a.0.cmp(&b.0));
            array.extend(keyed.into_iter().map(|(_, v)| v));
        }
        Ok(Self {
            policy: CACHE_POLICY,
            snapshot: CanonicalState::from_snapshot(snapshot),
            modules,
            discovery_diagnostics: cache.discovery_diagnostics.clone(),
            missing_inputs: cache.missing_inputs.clone(),
        })
    }
}
