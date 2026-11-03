//! Immutable object reads. Never checkout/stash/stage, execute project scripts or read working files.
use crate::worktree::run;
use ripple_core::{
    identity::RepoPath,
    revision::{
        CommitId, ComparisonSpec, DiffHunk, FileChange, FileChangeKind, LineRange, RevisionChanges,
        RevisionPair,
    },
    source::{EntryKind, ScanError, SourceTree, TreeEntry},
};
use std::{
    cell::Cell,
    collections::BTreeMap,
    path::{Path, PathBuf},
};

const MAX_ENTRIES: usize = 100_000;
const MAX_BLOB: usize = 8 * 1024 * 1024;
const MAX_BYTES: usize = 256 * 1024 * 1024;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitError(pub String);
impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for GitError {}
fn err(s: impl Into<String>) -> GitError {
    GitError(s.into())
}
fn command(root: &Path, args: &[&str]) -> Result<Vec<u8>, GitError> {
    let (ok, bytes) = run(root, args).map_err(err)?;
    if !ok {
        return Err(err(format!(
            "Git {} failed; verify refs and local objects (no network fetch is requested)",
            args[0]
        )));
    }
    Ok(bytes)
}
fn text(bytes: &[u8]) -> Result<&str, GitError> {
    std::str::from_utf8(bytes).map_err(|_| err("non-UTF-8 Git record unsupported"))
}
fn oid(value: &str) -> Result<String, GitError> {
    CommitId::new(value.to_string())
        .map(|x| x.as_str().to_owned())
        .map_err(|e| err(e.to_string()))
}
fn portable(bytes: &[u8]) -> Result<RepoPath, GitError> {
    let value = text(bytes)?;
    let p = RepoPath::new(value).map_err(|e| err(e.to_string()))?;
    if p.as_str() != value {
        return Err(err("non-portable Git path rejected"));
    }
    Ok(p)
}
fn fields(bytes: &[u8]) -> Result<Vec<&[u8]>, GitError> {
    if !bytes.is_empty() && !bytes.ends_with(&[0]) {
        return Err(err("unterminated NUL Git record"));
    }
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let rows = bytes[..bytes.len() - 1]
        .split(|b| *b == 0)
        .collect::<Vec<_>>();
    if rows.iter().any(|row| row.is_empty()) {
        return Err(err("empty NUL Git record"));
    }
    Ok(rows)
}
#[derive(Debug, Clone)]
struct Object {
    mode: String,
    id: String,
}
#[derive(Debug, Clone)]
pub struct RevisionTree {
    root: PathBuf,
    pub revision: CommitId,
    objects: BTreeMap<RepoPath, Object>,
    directories: BTreeMap<Option<RepoPath>, Vec<TreeEntry>>,
    bytes_read: Cell<usize>,
}
impl RevisionTree {
    fn load(root: &Path, revision: CommitId) -> Result<Self, GitError> {
        let raw = command(
            root,
            &[
                "ls-tree",
                "-r",
                "-t",
                "-z",
                "--full-tree",
                revision.as_str(),
                "--",
            ],
        )?;
        let mut objects = BTreeMap::new();
        let mut directories = BTreeMap::<Option<RepoPath>, Vec<TreeEntry>>::new();
        for row in fields(&raw)? {
            if objects.len() >= MAX_ENTRIES {
                return Err(err("revision tree exceeds 100000 entries"));
            }
            let tab = row
                .iter()
                .position(|b| *b == b'\t')
                .ok_or_else(|| err("invalid tree record"))?;
            let head = text(&row[..tab])?.split(' ').collect::<Vec<_>>();
            if head.len() != 3 {
                return Err(err("invalid tree header"));
            }
            let path = portable(&row[tab + 1..])?;
            let id = oid(head[2])?;
            let kind = match (head[0], head[1]) {
                ("040000", "tree") => EntryKind::Directory,
                ("100644" | "100755", "blob") => EntryKind::File,
                ("120000", "blob") => EntryKind::Symlink,
                ("160000", "commit") => return Err(err("historical submodules are unsupported")),
                _ => return Err(err("unsupported Git object mode/type")),
            };
            let parent = path
                .as_str()
                .rsplit_once('/')
                .map(|(p, _)| RepoPath::new(p).map_err(|e| err(e.to_string())))
                .transpose()?;
            directories.entry(parent).or_default().push(TreeEntry {
                path: path.clone(),
                kind,
            });
            if objects
                .insert(
                    path,
                    Object {
                        mode: head[0].into(),
                        id,
                    },
                )
                .is_some()
            {
                return Err(err("duplicate tree path"));
            }
        }
        for list in directories.values_mut() {
            list.sort_by(|a, b| a.path.cmp(&b.path));
        }
        Ok(Self {
            root: root.into(),
            revision,
            objects,
            directories,
            bytes_read: Cell::new(0),
        })
    }
    fn blob(&self, object: &Object) -> Result<Vec<u8>, GitError> {
        if !matches!(object.mode.as_str(), "100644" | "100755") {
            return Err(err("only regular historical blobs may be read"));
        }
        let size = command(&self.root, &["cat-file", "-s", &object.id])?;
        let size: usize = text(&size)?
            .trim()
            .parse()
            .map_err(|_| err("invalid blob size"))?;
        let total = self
            .bytes_read
            .get()
            .checked_add(size)
            .ok_or_else(|| err("blob budget overflow"))?;
        if size > MAX_BLOB || total > MAX_BYTES {
            return Err(err(
                "historical blob exceeds 8 MiB/input or 256 MiB endpoint read budget",
            ));
        }
        let bytes = command(&self.root, &["cat-file", "blob", &object.id])?;
        if bytes.len() != size {
            return Err(err("blob length mismatch"));
        }
        self.bytes_read.set(total);
        Ok(bytes)
    }
}
impl SourceTree for RevisionTree {
    fn entries(
        &self,
        directory: Option<&RepoPath>,
        max_entries: usize,
    ) -> Result<Vec<TreeEntry>, ScanError> {
        let list = self
            .directories
            .get(&directory.cloned())
            .cloned()
            .unwrap_or_default();
        if list.len() > max_entries {
            return Err(ScanError("revision directory entry budget exceeded".into()));
        }
        Ok(list)
    }
    fn read(&self, path: &RepoPath) -> Result<Vec<u8>, ScanError> {
        let object = self
            .objects
            .get(path)
            .ok_or_else(|| ScanError(format!("{path}: missing historical input")))?;
        self.blob(object).map_err(|e| ScanError(e.to_string()))
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffDiagnostic {
    pub path: RepoPath,
    pub reason: &'static str,
}
#[derive(Debug)]
pub struct RevisionSnapshots {
    pub base: RevisionTree,
    pub head: RevisionTree,
    pub changes: RevisionChanges,
    pub diagnostics: Vec<DiffDiagnostic>,
}
#[derive(Debug)]
pub struct Repository {
    root: PathBuf,
}
impl Repository {
    pub fn open(root: &Path) -> Result<Self, GitError> {
        let root = std::fs::canonicalize(root).map_err(|e| err(e.to_string()))?;
        let top = command(&root, &["rev-parse", "--show-toplevel"])?;
        let actual = std::fs::canonicalize(text(&top)?.trim_end_matches(['\r', '\n']))
            .map_err(|e| err(e.to_string()))?;
        if actual != root {
            return Err(err("use the Git repository root"));
        }
        Ok(Self { root })
    }
    pub fn resolve(&self, spec: &ComparisonSpec) -> Result<RevisionPair, GitError> {
        // Revalidate public fields even when the caller did not use ComparisonSpec::parse.
        let validated = ComparisonSpec::parse(&format!("{}..{}", spec.base_ref, spec.head_ref))
            .map_err(|e| err(e.to_string()))?;
        let resolve = |reference: &str| {
            let value = format!("{reference}^{{commit}}");
            let bytes = command(
                &self.root,
                &["rev-parse", "--verify", "--end-of-options", &value],
            )?;
            CommitId::new(text(&bytes)?.trim().to_owned()).map_err(|e| err(e.to_string()))
        };
        Ok(RevisionPair {
            base: resolve(&validated.base_ref)?,
            head: resolve(&validated.head_ref)?,
        })
    }
    pub fn compare(&self, spec: &ComparisonSpec) -> Result<RevisionSnapshots, GitError> {
        self.compare_pair(self.resolve(spec)?)
    }
    pub fn compare_pair(&self, pair: RevisionPair) -> Result<RevisionSnapshots, GitError> {
        let base = RevisionTree::load(&self.root, pair.base.clone())?;
        let head = RevisionTree::load(&self.root, pair.head.clone())?;
        let raw = command(
            &self.root,
            &[
                "diff-tree",
                "--raw",
                "-z",
                "-r",
                "--no-commit-id",
                "--no-abbrev",
                "--no-ext-diff",
                "--no-textconv",
                "--no-renames",
                "--find-renames=50%",
                "-l1000",
                pair.base.as_str(),
                pair.head.as_str(),
                "--",
            ],
        )?;
        let mut files = parse_raw(&raw)?;
        let mut diagnostics = Vec::new();
        for file in &mut files {
            let old = file.old_path.as_ref().and_then(|p| base.objects.get(p));
            let new = file.new_path.as_ref().and_then(|p| head.objects.get(p));
            let path = file
                .new_path
                .as_ref()
                .or(file.old_path.as_ref())
                .unwrap()
                .clone();
            if old
                .into_iter()
                .chain(new)
                .any(|o| !matches!(o.mode.as_str(), "100644" | "100755"))
            {
                diagnostics.push(DiffDiagnostic {
                    path,
                    reason: "unsupported_file_mode",
                });
                continue;
            }
            let old_bytes = old.map(|o| base.blob(o)).transpose()?;
            let new_bytes = new.map(|o| head.blob(o)).transpose()?;
            if old_bytes
                .iter()
                .chain(new_bytes.iter())
                .any(|b| b.contains(&0) || std::str::from_utf8(b).is_err())
            {
                diagnostics.push(DiffDiagnostic {
                    path,
                    reason: "binary_or_non_utf8",
                });
                continue;
            }
            file.hunks = match (old, new) {
                (Some(a), Some(b)) if a.id != b.id => {
                    let patch = command(
                        &self.root,
                        &[
                            "diff",
                            "--no-ext-diff",
                            "--no-textconv",
                            "--no-color",
                            "--no-renames",
                            "--no-prefix",
                            "--diff-algorithm=myers",
                            "--no-indent-heuristic",
                            "--unified=0",
                            "--inter-hunk-context=0",
                            "--output-indicator-new=+",
                            "--output-indicator-old=-",
                            "--output-indicator-context= ",
                            "--text",
                            &a.id,
                            &b.id,
                            "--",
                        ],
                    )?;
                    parse_hunks(&patch)?
                }
                (None, Some(_)) => whole_file(new_bytes.as_deref().unwrap(), true)?,
                (Some(_), None) => whole_file(old_bytes.as_deref().unwrap(), false)?,
                _ => Vec::new(),
            };
        }
        let changes = RevisionChanges::new(pair, files).map_err(|e| err(e.to_string()))?;
        Ok(RevisionSnapshots {
            base,
            head,
            changes,
            diagnostics,
        })
    }
}
fn whole_file(bytes: &[u8], added: bool) -> Result<Vec<DiffHunk>, GitError> {
    let count = bytes.iter().filter(|b| **b == b'\n').count()
        + usize::from(!bytes.is_empty() && !bytes.ends_with(b"\n"));
    let count = u32::try_from(count).map_err(|_| err("too many lines"))?;
    if count == 0 {
        return Ok(Vec::new());
    }
    let full = LineRange { start: 1, count };
    let empty = LineRange { start: 0, count: 0 };
    Ok(vec![if added {
        DiffHunk {
            old: empty,
            new: full,
        }
    } else {
        DiffHunk {
            old: full,
            new: empty,
        }
    }])
}
fn parse_raw(raw: &[u8]) -> Result<Vec<FileChange>, GitError> {
    let rows = fields(raw)?;
    let mut i = 0;
    let mut files = Vec::new();
    while i < rows.len() {
        let header = text(rows[i])?.split(' ').collect::<Vec<_>>();
        i += 1;
        if header.len() != 5 || !header[0].starts_with(':') {
            return Err(err("invalid raw diff header"));
        }
        oid(header[2])?;
        oid(header[3])?;
        let status = header[4];
        let path = portable(rows.get(i).ok_or_else(|| err("missing diff path"))?)?;
        i += 1;
        let (old_path, new_path, kind, rename_similarity) = match status {
            "A" => (None, Some(path), FileChangeKind::Added, None),
            "D" => (Some(path), None, FileChangeKind::Deleted, None),
            "M" | "T" => (
                Some(path.clone()),
                Some(path),
                FileChangeKind::Modified,
                None,
            ),
            s if s.starts_with('R') => {
                let similarity = s[1..]
                    .parse::<u8>()
                    .map_err(|_| err("invalid rename score"))?;
                let new = portable(
                    rows.get(i)
                        .ok_or_else(|| err("missing rename destination"))?,
                )?;
                i += 1;
                (
                    Some(path),
                    Some(new),
                    FileChangeKind::Renamed,
                    Some(similarity),
                )
            }
            _ => return Err(err("unsupported raw diff status")),
        };
        files.push(FileChange {
            old_path,
            new_path,
            kind,
            rename_similarity,
            hunks: Vec::new(),
        });
    }
    Ok(files)
}
fn parse_hunks(patch: &[u8]) -> Result<Vec<DiffHunk>, GitError> {
    let mut result = Vec::new();
    for line in patch
        .split(|b| *b == b'\n')
        .filter(|l| l.starts_with(b"@@ "))
    {
        let parts = text(line)?.split(' ').collect::<Vec<_>>();
        if parts.len() < 4 || parts[3] != "@@" {
            return Err(err("invalid hunk header"));
        }
        let range = |s: &str, prefix: char| -> Result<LineRange, GitError> {
            let s = s
                .strip_prefix(prefix)
                .ok_or_else(|| err("invalid hunk range sign"))?;
            let (start, count) = s.split_once(',').unwrap_or((s, "1"));
            let range = LineRange {
                start: start.parse().map_err(|_| err("invalid hunk start"))?,
                count: count.parse().map_err(|_| err("invalid hunk count"))?,
            };
            range.validate().map_err(|e| err(e.to_string()))?;
            Ok(range)
        };
        result.push(DiffHunk {
            old: range(parts[1], '-')?,
            new: range(parts[2], '+')?,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_records_and_overflow_fail_instead_of_partial_output() {
        assert!(fields(b"record without terminator").is_err());
        assert!(fields(b"first\0\0second\0").is_err());
        assert!(parse_raw(b"bad\0path\0").is_err());
        assert!(parse_raw(b":100644 100644 deadbeef deadbeef M\0p.ts\0").is_err());
        assert!(parse_hunks(b"@@ -4294967296 +1 @@\n").is_err());
        assert!(parse_hunks(b"@@ -0 +1 @@\n").is_err());
        assert!(portable(b"a\\b.ts").is_err());
        assert!(portable(b"line\nfile.ts").is_err());
        let hunks =
            parse_hunks(b"@@ -1,0 +2 @@\n+@@ fake\n\\ No newline at end of file\n").unwrap();
        assert_eq!(
            hunks,
            vec![DiffHunk {
                old: LineRange { start: 1, count: 0 },
                new: LineRange { start: 2, count: 1 }
            }]
        );
    }
}
