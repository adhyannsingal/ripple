use crate::storage::{InputBytes, input_hash};
use ripple_core::{
    identity::RepoPath,
    index::{CapturedInput, InputManifest, InputRole},
    revision::SnapshotOrigin,
};
use ripple_parse::scanner::{Discovery, EntryKind, SourceTree, TreeEntry, Worktree};
use ripple_resolve::config::{CapturedConfigs, ConfigInput, EffectiveConfig};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
const LOCKS: &[&str] = &[
    "package-lock.json",
    "npm-shrinkwrap.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "bun.lock",
    "bun.lockb",
];
struct BudgetTree<'a, T> {
    tree: &'a T,
    total: Cell<usize>,
    listed: Cell<usize>,
    listings: RefCell<BTreeMap<Option<RepoPath>, BTreeMap<String, TreeEntry>>>,
}
impl<T: SourceTree> SourceTree for BudgetTree<'_, T> {
    fn entries(
        &self,
        dir: Option<&RepoPath>,
        max: usize,
    ) -> Result<Vec<TreeEntry>, ripple_parse::scanner::ScanError> {
        if let Some(list) = self.listings.borrow().get(&dir.cloned()) {
            if list.len() > max {
                return Err(ripple_parse::scanner::ScanError(
                    "directory entry limit exceeded".into(),
                ));
            }
            return Ok(list.values().cloned().collect());
        }
        let entries = self.tree.entries(dir, max)?;
        let count = self.listed.get().saturating_add(entries.len());
        if count > 100_000 {
            return Err(ripple_parse::scanner::ScanError(
                "capture lists more than 100000 entries".into(),
            ));
        }
        self.listed.set(count);
        let mut list = BTreeMap::new();
        let mut folded = BTreeSet::new();
        for entry in &entries {
            let name = entry
                .path
                .as_str()
                .rsplit('/')
                .next()
                .unwrap_or("")
                .to_string();
            if !folded.insert(name.to_lowercase()) {
                return Err(ripple_parse::scanner::ScanError(
                    "case-colliding auxiliary path".into(),
                ));
            }
            list.insert(name, entry.clone());
        }
        self.listings.borrow_mut().insert(dir.cloned(), list);
        Ok(entries)
    }
    fn read(&self, path: &RepoPath) -> Result<Vec<u8>, ripple_parse::scanner::ScanError> {
        let bytes = self.tree.read(path)?;
        let total = self.total.get().saturating_add(bytes.len());
        if bytes.len() > 16 * 1024 * 1024 || total > 256 * 1024 * 1024 {
            return Err(ripple_parse::scanner::ScanError(
                "captured byte budget exceeds 16 MiB/file or 256 MiB total".into(),
            ));
        }
        self.total.set(total);
        Ok(bytes)
    }
}
fn optional(
    tree: &BudgetTree<'_, impl SourceTree>,
    path: &RepoPath,
) -> Result<Option<Vec<u8>>, String> {
    let parts = path.as_str().split('/').collect::<Vec<_>>();
    let mut parent = None;
    for (i, part) in parts.iter().enumerate() {
        if [
            ".git",
            ".ripple",
            "node_modules",
            "dist",
            "build",
            "coverage",
            "generated",
        ]
        .contains(part)
        {
            return Err("configuration dependency enters an excluded directory".into());
        }
        if !tree.listings.borrow().contains_key(&parent) {
            tree.entries(parent.as_ref(), 100_000)
                .map_err(|e| e.to_string())?;
        }
        let entry = tree
            .listings
            .borrow()
            .get(&parent)
            .and_then(|list| list.get(*part))
            .cloned();
        let Some(entry) = entry else { return Ok(None) };
        if i + 1 == parts.len() {
            if entry.kind != EntryKind::File {
                return Err(format!("{path}: auxiliary input is not a regular file"));
            }
            return tree.read(path).map(Some).map_err(|e| e.to_string());
        }
        if entry.kind != EntryKind::Directory {
            return Err(format!(
                "{path}: auxiliary parent is not a regular directory"
            ));
        }
        parent = Some(entry.path.clone());
    }
    Ok(None)
}
pub(crate) struct Capture {
    pub discovery: Discovery,
    pub inputs: Vec<InputBytes>,
    pub manifest: InputManifest,
    pub missing: Vec<RepoPath>,
    pub config: EffectiveConfig,
    pub origin: SnapshotOrigin,
}
impl Capture {
    pub fn matches(&self, other: &Self) -> bool {
        self.manifest == other.manifest
            && self.missing == other.missing
            && self.origin == other.origin
            && self.discovery.diagnostics == other.discovery.diagnostics
            && self.config.fingerprint() == other.config.fingerprint()
    }
}
pub(crate) fn capture(
    root: &Path,
    provenance: crate::build::Provenance,
) -> Result<Capture, String> {
    let origin = provenance(root)?;
    let tree = Worktree::new(root).map_err(|e| e.to_string())?;
    let captured = capture_tree(&tree, origin.clone())?;
    if origin != provenance(root)? {
        return Err("Git provenance changed during capture".into());
    }
    Ok(captured)
}
/// The caller supplies an immutable provider/origin; worktree callers verify separately.
pub(crate) fn capture_tree(
    provider: &impl SourceTree,
    origin: SnapshotOrigin,
) -> Result<Capture, String> {
    let tree = BudgetTree {
        tree: provider,
        total: Cell::new(0),
        listed: Cell::new(0),
        listings: RefCell::new(BTreeMap::new()),
    };
    let discovery = ripple_parse::scanner::discover(&tree).map_err(|e| e.to_string())?;
    if tree
        .listings
        .borrow()
        .iter()
        .any(|(dir, entries)| dir.is_some() && entries.contains_key(".git"))
    {
        return Err("nested Git projects are unsupported in the single-project M2 scope".into());
    }
    let mut bytes = discovery
        .inputs
        .iter()
        .map(|i| (i.path.clone(), i.bytes.clone()))
        .collect::<BTreeMap<_, _>>();
    for name in ["tsconfig.json", "package.json"] {
        let path = RepoPath::new(name).map_err(|e| e.to_string())?;
        if let std::collections::btree_map::Entry::Vacant(entry) = bytes.entry(path.clone())
            && let Some(value) = optional(&tree, &path)?
        {
            entry.insert(value);
        }
    }
    let mut probes = BTreeSet::new();
    let root_config = RepoPath::new("tsconfig.json").map_err(|e| e.to_string())?;
    probes.insert(root_config.clone());
    let mut configs = BTreeMap::new();
    if let Some(input) = bytes.get(&root_config) {
        configs.insert(root_config.clone(), input.clone());
    }
    let mut config = if configs.is_empty() {
        EffectiveConfig::implicit(root_config.clone())?
    } else {
        let mut selected = None;
        for _ in 0..33 {
            let evaluated = CapturedConfigs::new(configs.iter().map(|(path, bytes)| ConfigInput {
                path: path.clone(),
                bytes: bytes.clone(),
            }))
            .map_err(|e| e.to_string())?
            .evaluate(&root_config)
            .map_err(|e| e.to_string())?;
            let mut changed = false;
            for input in evaluated.inputs() {
                probes.insert(input.path.clone());
                if configs.contains_key(&input.path) {
                    continue;
                }
                let value = if let Some(value) = bytes.get(&input.path) {
                    Some(value.clone())
                } else {
                    optional(&tree, &input.path)?
                };
                if let Some(value) = value {
                    bytes.insert(input.path.clone(), value.clone());
                    configs.insert(input.path.clone(), value);
                    changed = true;
                    break;
                }
            }
            if !changed {
                selected = Some(evaluated);
                break;
            }
        }
        selected.ok_or("configuration capture did not converge")?
    };
    let directories = tree.listings.borrow().keys().cloned().collect::<Vec<_>>();
    for dir in directories {
        let path = RepoPath::new(match dir {
            Some(dir) => format!("{dir}/package.json"),
            None => "package.json".into(),
        })
        .map_err(|e| e.to_string())?;
        probes.insert(path.clone());
        if let std::collections::btree_map::Entry::Vacant(entry) = bytes.entry(path.clone())
            && let Some(value) = optional(&tree, &path)?
        {
            entry.insert(value);
        }
    }
    config = config.with_package_manifests(
        bytes
            .iter()
            .filter(|(p, _)| p.as_str().rsplit('/').next() == Some("package.json"))
            .map(|(p, b)| (p, b.as_slice())),
    )?;
    for name in LOCKS {
        let path = RepoPath::new(name).map_err(|e| e.to_string())?;
        probes.insert(path.clone());
        if let Some(value) = optional(&tree, &path)? {
            bytes.insert(path, value);
        }
    }
    // Root manifests may be absent on the first scan: retain absence for verification.
    probes.insert(RepoPath::new("package.json").map_err(|e| e.to_string())?);
    let missing = probes
        .into_iter()
        .filter(|p| !bytes.contains_key(p))
        .collect::<Vec<_>>();
    let manifest = InputManifest::new(
        bytes
            .iter()
            .map(|(path, bytes)| CapturedInput {
                path: path.clone(),
                byte_len: bytes.len() as u64,
                content_hash: input_hash(bytes),
                role: if discovery.sources.get(path).is_some() {
                    InputRole::Source
                } else if matches!(
                    path.as_str().rsplit('/').next(),
                    Some(".gitignore" | ".ignore")
                ) {
                    InputRole::IgnorePolicy
                } else if path.as_str().rsplit('/').next() == Some("package.json") {
                    InputRole::PackageManifest
                } else if LOCKS.contains(&path.as_str()) {
                    InputRole::Lockfile
                } else {
                    InputRole::Configuration
                },
            })
            .collect(),
    )
    .map_err(|e| e.to_string())?;
    Ok(Capture {
        discovery,
        inputs: bytes
            .into_iter()
            .map(|(path, bytes)| InputBytes { path, bytes })
            .collect(),
        manifest,
        missing,
        config,
        origin,
    })
}
