//! Generation/input contracts, not SQLite storage or live filesystem verification.
use crate::{
    graph::{GraphSnapshot, NodeId},
    identity::{ProjectKey, RepoPath},
    results::{RevisionSide, Witness},
    revision::SnapshotOrigin,
    source::SourceSnapshot,
    syntax::{FileFacts, VersionedHash},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const SCHEMA_VERSION: u32 = 1;
pub const HASH_VERSION: u32 = 1;
pub const MAX_CAPTURE_ATTEMPTS: usize = 3;
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid index contract: {0}")]
pub struct IndexError(pub &'static str);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexVersions {
    pub schema_version: u32,
    pub hash_version: u32,
    pub analyzer_version: String,
}
impl IndexVersions {
    pub fn validate(&self) -> Result<(), IndexError> {
        if self.schema_version == 0
            || self.hash_version == 0
            || self.analyzer_version.trim().is_empty()
            || self.analyzer_version.chars().any(char::is_control)
        {
            return Err(IndexError(
                "schema/hash versions and analyzer compatibility token are required",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputRole {
    Source,
    Configuration,
    IgnorePolicy,
    PackageManifest,
    Lockfile,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CapturedInput {
    pub path: RepoPath,
    pub role: InputRole,
    pub byte_len: u64,
    pub content_hash: VersionedHash,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<CapturedInput>", into = "Vec<CapturedInput>")]
pub struct InputManifest(Vec<CapturedInput>);
impl InputManifest {
    pub fn new(mut inputs: Vec<CapturedInput>) -> Result<Self, IndexError> {
        for input in &inputs {
            input
                .content_hash
                .validate()
                .map_err(|_| IndexError("invalid captured-input digest"))?;
        }
        inputs.sort_by(|a, b| a.path.cmp(&b.path));
        if inputs.windows(2).any(|p| p[0].path == p[1].path) {
            return Err(IndexError("duplicate input path/role"));
        }
        Ok(Self(inputs))
    }
    pub fn entries(&self) -> &[CapturedInput] {
        &self.0
    }
}
impl TryFrom<Vec<CapturedInput>> for InputManifest {
    type Error = IndexError;
    fn try_from(v: Vec<CapturedInput>) -> Result<Self, Self::Error> {
        Self::new(v)
    }
}
impl From<InputManifest> for Vec<CapturedInput> {
    fn from(v: InputManifest) -> Self {
        v.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexMetadata {
    pub project: ProjectKey,
    pub origin: SnapshotOrigin,
    pub versions: IndexVersions,
    pub effective_config_hash: Option<VersionedHash>,
    pub generation: u64,
    pub created_at: String,
}
/// Transport/persistence DTO. Validate with captured sources to obtain IndexSnapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotData {
    pub metadata: IndexMetadata,
    pub inputs: InputManifest,
    pub files: Vec<FileFacts>,
    pub graph: GraphSnapshot,
}
/// Validation establishes captured-view consistency, never live filesystem freshness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct IndexSnapshot(SnapshotData);
impl SnapshotData {
    pub fn validate(mut self, sources: &SourceSnapshot) -> Result<IndexSnapshot, IndexError> {
        self.metadata.versions.validate()?;
        if self.metadata.generation == 0 || self.metadata.created_at.trim().is_empty() {
            return Err(IndexError(
                "complete generation requires number and creation metadata",
            ));
        }
        let version = self.metadata.versions.hash_version;
        if let Some(hash) = &self.metadata.effective_config_hash {
            hash.validate()
                .map_err(|_| IndexError("invalid effective-config digest"))?;
            if hash.version != version {
                return Err(IndexError("effective-config hash version mismatch"));
            }
        }
        if self
            .inputs
            .entries()
            .iter()
            .any(|i| i.content_hash.version != version)
        {
            return Err(IndexError("input hash version mismatch"));
        }
        let source_inputs: BTreeMap<_, _> = self
            .inputs
            .entries()
            .iter()
            .filter(|i| i.role == InputRole::Source)
            .map(|i| (&i.path, i))
            .collect();
        let source_paths: BTreeSet<_> = sources.files().map(|f| f.path()).collect();
        if source_paths != source_inputs.keys().copied().collect() {
            return Err(IndexError(
                "captured source membership differs from input manifest",
            ));
        }
        self.files.sort_by(|a, b| a.path.cmp(&b.path));
        if self.files.windows(2).any(|p| p[0].path == p[1].path)
            || self.files.iter().map(|f| &f.path).collect::<BTreeSet<_>>() != source_paths
        {
            return Err(IndexError(
                "syntax facts must cover each captured source once",
            ));
        }
        let mut symbols = BTreeSet::new();
        for facts in &self.files {
            let source = sources
                .get(&facts.path)
                .ok_or(IndexError("missing captured source"))?;
            if source_inputs[&facts.path].byte_len != source.text().len() as u64 {
                return Err(IndexError("captured byte length mismatch"));
            }
            facts
                .validate(source)
                .map_err(|_| IndexError("syntax facts disagree with captured source"))?;
            for symbol in &facts.symbols {
                if symbol.identity.project() != &self.metadata.project {
                    return Err(IndexError("symbol project mismatch"));
                }
                for hash in [&symbol.signature_hash, &symbol.body_hash]
                    .into_iter()
                    .flatten()
                {
                    if hash.version != version {
                        return Err(IndexError("symbol hash version mismatch"));
                    }
                }
                let node = self
                    .graph
                    .node(&NodeId::Symbol(symbol.identity.clone()))
                    .ok_or(IndexError("syntax symbol absent from graph"))?;
                if node.span != Some(symbol.span) {
                    return Err(IndexError("symbol/graph span mismatch"));
                }
                symbols.insert(symbol.identity.clone());
            }
        }
        let mut graph_files = BTreeSet::new();
        let mut graph_symbols = BTreeSet::new();
        for node in self.graph.nodes() {
            if node.id.project() != &self.metadata.project {
                return Err(IndexError("graph project mismatch"));
            }
            let source = sources
                .get(node.id.path())
                .ok_or(IndexError("graph node outside captured sources"))?;
            if let Some(span) = node.span {
                span.validate_against(source.text())
                    .map_err(|_| IndexError("graph node coordinates disagree with source"))?;
            }
            match &node.id {
                NodeId::File { path, .. } => {
                    graph_files.insert(path);
                }
                NodeId::Symbol(id) => {
                    graph_symbols.insert(id.clone());
                }
                NodeId::Opaque { .. } => {}
            }
        }
        if graph_files != source_paths || graph_symbols != symbols {
            return Err(IndexError("graph and syntax/source membership disagree"));
        }
        for location in self
            .graph
            .edges()
            .iter()
            .map(|e| &e.provenance)
            .chain(self.graph.unresolved().iter().map(|r| &r.provenance))
        {
            let source = sources
                .get(&location.file)
                .ok_or(IndexError("provenance source missing"))?;
            location
                .span
                .validate_against(source.text())
                .map_err(|_| IndexError("provenance coordinates disagree with source"))?;
        }
        Ok(IndexSnapshot(self))
    }
}
impl IndexSnapshot {
    pub fn metadata(&self) -> &IndexMetadata {
        &self.0.metadata
    }
    pub fn inputs(&self) -> &InputManifest {
        &self.0.inputs
    }
    pub fn files(&self) -> &[FileFacts] {
        &self.0.files
    }
    pub fn graph(&self) -> &GraphSnapshot {
        &self.0.graph
    }
    pub fn data(&self) -> &SnapshotData {
        &self.0
    }
    pub fn ensure_compatible(&self, expected: &IndexVersions) -> Result<(), IndexError> {
        expected.validate()?;
        if &self.metadata().versions != expected {
            return Err(IndexError(
                "incompatible schema/analyzer/hash version; rebuild required",
            ));
        }
        Ok(())
    }
    /// The caller must actually recapture/re-hash inputs and HEAD; this performs no IO.
    pub fn check_observation(
        &self,
        observed: &InputManifest,
        origin: &SnapshotOrigin,
    ) -> Result<(), IndexError> {
        if self.inputs() != observed || &self.metadata().origin != origin {
            return Err(IndexError(
                "captured inputs or HEAD changed; retry capture before publication/query",
            ));
        }
        Ok(())
    }
    pub fn validate_worktree_witness(&self, witness: &Witness) -> Result<(), IndexError> {
        if !matches!(self.metadata().origin, SnapshotOrigin::Worktree { .. })
            || witness.revision != RevisionSide::Worktree
        {
            return Err(IndexError(
                "worktree witness requires a worktree generation",
            ));
        }
        witness
            .validate(self.graph())
            .map_err(|_| IndexError("invalid worktree witness"))
    }
}
