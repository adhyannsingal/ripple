//! Configured export-origin closure. No guessed call edges, IO or incremental cutoff.
use crate::{
    ResolutionError,
    config::{CONFIG_VERSION, ConfigDependency, ConfigDiagnostic, EffectiveConfig},
    exports::{self, BindingOutcome, BindingProblem, DependencyKind},
    paths::{ConfigOutcome, ConfiguredLookup, ConfiguredResolver},
};
use ripple_core::{
    graph::SourceLocation,
    identity::{ProjectKey, RepoPath, SymbolId},
    source::{SourceSnapshot, SourceSpan},
    syntax::{BindingKind, ExportKind, ExportName, FileFacts, ImportedName, VersionedHash},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const MODULES_VERSION: &str = "ripple-modules-v1";
const MAX_CELLS: usize = 100_000;
const MAX_WORK: usize = 5_000_000;
const MAX_ROUNDS: usize = 512;
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExportKey {
    pub file: RepoPath,
    pub name: ExportName,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ExportProblem {
    UnsupportedConfig,
    IncompleteSource,
    ModuleUnavailable,
    UnsupportedLocal,
    DuplicateExplicit,
    MissingExport,
    NoSupportedOrigin,
    UnknownStarSurface,
    AmbiguousOrigins,
    UnsupportedNamespace,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ExportRule {
    Named,
    Star,
    ImportedAlias,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExportStep {
    pub from: ExportKey,
    pub to: ExportKey,
    pub type_only: bool,
    pub location: SourceLocation,
    pub rule: ExportRule,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportBinding {
    Symbol {
        identity: SymbolId,
        type_only: bool,
        declaration: SourceLocation,
        witness: Vec<ExportStep>,
    },
    Unresolved {
        reasons: Vec<ExportProblem>,
        candidates: Vec<SymbolId>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportResolution {
    pub key: ExportKey,
    /// Explicit syntax flag; None for names supplied solely by star exports.
    pub syntactic_type_only: Option<bool>,
    pub binding: ExportBinding,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleDependency {
    pub file: RepoPath,
    pub span: SourceSpan,
    pub kind: DependencyKind,
    pub lookup: ConfiguredLookup,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedBinding {
    pub file: RepoPath,
    pub span: SourceSpan,
    pub local: String,
    pub imported: ImportedName,
    pub type_only: bool,
    pub binding: ExportBinding,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportSurface {
    pub file: RepoPath,
    pub unknown_star_surface: bool,
    pub fingerprint: VersionedHash,
}
/// Serializable evidence only. Revalidate deserialized artifacts against captured inputs before use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleAnalysis {
    pub project: ProjectKey,
    pub version: String,
    pub config_version: String,
    pub config_hash: VersionedHash,
    pub config_inputs: Vec<ConfigDependency>,
    pub config_diagnostics: Vec<ConfigDiagnostic>,
    pub exports: Vec<ExportResolution>,
    pub imports: Vec<ImportedBinding>,
    pub dependencies: Vec<ModuleDependency>,
    pub transfers: Vec<ExportStep>,
    pub surfaces: Vec<ExportSurface>,
    pub incomplete: bool,
}
impl ModuleAnalysis {
    pub fn validate(
        &self,
        project: &ProjectKey,
        sources: &SourceSnapshot,
        files: &[FileFacts],
        config: &EffectiveConfig,
    ) -> Result<(), ResolutionError> {
        if self != &resolve_modules(project, sources, files, config)? {
            return Err(ResolutionError(
                "Module artifact differs from validated captured analysis; rebuild".into(),
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Cell {
    origins: BTreeMap<SymbolId, bool>,
    issues: BTreeSet<ExportProblem>,
}
#[derive(Clone)]
struct Anchor {
    identity: SymbolId,
    type_only: bool,
    location: SourceLocation,
}
#[derive(Clone)]
struct Star {
    from: RepoPath,
    to: RepoPath,
    type_only: bool,
    location: SourceLocation,
}
fn key(file: &RepoPath, name: &ExportName) -> ExportKey {
    ExportKey {
        file: file.clone(),
        name: name.clone(),
    }
}
fn location(file: &RepoPath, span: SourceSpan) -> SourceLocation {
    SourceLocation {
        file: file.clone(),
        span,
    }
}
fn imported_name(name: &ImportedName) -> Option<ExportName> {
    match name {
        ImportedName::Named(n) => Some(ExportName::Named(n.clone())),
        ImportedName::Default => Some(ExportName::Default),
        ImportedName::Namespace => None,
    }
}
fn digest(bytes: &[u8]) -> VersionedHash {
    VersionedHash {
        version: 1,
        digest: Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    }
}
fn spend(work: &mut usize, n: usize) -> Result<(), ResolutionError> {
    *work = work.saturating_add(n);
    if *work > MAX_WORK {
        return Err(ResolutionError(
            "Export fixed-point work budget exceeds 5000000; no partial result published".into(),
        ));
    }
    Ok(())
}
fn blocked(reason: ExportProblem) -> ExportBinding {
    ExportBinding::Unresolved {
        reasons: vec![reason],
        candidates: vec![],
    }
}
fn target(lookup: &ConfiguredLookup) -> Option<&RepoPath> {
    if let ConfigOutcome::Found { path, .. } = &lookup.outcome {
        Some(path)
    } else {
        None
    }
}
struct Engine {
    cells: BTreeMap<ExportKey, Cell>,
    explicit: BTreeSet<ExportKey>,
    anchors: BTreeMap<ExportKey, Anchor>,
    links: BTreeSet<ExportStep>,
    outgoing: BTreeMap<ExportKey, Vec<ExportStep>>,
    stars: Vec<Star>,
    unknown: BTreeSet<RepoPath>,
    work: usize,
}
impl Engine {
    fn issue(&mut self, key: &ExportKey, problem: ExportProblem) {
        self.cells
            .entry(key.clone())
            .or_default()
            .issues
            .insert(problem);
    }
    fn transfer(
        &mut self,
        from: ExportKey,
        to: ExportKey,
        type_only: bool,
        loc: SourceLocation,
        rule: ExportRule,
    ) {
        self.links.insert(ExportStep {
            from,
            to,
            type_only,
            location: loc,
            rule,
        });
    }
    fn close(&mut self) -> Result<(), ResolutionError> {
        let opaque = self.unknown.clone();
        // Discover all names/origins before judging missing/cyclic providers. No provisional
        // success is exposed while a later star branch can still add a conflicting origin.
        for round in 0..MAX_ROUNDS {
            spend(&mut self.work, self.cells.len())?;
            let before = self.cells.clone();
            let old_unknown = self.unknown.clone();
            let old_links = self.links.len();
            let mut by_file = BTreeMap::<&RepoPath, Vec<&ExportKey>>::new();
            for key in before.keys() {
                by_file.entry(&key.file).or_default().push(key);
            }
            for star in &self.stars {
                spend(&mut self.work, 1)?;
                if old_unknown.contains(&star.to) {
                    self.unknown.insert(star.from.clone());
                }
                for source in by_file
                    .get(&star.to)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|k| k.name != ExportName::Default)
                {
                    spend(&mut self.work, 1)?;
                    let dest = key(&star.from, &source.name);
                    if self.explicit.contains(&dest) {
                        continue;
                    }
                    self.cells.entry(dest.clone()).or_default();
                    self.links.insert(ExportStep {
                        from: dest,
                        to: source.clone(),
                        type_only: star.type_only,
                        location: star.location.clone(),
                        rule: ExportRule::Star,
                    });
                }
            }
            if self.cells.len() > MAX_CELLS || self.links.len() > MAX_CELLS {
                return Err(ResolutionError(
                    "Export cell/transfer budget exceeds 100000".into(),
                ));
            }
            for link in &self.links {
                spend(&mut self.work, 1)?;
                let Some(source) = before.get(&link.to) else {
                    continue;
                };
                let dest = self
                    .cells
                    .get_mut(&link.from)
                    .expect("declared export destination");
                for (id, type_only) in &source.origins {
                    spend(&mut self.work, 1)?;
                    let flag = *type_only || link.type_only;
                    dest.origins
                        .entry(id.clone())
                        .and_modify(|old| *old &= flag)
                        .or_insert(flag);
                }
            }
            if self.cells == before && self.unknown == old_unknown && self.links.len() == old_links
            {
                break;
            }
            if round + 1 == MAX_ROUNDS {
                return Err(ResolutionError(
                    "Export closure exceeds 512 rounds; no partial result published".into(),
                ));
            }
        }
        // Only now can an empty origin set be classified. Propagate uncertainty separately
        // so an empty named cycle on one star branch cannot leave a false unique winner.
        let before_keys = self.cells.keys().cloned().collect::<BTreeSet<_>>();
        let mut stars_by_from = BTreeMap::<&RepoPath, Vec<&Star>>::new();
        for star in &self.stars {
            stars_by_from.entry(&star.from).or_default().push(star);
        }
        for (key, cell) in &mut self.cells {
            if cell.origins.is_empty() {
                cell.issues.insert(ExportProblem::NoSupportedOrigin);
            }
            if cell.origins.len() > 1 {
                cell.issues.insert(ExportProblem::AmbiguousOrigins);
            }
            if !self.explicit.contains(key)
                && (opaque.contains(&key.file)
                    || stars_by_from
                        .get(&key.file)
                        .into_iter()
                        .flatten()
                        .any(|star| {
                            self.unknown.contains(&star.to)
                                && !before_keys.contains(&ExportKey {
                                    file: star.to.clone(),
                                    name: key.name.clone(),
                                })
                        }))
            {
                cell.issues.insert(ExportProblem::UnknownStarSurface);
            }
        }
        for link in &self.links {
            if !self.cells.contains_key(&link.to) {
                let reason = if self.unknown.contains(&link.to.file) {
                    ExportProblem::UnknownStarSurface
                } else {
                    ExportProblem::MissingExport
                };
                self.cells
                    .get_mut(&link.from)
                    .expect("destination")
                    .issues
                    .insert(reason);
            }
        }
        for round in 0..MAX_ROUNDS {
            spend(&mut self.work, self.cells.len())?;
            let before = self.cells.clone();
            for link in &self.links {
                spend(&mut self.work, 1)?;
                if let Some(source) = before.get(&link.to) {
                    self.cells
                        .get_mut(&link.from)
                        .expect("destination")
                        .issues
                        .extend(source.issues.iter().copied());
                }
            }
            if self.cells == before {
                return Ok(());
            }
            if round + 1 == MAX_ROUNDS {
                return Err(ResolutionError(
                    "Export uncertainty closure exceeds 512 rounds".into(),
                ));
            }
        }
        unreachable!("bounded loop returns")
    }
    fn binding(&mut self, start: &ExportKey) -> Result<ExportBinding, ResolutionError> {
        let Some(cell) = self.cells.get(start) else {
            return Ok(blocked(if self.unknown.contains(&start.file) {
                ExportProblem::UnknownStarSurface
            } else {
                ExportProblem::MissingExport
            }));
        };
        if !cell.issues.is_empty() || cell.origins.len() != 1 {
            return Ok(ExportBinding::Unresolved {
                reasons: cell.issues.iter().copied().collect(),
                candidates: cell.origins.keys().cloned().collect(),
            });
        }
        let (identity, type_only) = cell.origins.iter().next().expect("unique origin");
        let identity = identity.clone();
        let type_only = *type_only;
        // BFS through the fixed dependency relation provides a finite original-declaration
        // witness, retaining type-only path state. No recursive traversal through barrels.
        let start_state = (start.clone(), false);
        let mut queue = VecDeque::from([start_state.clone()]);
        let mut previous =
            BTreeMap::<(ExportKey, bool), Option<((ExportKey, bool), ExportStep)>>::from([(
                start_state,
                None,
            )]);
        while let Some(state) = queue.pop_front() {
            spend(&mut self.work, 1)?;
            if let Some(anchor) = self.anchors.get(&state.0)
                && anchor.identity == identity
                && (state.1 || anchor.type_only) == type_only
            {
                let mut hops = Vec::new();
                let mut end = state.clone();
                while let Some((parent, step)) = previous[&end].clone() {
                    hops.push(step);
                    end = parent;
                }
                hops.reverse();
                return Ok(ExportBinding::Symbol {
                    identity,
                    type_only,
                    declaration: anchor.location.clone(),
                    witness: hops,
                });
            }
            for step in self.outgoing.get(&state.0).into_iter().flatten() {
                spend(&mut self.work, 1)?;
                if !self
                    .cells
                    .get(&step.to)
                    .is_some_and(|c| c.origins.contains_key(&identity))
                {
                    continue;
                }
                let next = (step.to.clone(), state.1 || step.type_only);
                if !previous.contains_key(&next) {
                    previous.insert(next.clone(), Some((state.clone(), step.clone())));
                    queue.push_back(next);
                }
            }
        }
        Err(ResolutionError(
            "Internal export origin lacks a declaration witness".into(),
        ))
    }
}
/// Resolve supported configured module surfaces. The result is evidence, not CALLS edges.
pub fn resolve_modules(
    project: &ProjectKey,
    sources: &SourceSnapshot,
    files: &[FileFacts],
    config: &EffectiveConfig,
) -> Result<ModuleAnalysis, ResolutionError> {
    let resolver = ConfiguredResolver::new(config, sources.files().map(|s| s.path().clone()))?;
    let facts = exports::validate_facts(project, sources, files)?;
    let mut engine = Engine {
        cells: BTreeMap::new(),
        explicit: BTreeSet::new(),
        anchors: BTreeMap::new(),
        links: BTreeSet::new(),
        outgoing: BTreeMap::new(),
        stars: Vec::new(),
        unknown: BTreeSet::new(),
        work: 0,
    };
    let mut dependencies = Vec::new();
    let mut imports_lookup = BTreeMap::new();
    let mut reexports_lookup = BTreeMap::new();
    for f in facts.values() {
        if exports::incomplete(f) || !config.supported() {
            engine.unknown.insert(f.path.clone());
        }
        for import in &f.imports {
            let lookup = resolver.lookup(&f.path, &import.specifier);
            imports_lookup.insert((f.path.clone(), import.span), lookup.clone());
            dependencies.push(ModuleDependency {
                file: f.path.clone(),
                span: import.span,
                kind: DependencyKind::Import,
                lookup,
            });
        }
        for export in &f.exports {
            if let ExportKind::ReExport { specifier, .. } | ExportKind::Star { specifier, .. } =
                &export.kind
            {
                let lookup = resolver.lookup(&f.path, specifier);
                reexports_lookup.insert((f.path.clone(), export.span), lookup.clone());
                dependencies.push(ModuleDependency {
                    file: f.path.clone(),
                    span: export.span,
                    kind: DependencyKind::Reexport,
                    lookup,
                });
            }
        }
        let direct = exports::direct(f)
            .into_iter()
            .map(|d| (d.exported.clone(), d))
            .collect::<BTreeMap<_, _>>();
        let mut declarations = BTreeMap::<ExportName, usize>::new();
        // Local overload groups are already checked/grouped by direct(). Count the group once.
        for name in direct.keys() {
            declarations.insert(name.clone(), 1);
        }
        for e in &f.exports {
            if let ExportKind::ReExport { exported, .. } = &e.kind {
                *declarations.entry(exported.clone()).or_default() += 1;
            }
        }
        for (name, count) in declarations {
            let k = key(&f.path, &name);
            engine.explicit.insert(k.clone());
            engine.cells.entry(k.clone()).or_default();
            if count > 1 {
                engine.issue(&k, ExportProblem::DuplicateExplicit);
            }
            if !config.supported() {
                engine.issue(&k, ExportProblem::UnsupportedConfig);
            }
            if exports::incomplete(f) {
                engine.issue(&k, ExportProblem::IncompleteSource);
            }
        }
        for d in direct.values() {
            let k = key(&f.path, &d.exported);
            match &d.binding {
                BindingOutcome::Symbol { identity } => {
                    engine
                        .cells
                        .get_mut(&k)
                        .expect("direct cell")
                        .origins
                        .insert(identity.clone(), d.type_only);
                    engine.anchors.insert(
                        k,
                        Anchor {
                            identity: identity.clone(),
                            type_only: d.type_only,
                            location: location(
                                &f.path,
                                f.symbols
                                    .iter()
                                    .find(|s| s.identity == *identity)
                                    .expect("validated direct symbol")
                                    .span,
                            ),
                        },
                    );
                }
                BindingOutcome::Unresolved {
                    reason: BindingProblem::ImportedAliasDeferred,
                } => {
                    let locals = f
                        .exports
                        .iter()
                        .filter_map(|e| match &e.kind {
                            ExportKind::Local {
                                local, exported, ..
                            } if exported == &d.exported => Some(local),
                            _ => None,
                        })
                        .collect::<BTreeSet<_>>();
                    let root = f
                        .scopes
                        .iter()
                        .filter(|s| s.parent.is_none())
                        .collect::<Vec<_>>();
                    let local = locals.iter().next().copied();
                    let lexical = local.and_then(|local| {
                        if root.len() == 1 {
                            let bs = root[0]
                                .bindings
                                .iter()
                                .filter(|b| b.name == *local)
                                .collect::<Vec<_>>();
                            (bs.len() == 1 && bs[0].kind == BindingKind::Import).then_some(local)
                        } else {
                            None
                        }
                    });
                    let choices = lexical
                        .map(|local| {
                            f.imports
                                .iter()
                                .flat_map(|i| {
                                    i.bindings
                                        .iter()
                                        .filter(move |b| &b.local == local)
                                        .map(move |b| (i, b))
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    if locals.len() == 1 && choices.len() == 1 {
                        let (import, binding) = choices[0];
                        let lookup = &imports_lookup[&(f.path.clone(), import.span)];
                        match (target(lookup), imported_name(&binding.imported)) {
                            (Some(provider), Some(name)) => engine.transfer(
                                k,
                                key(provider, &name),
                                d.type_only || binding.type_only,
                                location(&f.path, d.provenance[0]),
                                ExportRule::ImportedAlias,
                            ),
                            (_, None) => engine.issue(&k, ExportProblem::UnsupportedNamespace),
                            _ => engine.issue(&k, ExportProblem::ModuleUnavailable),
                        }
                    } else {
                        engine.issue(&k, ExportProblem::UnsupportedLocal);
                    }
                }
                BindingOutcome::Unresolved { .. } => {
                    engine.issue(&k, ExportProblem::UnsupportedLocal)
                }
            }
        }
        for export in &f.exports {
            match &export.kind {
                ExportKind::ReExport {
                    imported,
                    exported,
                    type_only,
                    ..
                } => {
                    let k = key(&f.path, exported);
                    let lookup = &reexports_lookup[&(f.path.clone(), export.span)];
                    if let Some(provider) = target(lookup) {
                        engine.transfer(
                            k,
                            key(provider, imported),
                            *type_only,
                            location(&f.path, export.span),
                            ExportRule::Named,
                        );
                    } else {
                        engine.issue(&k, ExportProblem::ModuleUnavailable);
                    }
                }
                ExportKind::Star { type_only, .. } => {
                    let lookup = &reexports_lookup[&(f.path.clone(), export.span)];
                    if let Some(provider) = target(lookup) {
                        engine.stars.push(Star {
                            from: f.path.clone(),
                            to: provider.clone(),
                            type_only: *type_only,
                            location: location(&f.path, export.span),
                        });
                    } else {
                        engine.unknown.insert(f.path.clone());
                    }
                }
                _ => {}
            }
        }
    }
    engine.close()?;
    for step in &engine.links {
        engine
            .outgoing
            .entry(step.from.clone())
            .or_default()
            .push(step.clone());
    }
    let mut syntax_flags = BTreeMap::<ExportKey, bool>::new();
    for f in facts.values() {
        for e in &f.exports {
            if let ExportKind::Local {
                exported,
                type_only,
                ..
            }
            | ExportKind::ReExport {
                exported,
                type_only,
                ..
            } = &e.kind
            {
                syntax_flags
                    .entry(key(&f.path, exported))
                    .and_modify(|old| *old &= *type_only)
                    .or_insert(*type_only);
            }
        }
    }
    let keys = engine.cells.keys().cloned().collect::<Vec<_>>();
    let mut resolved = Vec::new();
    for k in keys {
        let binding = engine.binding(&k)?;
        let syntactic_type_only = syntax_flags.get(&k).copied();
        resolved.push(ExportResolution {
            key: k,
            syntactic_type_only,
            binding,
        });
    }
    let mut imports = Vec::new();
    for f in facts.values() {
        for import in &f.imports {
            let lookup = &imports_lookup[&(f.path.clone(), import.span)];
            for b in &import.bindings {
                let binding = if !config.supported() {
                    blocked(ExportProblem::UnsupportedConfig)
                } else if exports::incomplete(f) {
                    blocked(ExportProblem::IncompleteSource)
                } else if let Some(name) = imported_name(&b.imported) {
                    if let Some(provider) = target(lookup) {
                        engine.binding(&key(provider, &name))?
                    } else {
                        blocked(ExportProblem::ModuleUnavailable)
                    }
                } else {
                    blocked(ExportProblem::UnsupportedNamespace)
                };
                let type_only = b.type_only
                    || matches!(
                        &binding,
                        ExportBinding::Symbol {
                            type_only: true,
                            ..
                        }
                    );
                imports.push(ImportedBinding {
                    file: f.path.clone(),
                    span: b.span,
                    local: b.local.clone(),
                    imported: b.imported.clone(),
                    type_only,
                    binding,
                });
            }
        }
    }
    imports.sort_by(|a, b| (&a.file, a.span, &a.local).cmp(&(&b.file, b.span, &b.local)));
    dependencies.sort_by(|a, b| {
        (&a.file, a.span, a.kind, &a.lookup.specifier).cmp(&(
            &b.file,
            b.span,
            b.kind,
            &b.lookup.specifier,
        ))
    });
    let signatures = files
        .iter()
        .flat_map(|f| f.symbols.iter())
        .map(|s| (s.identity.clone(), s.signature_hash.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut exports_by_file = BTreeMap::<&RepoPath, Vec<&ExportResolution>>::new();
    for export in &resolved {
        exports_by_file
            .entry(&export.key.file)
            .or_default()
            .push(export);
    }
    let mut dependencies_by_file = BTreeMap::<&RepoPath, Vec<&ModuleDependency>>::new();
    for dependency in &dependencies {
        dependencies_by_file
            .entry(&dependency.file)
            .or_default()
            .push(dependency);
    }
    let mut surfaces = Vec::new();
    for path in facts.keys() {
        let exports = exports_by_file
            .get(path)
            .into_iter()
            .flatten()
            .map(|e| {
                let origins = engine.cells[&e.key]
                    .origins
                    .iter()
                    .map(|(id, flag)| (id, flag, &signatures[id]))
                    .collect::<Vec<_>>();
                (
                    &e.key.name,
                    e.syntactic_type_only,
                    origins,
                    &engine.cells[&e.key].issues,
                )
            })
            .collect::<Vec<_>>();
        let lookups = dependencies_by_file
            .get(path)
            .into_iter()
            .flatten()
            .map(|d| (&d.kind, &d.lookup))
            .collect::<Vec<_>>();
        let bytes = serde_json::to_vec(&(
            "ripple.external.v1",
            MODULES_VERSION,
            project,
            config.fingerprint(),
            path,
            &exports,
            engine.unknown.contains(path),
            lookups,
        ))
        .map_err(|e| ResolutionError(e.to_string()))?;
        surfaces.push(ExportSurface {
            file: path.clone(),
            unknown_star_surface: engine.unknown.contains(path),
            fingerprint: digest(&bytes),
        });
    }
    let incomplete = !config.supported()
        || !engine.unknown.is_empty()
        || files.iter().any(|f| !f.diagnostics.is_empty())
        || resolved
            .iter()
            .any(|e| matches!(e.binding, ExportBinding::Unresolved { .. }))
        || imports
            .iter()
            .any(|i| matches!(i.binding, ExportBinding::Unresolved { .. }))
        || dependencies
            .iter()
            .any(|d| matches!(d.lookup.outcome, ConfigOutcome::Unresolved { .. }));
    Ok(ModuleAnalysis {
        project: project.clone(),
        version: MODULES_VERSION.into(),
        config_version: CONFIG_VERSION.into(),
        config_hash: config.fingerprint().clone(),
        config_inputs: config.inputs().to_vec(),
        config_diagnostics: config.diagnostics().to_vec(),
        exports: resolved,
        imports,
        dependencies,
        transfers: engine.links.into_iter().collect(),
        surfaces,
        incomplete,
    })
}
