//! Pure base/head syntax change classification. Git/IO, storage and parsing belong to callers.
use ripple_core::{
    identity::{RepoPath, SymbolId, SymbolKind},
    revision::{CommitId, FileChange, FileChangeKind, LineRange, RevisionChanges, RevisionPair},
    source::SourceSnapshot,
    syntax::{FileFacts, ParseStatus, SymbolFact, VersionedHash},
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
pub const CHANGE_POLICY: &str = "ripple-changes-v1";
const MAX_RECORDS: usize = 1_000_000;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeError(pub String);
impl std::fmt::Display for ChangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ChangeError {}
fn err(s: impl Into<String>) -> ChangeError {
    ChangeError(s.into())
}
/// Callers bind captured sources/facts to immutable commits; this layer performs no Git reads.
pub struct Endpoint<'a> {
    pub revision: &'a CommitId,
    pub analyzer_version: &'a str,
    pub sources: &'a SourceSnapshot,
    pub facts: &'a [FileFacts],
    /// Captured effective config/ignore/package inputs, including inherited nonstandard config names.
    pub global_inputs: &'a BTreeSet<RepoPath>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChangeKind {
    Added,
    Deleted,
    BodyModified,
    SignatureModified,
    Unchanged,
    Uncertain,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Reason {
    PresenceChanged,
    FingerprintChanged,
    FingerprintEqual,
    UncertainFingerprint,
    EndpointDiagnostics,
    OutsideSymbols,
    BoundaryAnchor,
    AmbiguousOwnership,
    UnindexedInput,
    NoTextHunks,
    NoSupportedSymbolChange,
    RenameAsDeleteAdd,
    GlobalInputChanged,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SideMapping {
    pub range: Option<ByteRange>,
    /// Coarsely touched owners, not proof that every owner semantically changed.
    pub owners: Vec<SymbolId>,
    pub outside: Vec<ByteRange>,
    pub ambiguous: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HunkMapping {
    pub base: SideMapping,
    pub head: SideMapping,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileDelta {
    pub change: FileChange,
    pub hunks: Vec<HunkMapping>,
    pub reasons: Vec<Reason>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SymbolChange {
    pub identity: SymbolId,
    pub base: Option<SymbolFact>,
    pub head: Option<SymbolFact>,
    pub kind: ChangeKind,
    pub signature_changed: Option<bool>,
    pub body_changed: Option<bool>,
    pub reasons: Vec<Reason>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangeSet {
    policy: &'static str,
    revisions: RevisionPair,
    files: Vec<FileDelta>,
    symbols: Vec<SymbolChange>,
    incomplete: bool,
    global_invalidation: bool,
}
impl ChangeSet {
    pub fn revisions(&self) -> &RevisionPair {
        &self.revisions
    }
    pub fn files(&self) -> &[FileDelta] {
        &self.files
    }
    pub fn symbols(&self) -> &[SymbolChange] {
        &self.symbols
    }
    pub fn incomplete(&self) -> bool {
        self.incomplete
    }
    pub fn global_invalidation(&self) -> bool {
        self.global_invalidation
    }
}
struct Segment {
    range: ByteRange,
    owner: Option<SymbolId>,
    owner_span: Option<ByteRange>,
    ambiguous: bool,
}
struct File<'a> {
    facts: &'a FileFacts,
    lines: Vec<usize>,
    length: usize,
    physical_lines: usize,
    segments: Vec<Segment>,
}
fn segments(f: &FileFacts, length: usize) -> Vec<Segment> {
    let mut events = BTreeMap::<usize, Vec<(bool, &SymbolFact)>>::new();
    events.entry(0).or_default();
    events.entry(length).or_default();
    for s in &f.symbols {
        if s.span.start().byte == s.span.end().byte {
            continue;
        }
        events
            .entry(s.span.start().byte)
            .or_default()
            .push((true, s));
        events
            .entry(s.span.end().byte)
            .or_default()
            .push((false, s));
    }
    let mut active = BTreeSet::new();
    let mut iter = events.iter().peekable();
    let mut result = Vec::new();
    while let Some((&start, changes)) = iter.next() {
        for &(add, s) in changes
            .iter()
            .filter(|(add, _)| !*add)
            .chain(changes.iter().filter(|(add, _)| *add))
        {
            let key = (s.span.end().byte - s.span.start().byte, s);
            if add {
                active.insert(key);
            } else {
                active.remove(&key);
            }
        }
        let Some(end) = iter.peek().map(|(p, _)| **p) else {
            break;
        };
        let first = active.first();
        let ambiguous = first
            .is_some_and(|(len, _)| active.iter().nth(1).is_some_and(|(other, _)| other == len));
        let owner = if ambiguous {
            None
        } else {
            first.map(|(_, s)| *s)
        };
        result.push(Segment {
            range: ByteRange { start, end },
            owner: owner.map(|s| s.identity.clone()),
            owner_span: owner.map(|s| ByteRange {
                start: s.span.start().byte,
                end: s.span.end().byte,
            }),
            ambiguous,
        });
    }
    result
}
fn prepare<'a>(e: &Endpoint<'a>) -> Result<BTreeMap<RepoPath, File<'a>>, ChangeError> {
    let mut files = BTreeMap::new();
    for f in e.facts {
        let source = e
            .sources
            .get(&f.path)
            .ok_or_else(|| err("facts outside captured source inventory"))?;
        f.validate(source).map_err(|x| err(x.to_string()))?;
        let bytes = source.text().as_bytes();
        let mut lines = vec![0];
        lines.extend(
            bytes
                .iter()
                .enumerate()
                .filter(|(_, b)| **b == b'\n')
                .map(|(i, _)| i + 1),
        );
        let physical_lines = bytes.iter().filter(|b| **b == b'\n').count()
            + usize::from(!bytes.is_empty() && !bytes.ends_with(b"\n"));
        if files
            .insert(
                f.path.clone(),
                File {
                    facts: f,
                    lines,
                    length: bytes.len(),
                    physical_lines,
                    segments: segments(f, bytes.len()),
                },
            )
            .is_some()
        {
            return Err(err("duplicate fact file"));
        }
    }
    if files.len() != e.sources.files().count() {
        return Err(err("captured source missing facts"));
    }
    Ok(files)
}
fn map(file: Option<&File<'_>>, range: LineRange) -> Result<SideMapping, ChangeError> {
    let Some(file) = file else {
        return Ok(SideMapping {
            range: None,
            owners: vec![],
            outside: vec![],
            ambiguous: false,
        });
    };
    range.validate().map_err(|x| err(x.to_string()))?;
    let (start_line, end_line) = if range.count == 0 {
        (range.start as usize, range.start as usize)
    } else {
        (
            range.start as usize - 1,
            range.start as usize - 1 + range.count as usize,
        )
    };
    if end_line > file.physical_lines {
        return Err(err("Git hunk outside captured source lines"));
    }
    let offset = |line: usize| file.lines.get(line).copied().unwrap_or(file.length);
    let range = ByteRange {
        start: offset(start_line),
        end: offset(end_line),
    };
    let mut owners = BTreeSet::new();
    let mut outside = Vec::new();
    let mut ambiguous = false;
    let index = file
        .segments
        .partition_point(|s| s.range.end <= range.start);
    if range.start == range.end {
        if let Some(s) = file.segments.get(index) {
            ambiguous = s.ambiguous;
            if s.owner_span
                .is_some_and(|span| span.start < range.start && range.start < span.end)
            {
                owners.extend(s.owner.clone());
            } else {
                outside.push(range);
            }
        } else {
            outside.push(range);
        }
    } else {
        for s in file.segments[index..]
            .iter()
            .take_while(|s| s.range.start < range.end)
        {
            ambiguous |= s.ambiguous;
            if let Some(id) = &s.owner {
                owners.insert(id.clone());
            } else {
                outside.push(ByteRange {
                    start: s.range.start.max(range.start),
                    end: s.range.end.min(range.end),
                });
            }
        }
    }
    Ok(SideMapping {
        range: Some(range),
        owners: owners.into_iter().collect(),
        outside,
        ambiguous,
    })
}
fn compare_hash(a: &Option<VersionedHash>, b: &Option<VersionedHash>) -> Option<bool> {
    match (a, b) {
        (Some(a), Some(b)) if a.version == b.version => Some(a.digest != b.digest),
        _ => None,
    }
}
fn complete(f: Option<&File<'_>>) -> bool {
    f.is_some_and(|f| f.facts.parse_status == ParseStatus::Clean && f.facts.diagnostics.is_empty())
}
fn global(path: &RepoPath, a: &Endpoint<'_>, b: &Endpoint<'_>) -> bool {
    let name = path.as_str().rsplit('/').next().unwrap_or("");
    a.global_inputs.contains(path)
        || b.global_inputs.contains(path)
        || matches!(
            name,
            "package.json"
                | "package-lock.json"
                | "npm-shrinkwrap.json"
                | "yarn.lock"
                | "pnpm-lock.yaml"
                | "bun.lock"
                | "bun.lockb"
                | ".gitignore"
                | ".ignore"
        )
        || (name.starts_with("tsconfig") && (name.ends_with(".json") || name.ends_with(".jsonc")))
}
/// The immutable source-to-commit binding is supplied by the caller; facts/spans and declared IDs are checked here.
pub fn classify(
    changes: &RevisionChanges,
    base: Endpoint<'_>,
    head: Endpoint<'_>,
) -> Result<ChangeSet, ChangeError> {
    if base.revision != &changes.revisions().base || head.revision != &changes.revisions().head {
        return Err(err("endpoint revisions disagree with Git changes"));
    }
    if base.analyzer_version.is_empty() || base.analyzer_version != head.analyzer_version {
        return Err(err("build both endpoints with the same analyzer"));
    }
    let records = base
        .facts
        .iter()
        .chain(head.facts)
        .map(|f| 1 + f.symbols.len())
        .sum::<usize>()
        + changes
            .files()
            .iter()
            .map(|f| 1 + f.hunks.len())
            .sum::<usize>();
    if records > MAX_RECORDS {
        return Err(err(
            "change classification exceeds one million input records",
        ));
    }
    let projects = base
        .facts
        .iter()
        .chain(head.facts)
        .flat_map(|f| f.symbols.iter().map(|s| s.identity.project()))
        .collect::<BTreeSet<_>>();
    if projects.len() > 1 {
        return Err(err("mixed project facts"));
    }
    let a = prepare(&base)?;
    let b = prepare(&head)?;
    if base.revision == head.revision
        && (base.sources != head.sources || !changes.files().is_empty())
    {
        return Err(err(
            "identical commit endpoints must have identical source and no changes",
        ));
    }
    let mut result = ChangeSet {
        policy: CHANGE_POLICY,
        revisions: changes.revisions().clone(),
        files: vec![],
        symbols: vec![],
        incomplete: false,
        global_invalidation: false,
    };
    for change in changes.files() {
        let af = change.old_path.as_ref().and_then(|p| a.get(p));
        let bf = change.new_path.as_ref().and_then(|p| b.get(p));
        let mut reasons = BTreeSet::new();
        let mut touched = BTreeSet::new();
        let mut hunks = Vec::new();
        if change.kind == FileChangeKind::Renamed {
            reasons.insert(Reason::RenameAsDeleteAdd);
        }
        if change.hunks.is_empty() {
            reasons.insert(Reason::NoTextHunks);
        }
        if change
            .old_path
            .iter()
            .chain(change.new_path.iter())
            .any(|p| global(p, &base, &head))
        {
            reasons.insert(Reason::GlobalInputChanged);
            result.global_invalidation = true;
        }
        if (change.old_path.is_some() && af.is_none())
            || (change.new_path.is_some() && bf.is_none())
        {
            reasons.insert(Reason::UnindexedInput);
            result.incomplete = true;
        }
        if af.into_iter().chain(bf).any(|f| !complete(Some(f))) {
            reasons.insert(Reason::EndpointDiagnostics);
            result.incomplete = true;
        }
        for h in &change.hunks {
            let old = map(af, h.old)?;
            let new = map(bf, h.new)?;
            for m in [&old, &new] {
                touched.extend(m.owners.iter().cloned());
                if !m.outside.is_empty() {
                    reasons.insert(Reason::OutsideSymbols);
                    if m.range.is_some_and(|r| r.start == r.end) {
                        reasons.insert(Reason::BoundaryAnchor);
                    }
                }
                if m.ambiguous {
                    reasons.insert(Reason::AmbiguousOwnership);
                    result.incomplete = true;
                }
            }
            hunks.push(HunkMapping {
                base: old,
                head: new,
            });
        }
        let old = af
            .into_iter()
            .flat_map(|f| f.facts.symbols.iter())
            .map(|s| (s.identity.clone(), s))
            .collect::<BTreeMap<_, _>>();
        let new = bf
            .into_iter()
            .flat_map(|f| f.facts.symbols.iter())
            .map(|s| (s.identity.clone(), s))
            .collect::<BTreeMap<_, _>>();
        let ids = old
            .keys()
            .chain(new.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let initial = result.symbols.len();
        for id in ids {
            let before = old.get(&id).copied();
            let after = new.get(&id).copied();
            let mut why = Vec::new();
            let (sig, body, kind) = match (before, after) {
                (Some(x), Some(y)) => {
                    let sig = compare_hash(&x.signature_hash, &y.signature_hash);
                    let body = compare_hash(&x.body_hash, &y.body_hash);
                    let kind = match (sig, body) {
                        (Some(true), _) => ChangeKind::SignatureModified,
                        (Some(false), Some(true)) => ChangeKind::BodyModified,
                        (Some(false), Some(false)) => ChangeKind::Unchanged,
                        _ => ChangeKind::Uncertain,
                    };
                    if sig.is_none() || body.is_none() {
                        why.push(Reason::UncertainFingerprint);
                        result.incomplete = true;
                    }
                    if kind == ChangeKind::Unchanged
                        && (id.kind() == SymbolKind::Class || !touched.contains(&id))
                    {
                        continue;
                    }
                    if kind == ChangeKind::Unchanged {
                        why.push(Reason::FingerprintEqual);
                    } else if sig == Some(true) || body == Some(true) {
                        why.push(Reason::FingerprintChanged);
                    }
                    (sig, body, kind)
                }
                (Some(_), None) => {
                    why.push(Reason::PresenceChanged);
                    (
                        None,
                        None,
                        if matches!(
                            change.kind,
                            FileChangeKind::Deleted | FileChangeKind::Renamed
                        ) || complete(bf)
                        {
                            ChangeKind::Deleted
                        } else {
                            why.push(Reason::EndpointDiagnostics);
                            result.incomplete = true;
                            ChangeKind::Uncertain
                        },
                    )
                }
                (None, Some(_)) => {
                    why.push(Reason::PresenceChanged);
                    (
                        None,
                        None,
                        if matches!(change.kind, FileChangeKind::Added | FileChangeKind::Renamed)
                            || complete(af)
                        {
                            ChangeKind::Added
                        } else {
                            why.push(Reason::EndpointDiagnostics);
                            result.incomplete = true;
                            ChangeKind::Uncertain
                        },
                    )
                }
                _ => unreachable!("identity union"),
            };
            result.symbols.push(SymbolChange {
                identity: id,
                base: before.cloned(),
                head: after.cloned(),
                kind,
                signature_changed: sig,
                body_changed: body,
                reasons: why,
            });
        }
        if initial == result.symbols.len() {
            reasons.insert(Reason::NoSupportedSymbolChange);
        }
        result.files.push(FileDelta {
            change: change.clone(),
            hunks,
            reasons: reasons.into_iter().collect(),
        });
    }
    result.symbols.sort_by(|a, b| a.identity.cmp(&b.identity));
    Ok(result)
}
