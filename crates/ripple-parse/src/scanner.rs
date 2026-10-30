//! Discovery policy shared by worktrees and immutable tree providers.
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ripple_core::{
    identity::RepoPath,
    source::{SourceFile, SourceProvider, SourceSnapshot},
    syntax::Language,
};
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub const MAX_INPUT_BYTES: u64 = 16 * 1024 * 1024;
const EXCLUDED: &[&str] = &[
    "node_modules",
    ".git",
    ".ripple",
    "dist",
    "build",
    "coverage",
    "generated",
];

#[derive(Debug, thiserror::Error)]
#[error("source discovery: {0}")]
pub struct ScanError(pub String);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub path: RepoPath,
    pub kind: EntryKind,
}
/// Directory listings must contain only immediate children, with portable paths.
/// Implementations read immutable bytes or captured worktree bytes, never execute scripts.
pub trait SourceTree {
    fn entries(&self, directory: Option<&RepoPath>) -> Result<Vec<TreeEntry>, ScanError>;
    fn read(&self, path: &RepoPath) -> Result<Vec<u8>, ScanError>;
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredFile {
    pub path: RepoPath,
    pub language: Language,
    pub is_test: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedFile {
    pub path: RepoPath,
    pub bytes: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryDiagnostic {
    pub path: RepoPath,
    pub code: &'static str,
    pub message: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovery {
    pub files: Vec<DiscoveredFile>,
    pub sources: SourceSnapshot,
    /// Source, root config/package and active ignore bytes for later manifest hashing.
    pub inputs: Vec<CapturedFile>,
    pub diagnostics: Vec<DiscoveryDiagnostic>,
}
pub fn classify(path: &RepoPath) -> Option<(Language, bool)> {
    let name = path.as_str().rsplit('/').next()?;
    let language = if name.ends_with(".tsx") {
        Language::Tsx
    } else if name.ends_with(".ts") {
        Language::TypeScript
    } else {
        return None;
    };
    let is_test = [".test.ts", ".test.tsx", ".spec.ts", ".spec.tsx"]
        .iter()
        .any(|s| name.ends_with(s));
    Some((language, is_test))
}
fn read(tree: &impl SourceTree, path: &RepoPath) -> Result<Vec<u8>, ScanError> {
    let bytes = tree.read(path)?;
    if bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err(ScanError(format!(
            "{path}: input exceeds {MAX_INPUT_BYTES} bytes"
        )));
    }
    Ok(bytes)
}
fn utf8<'a>(bytes: &'a [u8], path: &RepoPath) -> Result<&'a str, ScanError> {
    std::str::from_utf8(bytes).map_err(|_| ScanError(format!("{path}: invalid UTF-8")))
}
fn ignored(rules: &[Gitignore], path: &RepoPath, is_dir: bool) -> bool {
    let candidate = Path::new("snapshot").join(path.as_str());
    for rule in rules.iter().rev() {
        let matched = rule.matched(&candidate, is_dir);
        if !matched.is_none() {
            return matched.is_ignore();
        }
    }
    false
}
pub fn discover(tree: &impl SourceTree) -> Result<Discovery, ScanError> {
    let mut files = Vec::new();
    let mut sources = Vec::new();
    let mut inputs = Vec::new();
    let mut diagnostics = Vec::new();
    let mut stack = vec![(None, Vec::<Gitignore>::new(), 0usize)];
    let mut seen = BTreeSet::new();
    let mut folded = BTreeSet::new();
    while let Some((directory, mut rules, depth)) = stack.pop() {
        if depth > 128 {
            return Err(ScanError("directory nesting exceeds 128".into()));
        }
        let mut entries = tree.entries(directory.as_ref())?;
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        for entry in &entries {
            let parent = entry.path.as_str().rsplit_once('/').map(|(p, _)| p);
            if parent != directory.as_ref().map(RepoPath::as_str)
                || !seen.insert(entry.path.clone())
            {
                return Err(ScanError(format!(
                    "invalid/duplicate immediate tree entry: {}",
                    entry.path
                )));
            }
            if !folded.insert(entry.path.as_str().to_lowercase()) {
                return Err(ScanError(format!(
                    "case-colliding portable path: {}",
                    entry.path
                )));
            }
            if seen.len() > 100_000 {
                return Err(ScanError("tree exceeds 100000 entries".into()));
            }
        }
        // Capture ignore bytes once; never load host-global or ancestor configuration.
        for name in [".gitignore", ".ignore"] {
            if let Some(entry) = entries.iter().find(|e| {
                e.path.as_str().rsplit('/').next() == Some(name) && e.kind == EntryKind::File
            }) {
                let bytes = read(tree, &entry.path)?;
                let root =
                    Path::new("snapshot").join(directory.as_ref().map_or("", RepoPath::as_str));
                let mut builder = GitignoreBuilder::new(root);
                for line in utf8(&bytes, &entry.path)?.lines() {
                    builder
                        .add_line(Some(PathBuf::from(entry.path.as_str())), line)
                        .map_err(|e| ScanError(format!("{}: {e}", entry.path)))?;
                }
                rules.push(builder.build().map_err(|e| ScanError(e.to_string()))?);
                inputs.push(CapturedFile {
                    path: entry.path.clone(),
                    bytes,
                });
            }
        }
        for entry in entries.into_iter().rev() {
            let name = entry.path.as_str().rsplit('/').next().unwrap_or("");
            if EXCLUDED.contains(&name) {
                continue;
            }
            if entry.kind == EntryKind::Symlink {
                diagnostics.push(DiscoveryDiagnostic {
                    path: entry.path,
                    code: "symlink_skipped",
                    message: "Symlinks are not followed, including links within the root".into(),
                });
                continue;
            }
            let root_config =
                directory.is_none() && ["tsconfig.json", "package.json"].contains(&name);
            let is_dir = entry.kind == EntryKind::Directory;
            if !root_config && ignored(&rules, &entry.path, is_dir) {
                continue;
            }
            if is_dir {
                stack.push((Some(entry.path), rules.clone(), depth + 1));
                continue;
            }
            if [".gitignore", ".ignore"].contains(&name) {
                continue;
            }
            if let Some((language, is_test)) = classify(&entry.path) {
                let bytes = read(tree, &entry.path)?;
                sources.push(SourceFile::new(
                    entry.path.clone(),
                    utf8(&bytes, &entry.path)?.into(),
                ));
                files.push(DiscoveredFile {
                    path: entry.path.clone(),
                    language,
                    is_test,
                });
                inputs.push(CapturedFile {
                    path: entry.path,
                    bytes,
                });
            } else if root_config {
                inputs.push(CapturedFile {
                    bytes: read(tree, &entry.path)?,
                    path: entry.path,
                });
            }
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    inputs.sort_by(|a, b| a.path.cmp(&b.path));
    diagnostics.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Discovery {
        files,
        sources: SourceSnapshot::new(sources).map_err(|e| ScanError(e.to_string()))?,
        inputs,
        diagnostics,
    })
}
#[derive(Debug)]
pub struct Worktree {
    root: PathBuf,
}
impl Worktree {
    /// The explicit root is the project boundary; parent project settings are not inherited.
    pub fn new(root: impl AsRef<Path>) -> Result<Self, ScanError> {
        let root = fs::canonicalize(root).map_err(|e| ScanError(e.to_string()))?;
        if !root.is_dir() {
            return Err(ScanError("project root must be a directory".into()));
        }
        Ok(Self { root })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn discover(&self) -> Result<Discovery, ScanError> {
        discover(self)
    }
    fn checked(&self, path: &RepoPath) -> Result<PathBuf, ScanError> {
        let mut current = self.root.clone();
        for part in path.as_str().split('/') {
            current.push(part);
            if fs::symlink_metadata(&current)
                .map_err(|e| ScanError(format!("{path}: {e}")))?
                .file_type()
                .is_symlink()
            {
                return Err(ScanError(format!("{path}: symlink changed during capture")));
            }
        }
        Ok(current)
    }
}
impl SourceTree for Worktree {
    fn entries(&self, directory: Option<&RepoPath>) -> Result<Vec<TreeEntry>, ScanError> {
        let dir = match directory {
            Some(p) => self.checked(p)?,
            None => self.root.clone(),
        };
        fs::read_dir(dir)
            .map_err(|e| ScanError(e.to_string()))?
            .map(|e| {
                let e = e.map_err(|e| ScanError(e.to_string()))?;
                let name = e
                    .file_name()
                    .into_string()
                    .map_err(|_| ScanError("non-UTF-8 filename unsupported".into()))?;
                if name.contains('\\') {
                    return Err(ScanError("literal backslash filename unsupported".into()));
                }
                let path = RepoPath::new(match directory {
                    Some(p) => format!("{p}/{name}"),
                    None => name,
                })
                .map_err(|e| ScanError(e.to_string()))?;
                let kind = e.file_type().map_err(|e| ScanError(e.to_string()))?;
                let kind = if kind.is_symlink() {
                    EntryKind::Symlink
                } else if kind.is_dir() {
                    EntryKind::Directory
                } else if kind.is_file() {
                    EntryKind::File
                } else {
                    return Err(ScanError(format!(
                        "{path}: special filesystem entry unsupported"
                    )));
                };
                Ok(TreeEntry { path, kind })
            })
            .collect()
    }
    fn read(&self, path: &RepoPath) -> Result<Vec<u8>, ScanError> {
        let path_on_disk = self.checked(path)?;
        let metadata = fs::symlink_metadata(&path_on_disk).map_err(|e| ScanError(e.to_string()))?;
        if !metadata.is_file() {
            return Err(ScanError(format!("{path}: expected regular file")));
        }
        let mut bytes = Vec::new();
        fs::File::open(path_on_disk)
            .map_err(|e| ScanError(e.to_string()))?
            .take(MAX_INPUT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| ScanError(e.to_string()))?;
        Ok(bytes)
    }
}
impl SourceProvider for Worktree {
    type Error = ScanError;
    fn capture(&self) -> Result<SourceSnapshot, ScanError> {
        Ok(self.discover()?.sources)
    }
}
