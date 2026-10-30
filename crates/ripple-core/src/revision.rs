//! Immutable Git endpoint/change contracts. No Git process or filesystem operations.
use crate::{
    identity::RepoPath,
    index::IndexSnapshot,
    results::{RevisionSide, Witness},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid revision contract: {0}")]
pub struct RevisionError(pub &'static str);
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CommitId(String);
impl CommitId {
    pub fn new(value: impl Into<String>) -> Result<Self, RevisionError> {
        let value = value.into();
        if !matches!(value.len(), 40 | 64) || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(RevisionError("expected a full hexadecimal Git commit ID"));
        }
        Ok(Self(value.to_ascii_lowercase()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for CommitId {
    type Error = RevisionError;
    fn try_from(v: String) -> Result<Self, Self::Error> {
        Self::new(v)
    }
}
impl From<CommitId> for String {
    fn from(v: CommitId) -> Self {
        v.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionPair {
    pub base: CommitId,
    pub head: CommitId,
}
/// Only parsing the explicit endpoint syntax; the Git adapter resolves refs once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonSpec {
    pub base_ref: String,
    pub head_ref: String,
}
impl ComparisonSpec {
    pub fn parse(value: &str) -> Result<Self, RevisionError> {
        if value.contains("...") {
            return Err(RevisionError(
                "three-dot comparison unsupported; use explicit A..B endpoints",
            ));
        }
        let (base, head) = value
            .split_once("..")
            .ok_or(RevisionError("expected A..B"))?;
        if [base, head].iter().any(|r| {
            r.is_empty()
                || r.trim() != *r
                || r.starts_with('-')
                || r.contains("..")
                || r.chars().any(char::is_control)
        }) {
            return Err(RevisionError("invalid or ambiguous endpoint reference"));
        }
        Ok(Self {
            base_ref: base.into(),
            head_ref: head.into(),
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SnapshotOrigin {
    Worktree { head: Option<CommitId>, dirty: bool },
    Commit { revision: CommitId },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LineRange {
    pub start: u32,
    pub count: u32,
}
impl LineRange {
    // Git empty ranges anchor after start; nonempty ranges are one-based.
    fn interval(self) -> (u64, u64) {
        let start = u64::from(self.start) - u64::from(self.count > 0);
        (start, start + u64::from(self.count))
    }

    pub fn validate(self) -> Result<(), RevisionError> {
        if (self.count > 0 && self.start == 0) || self.start.checked_add(self.count).is_none() {
            return Err(RevisionError(
                "nonempty Git ranges start at line 1; range overflow is invalid",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DiffHunk {
    pub old: LineRange,
    pub new: LineRange,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FileChangeKind {
    Added,
    Deleted,
    Modified,
    Renamed,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FileChange {
    pub old_path: Option<RepoPath>,
    pub new_path: Option<RepoPath>,
    pub kind: FileChangeKind,
    pub rename_similarity: Option<u8>,
    pub hunks: Vec<DiffHunk>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawChanges")]
pub struct RevisionChanges {
    revisions: RevisionPair,
    files: Vec<FileChange>,
}
#[derive(Deserialize)]
struct RawChanges {
    revisions: RevisionPair,
    files: Vec<FileChange>,
}
impl TryFrom<RawChanges> for RevisionChanges {
    type Error = RevisionError;
    fn try_from(raw: RawChanges) -> Result<Self, Self::Error> {
        Self::new(raw.revisions, raw.files)
    }
}
impl RevisionChanges {
    pub fn revisions(&self) -> &RevisionPair {
        &self.revisions
    }
    pub fn new(revisions: RevisionPair, mut files: Vec<FileChange>) -> Result<Self, RevisionError> {
        let (mut old_paths, mut new_paths) = (BTreeSet::new(), BTreeSet::new());
        for file in &mut files {
            let shape = match file.kind {
                FileChangeKind::Added => file.old_path.is_none() && file.new_path.is_some(),
                FileChangeKind::Deleted => file.old_path.is_some() && file.new_path.is_none(),
                FileChangeKind::Modified => {
                    file.old_path.is_some() && file.old_path == file.new_path
                }
                FileChangeKind::Renamed => {
                    file.old_path.is_some()
                        && file.new_path.is_some()
                        && file.old_path != file.new_path
                }
            };
            if !shape
                || file
                    .rename_similarity
                    .is_some_and(|v| v > 100 || file.kind != FileChangeKind::Renamed)
            {
                return Err(RevisionError(
                    "file change paths/rename evidence inconsistent",
                ));
            }
            if file
                .old_path
                .as_ref()
                .is_some_and(|p| !old_paths.insert(p.clone()))
                || file
                    .new_path
                    .as_ref()
                    .is_some_and(|p| !new_paths.insert(p.clone()))
            {
                return Err(RevisionError("duplicate old/new file change path"));
            }
            for hunk in &file.hunks {
                hunk.old.validate()?;
                hunk.new.validate()?;
                if (file.old_path.is_none() && hunk.old != (LineRange { start: 0, count: 0 }))
                    || (file.new_path.is_none() && hunk.new != (LineRange { start: 0, count: 0 }))
                {
                    return Err(RevisionError(
                        "nonexistent file side must have the zero range",
                    ));
                }
                if hunk.old.count == 0 && hunk.new.count == 0 {
                    return Err(RevisionError("hunk changes neither endpoint"));
                }
            }
            file.hunks.sort_by_key(|h| {
                let (old_start, old_end) = h.old.interval();
                let (new_start, new_end) = h.new.interval();
                (old_start, new_start, old_end, new_end)
            });
            if file.hunks.windows(2).any(|p| p[0] == p[1]) {
                return Err(RevisionError("duplicate diff hunk"));
            }
            if file.hunks.windows(2).any(|p| {
                p[1].old.interval().0 < p[0].old.interval().1
                    || p[1].new.interval().0 < p[0].new.interval().1
            }) {
                return Err(RevisionError("overlapping or crossing diff hunks"));
            }
        }
        files.sort();
        Ok(Self { revisions, files })
    }
    pub fn files(&self) -> &[FileChange] {
        &self.files
    }
}

/// Binds witness side labels to the two actual immutable snapshots, including deletions.
#[derive(Debug)]
pub struct RevisionGraphs<'a> {
    revisions: RevisionPair,
    base: &'a IndexSnapshot,
    head: &'a IndexSnapshot,
}
impl<'a> RevisionGraphs<'a> {
    pub fn revisions(&self) -> &RevisionPair {
        &self.revisions
    }
    pub fn new(
        revisions: RevisionPair,
        base: &'a IndexSnapshot,
        head: &'a IndexSnapshot,
    ) -> Result<Self, RevisionError> {
        if base.metadata().origin
            != (SnapshotOrigin::Commit {
                revision: revisions.base.clone(),
            })
            || head.metadata().origin
                != (SnapshotOrigin::Commit {
                    revision: revisions.head.clone(),
                })
            || base.metadata().project != head.metadata().project
            || base.metadata().versions != head.metadata().versions
        {
            return Err(RevisionError(
                "base/head snapshots must match immutable endpoints, project and analyzer formats",
            ));
        }
        Ok(Self {
            revisions,
            base,
            head,
        })
    }
    pub fn validate_witness(&self, witness: &Witness) -> Result<(), RevisionError> {
        let snapshot = match witness.revision {
            RevisionSide::Base => self.base,
            RevisionSide::Head => self.head,
            RevisionSide::Worktree => {
                return Err(RevisionError(
                    "worktree witness is not a two-revision result",
                ));
            }
        };
        witness
            .validate(snapshot.graph())
            .map_err(|_| RevisionError("witness invalid in its labelled revision graph"))
    }
}
