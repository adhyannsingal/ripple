//! Semantic projection for equality; operational provenance/freshness is checked separately.
use crate::{
    graph::GraphSnapshot,
    identity::ProjectKey,
    index::{IndexError, IndexSnapshot, IndexVersions, InputManifest},
    syntax::{FileFacts, VersionedHash},
};
use serde::{Deserialize, Serialize};

pub const CANONICAL_FORMAT_VERSION: u32 = 1;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawCanonicalState")]
pub struct CanonicalState {
    format_version: u32,
    project: ProjectKey,
    versions: IndexVersions,
    effective_config_hash: Option<VersionedHash>,
    inputs: InputManifest,
    files: Vec<FileFacts>,
    graph: GraphSnapshot,
}
#[derive(Deserialize)]
struct RawCanonicalState {
    format_version: u32,
    project: ProjectKey,
    versions: IndexVersions,
    effective_config_hash: Option<VersionedHash>,
    inputs: InputManifest,
    files: Vec<FileFacts>,
    graph: GraphSnapshot,
}
impl TryFrom<RawCanonicalState> for CanonicalState {
    type Error = IndexError;
    fn try_from(raw: RawCanonicalState) -> Result<Self, Self::Error> {
        if raw.format_version != CANONICAL_FORMAT_VERSION {
            return Err(IndexError("unsupported canonical comparison format"));
        }
        raw.versions.validate()?;
        Ok(Self {
            format_version: raw.format_version,
            project: raw.project,
            versions: raw.versions,
            effective_config_hash: raw.effective_config_hash,
            inputs: raw.inputs,
            files: normalize_files(raw.files),
            graph: raw.graph,
        })
    }
}
fn normalize_files(mut files: Vec<FileFacts>) -> Vec<FileFacts> {
    for file in &mut files {
        file.symbols.sort();
        for import in &mut file.imports {
            import.bindings.sort();
        }
        file.imports.sort();
        file.exports.sort();
        for scope in &mut file.scopes {
            scope.bindings.sort();
        }
        file.scopes.sort();
        file.references.sort();
        file.diagnostics.sort();
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}
impl CanonicalState {
    pub fn from_snapshot(snapshot: &IndexSnapshot) -> Self {
        let files = normalize_files(snapshot.files().to_vec());
        Self {
            format_version: CANONICAL_FORMAT_VERSION,
            project: snapshot.metadata().project.clone(),
            versions: snapshot.metadata().versions.clone(),
            effective_config_hash: snapshot.metadata().effective_config_hash.clone(),
            inputs: snapshot.inputs().clone(),
            files,
            graph: snapshot.graph().clone(),
        }
    }
}
