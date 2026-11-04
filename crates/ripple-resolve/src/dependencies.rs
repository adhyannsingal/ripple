//! Selective reuse over trusted, generation-bound analysis; no IO or syntax parsing.
use crate::{
    ResolutionError,
    config::{CONFIG_VERSION, EffectiveConfig},
    paths::ConfigOutcome,
    reexports::{ImportedBinding, MODULES_VERSION, ModuleAnalysis, resolve_modules},
    symbols::{self, GraphAnalysis},
};
use ripple_core::{
    graph::{Confidence, EdgeKind},
    identity::{ProjectKey, RepoPath},
    source::SourceSnapshot,
    syntax::{FileFacts, ParseStatus},
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReuseStats {
    pub modules_reused: bool,
    pub module_files_resolved: usize,
    pub reference_files_resolved: usize,
    pub reference_files_reused: usize,
    pub invalidation_candidates: Vec<RepoPath>,
    pub unchanged_external_surfaces: Vec<RepoPath>,
}
fn normalized(file: &FileFacts, module_only: bool) -> FileFacts {
    let mut f = file.clone();
    if module_only {
        f.references.clear();
        for symbol in &mut f.symbols {
            symbol.body_hash = None;
        }
    }
    f.symbols.sort();
    f
}
fn supported(file: &FileFacts) -> bool {
    file.parse_status == ParseStatus::Clean
        && file.diagnostics.is_empty()
        && file.symbols.iter().all(|s| s.signature_hash.is_some())
}
fn bindings(modules: &ModuleAnalysis) -> BTreeMap<&RepoPath, Vec<&ImportedBinding>> {
    let mut result = BTreeMap::<_, Vec<_>>::new();
    for binding in &modules.imports {
        result.entry(&binding.file).or_default().push(binding);
    }
    result
}
/// Watch both winning and missing lookup paths; cyclic re-export/import closure terminates.
pub fn affected_files(
    seeds: &BTreeSet<RepoPath>,
    before: &ModuleAnalysis,
    after: &ModuleAnalysis,
) -> BTreeSet<RepoPath> {
    let mut watchers = BTreeMap::<&RepoPath, BTreeSet<&RepoPath>>::new();
    for modules in [before, after] {
        for dep in &modules.dependencies {
            for attempt in &dep.lookup.attempts {
                watchers.entry(&attempt.path).or_default().insert(&dep.file);
            }
            if let ConfigOutcome::Found { path, .. } = &dep.lookup.outcome {
                watchers.entry(path).or_default().insert(&dep.file);
            }
        }
    }
    let mut seen = seeds.clone();
    let mut queue = VecDeque::from_iter(seeds.iter().cloned());
    while let Some(path) = queue.pop_front() {
        if let Some(files) = watchers.get(&path) {
            for file in files {
                if seen.insert((*file).clone()) {
                    queue.push_back((*file).clone());
                }
            }
        }
    }
    seen
}
/// Previous output must come from the verified compatible producer/cache. Structural
/// contracts and integrity seals are checked by index storage, not authenticated here.
pub fn resolve_with_reuse(
    project: &ProjectKey,
    sources: &SourceSnapshot,
    files: &[FileFacts],
    config: &EffectiveConfig,
    test_files: &BTreeSet<RepoPath>,
    previous: &GraphAnalysis,
    previous_files: &[FileFacts],
) -> Result<(GraphAnalysis, ReuseStats), ResolutionError> {
    crate::exports::validate_facts(project, sources, files)?;
    let old = previous_files
        .iter()
        .map(|f| (&f.path, f))
        .collect::<BTreeMap<_, _>>();
    let new = files
        .iter()
        .map(|f| (&f.path, f))
        .collect::<BTreeMap<_, _>>();
    let compatible = previous.modules.project == *project
        && previous.modules.config_hash == *config.fingerprint()
        && previous.modules.version == MODULES_VERSION
        && previous.modules.config_version == CONFIG_VERSION;
    let same_inventory = old.keys().eq(new.keys());
    let exact = same_inventory
        && new.iter().all(|(p, f)| {
            old.get(p)
                .is_some_and(|old| normalized(old, false) == normalized(f, false))
        });
    let module_equivalent = same_inventory
        && !previous.modules.incomplete
        && new.iter().all(|(p, f)| {
            old.get(p).is_some_and(|old| {
                supported(old) && supported(f) && normalized(old, true) == normalized(f, true)
            })
        });
    let modules_reused = compatible && (exact || module_equivalent);
    let modules = if modules_reused {
        previous.modules.clone()
    } else {
        resolve_modules(project, sources, files, config)?
    };
    let old_bindings = bindings(&previous.modules);
    let new_bindings = bindings(&modules);
    let candidate_files = previous
        .graph
        .edges()
        .iter()
        .filter(|e| {
            matches!(e.kind, EdgeKind::Calls | EdgeKind::References)
                && e.confidence == Confidence::Candidate
        })
        .map(|e| e.source.path())
        .collect::<BTreeSet<_>>();
    let mut reusable = BTreeSet::new();
    let mut changed = candidate_files
        .iter()
        .map(|p| (*p).clone())
        .collect::<BTreeSet<_>>();
    for (path, file) in &new {
        if compatible
            && !candidate_files.contains(path)
            && old
                .get(path)
                .is_some_and(|old| normalized(old, false) == normalized(file, false))
            && old_bindings.get(path) == new_bindings.get(path)
        {
            reusable.insert((*path).clone());
        }
        if old
            .get(path)
            .is_none_or(|old| normalized(old, false) != normalized(file, false))
        {
            changed.insert((*path).clone());
        }
    }
    for path in old.keys() {
        if !new.contains_key(path) {
            changed.insert((*path).clone());
        }
    }
    let old_surfaces = previous
        .modules
        .surfaces
        .iter()
        .map(|s| (&s.file, s))
        .collect::<BTreeMap<_, _>>();
    let mut unchanged_external_surfaces = Vec::new();
    for surface in &modules.surfaces {
        if let Some(old) = old_surfaces.get(&surface.file) {
            if !old.unknown_star_surface
                && !surface.unknown_star_surface
                && old.fingerprint == surface.fingerprint
            {
                unchanged_external_surfaces.push(surface.file.clone());
            } else {
                changed.insert(surface.file.clone());
            }
        } else {
            changed.insert(surface.file.clone());
        }
    }
    unchanged_external_surfaces.sort();
    let invalidation_candidates = affected_files(&changed, &previous.modules, &modules)
        .into_iter()
        .collect();
    let stats = ReuseStats {
        modules_reused,
        module_files_resolved: if modules_reused { 0 } else { files.len() },
        reference_files_resolved: files.len() - reusable.len(),
        reference_files_reused: reusable.len(),
        invalidation_candidates,
        unchanged_external_surfaces,
    };
    let analysis = symbols::graph_with_modules(
        project,
        sources,
        files,
        config,
        test_files,
        modules,
        Some((&previous.graph, &reusable)),
    )?;
    Ok((analysis, stats))
}
