//! Direct bindings only. Re-export expansion and calls are separate tasks.
use crate::{
    RESOLVER_VERSION, ResolutionError,
    modules::{ModuleLookup, ModuleOutcome, ModuleResolver},
};
use ripple_core::{
    identity::{ProjectKey, RepoPath, SymbolId, SymbolKind},
    source::{SourceSnapshot, SourceSpan},
    syntax::{
        BindingKind, ExportKind, ExportName, FileFacts, ImportedName, ParseStatus, SymbolFact,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BindingProblem {
    ModuleUnavailable,
    MissingExport,
    ReexportDeferred,
    NoSupportedSymbol,
    AmbiguousExport,
    AmbiguousLocal,
    ImportedAliasDeferred,
    TypeOnlyDeclaration,
    IncompleteProvider,
    IncompleteImporter,
    UnsupportedNamespace,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BindingOutcome {
    Symbol { identity: SymbolId },
    Unresolved { reason: BindingProblem },
}
fn unresolved(reason: BindingProblem) -> BindingOutcome {
    BindingOutcome::Unresolved { reason }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DirectExport {
    pub file: RepoPath,
    pub exported: ExportName,
    pub type_only: bool,
    pub binding: BindingOutcome,
    pub provenance: Vec<SourceSpan>,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeferredExport {
    pub file: RepoPath,
    pub kind: ExportKind,
    pub span: SourceSpan,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ImportBindingResolution {
    pub local: String,
    pub imported: ImportedName,
    pub type_only: bool,
    pub span: SourceSpan,
    pub binding: BindingOutcome,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ImportResolution {
    pub file: RepoPath,
    pub span: SourceSpan,
    pub lookup: ModuleLookup,
    pub bindings: Vec<ImportBindingResolution>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DependencyKind {
    Import,
    Reexport,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LookupDependency {
    pub file: RepoPath,
    pub span: SourceSpan,
    pub kind: DependencyKind,
    pub lookup: ModuleLookup,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResolutionDiagnostic {
    pub file: RepoPath,
    pub span: SourceSpan,
    pub code: String,
    pub reason: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleResolution {
    pub resolver_version: String,
    /// Config/module/package policy is a later stage; inventory matches alone are conditional.
    pub configuration_evaluated: bool,
    pub direct_exports: Vec<DirectExport>,
    pub deferred_exports: Vec<DeferredExport>,
    pub imports: Vec<ImportResolution>,
    pub dependencies: Vec<LookupDependency>,
    pub diagnostics: Vec<ResolutionDiagnostic>,
    pub incomplete_resolution: bool,
}
fn incomplete(facts: &FileFacts) -> bool {
    facts.parse_status != ParseStatus::Clean
        || facts.diagnostics.iter().any(|d| {
            d.code.contains("resource_limit")
                || d.code.contains("truncated")
                || d.code.starts_with("incomplete_")
                || matches!(
                    d.code.as_str(),
                    "unsupported_export_form"
                        | "unsupported_export_syntax"
                        | "unsupported_binding_name"
                        | "ambiguous_declaration"
                )
        })
}
fn direct(facts: &FileFacts) -> Vec<DirectExport> {
    let mut groups = BTreeMap::<ExportName, Vec<(&str, bool, SourceSpan)>>::new();
    for export in &facts.exports {
        if let ExportKind::Local {
            local,
            exported,
            type_only,
        } = &export.kind
        {
            groups
                .entry(exported.clone())
                .or_default()
                .push((local, *type_only, export.span));
        }
    }
    let mut symbols = BTreeMap::<&str, Vec<&SymbolFact>>::new();
    for symbol in &facts.symbols {
        let names = symbol.identity.qualified_name().components();
        if names.len() == 1 && symbol.identity.kind() != SymbolKind::Method {
            symbols.entry(&names[0]).or_default().push(symbol);
        }
    }
    let roots = facts
        .scopes
        .iter()
        .filter(|s| s.parent.is_none())
        .collect::<Vec<_>>();
    let mut locals = BTreeMap::new();
    if roots.len() == 1 {
        for binding in &roots[0].bindings {
            locals
                .entry(binding.name.as_str())
                .or_insert_with(Vec::new)
                .push(binding);
        }
    }
    groups
        .into_iter()
        .map(|(exported, items)| {
            let (local, type_only, _) = items[0];
            let mut provenance = items.iter().map(|i| i.2).collect::<Vec<_>>();
            provenance.sort();
            provenance.dedup();
            let matching = symbols.get(local).map(Vec::as_slice).unwrap_or_default();
            let binding = if incomplete(facts) || roots.len() != 1 {
                unresolved(BindingProblem::IncompleteProvider)
            } else if items.iter().any(|i| i.0 != local || i.1 != type_only) {
                unresolved(BindingProblem::AmbiguousExport)
            } else if matching.len() != 1 {
                unresolved(
                    if locals
                        .get(local)
                        .is_some_and(|bs| bs.iter().any(|b| b.kind == BindingKind::Import))
                    {
                        BindingProblem::ImportedAliasDeferred
                    } else if type_only {
                        BindingProblem::TypeOnlyDeclaration
                    } else {
                        BindingProblem::NoSupportedSymbol
                    },
                )
            } else {
                let symbol = matching[0];
                let local_bindings = locals.get(local).map(Vec::as_slice).unwrap_or_default();
                let anonymous =
                    local == "default" && symbol.identity.disambiguator() == "default-export";
                if symbol.body_span.is_none() || symbol.body_hash.is_none() {
                    unresolved(BindingProblem::IncompleteProvider)
                } else if items.len() > 1 && !items.iter().all(|i| symbol.span.encloses(i.2)) {
                    unresolved(BindingProblem::AmbiguousExport)
                } else if !anonymous
                    && (local_bindings.len() != 1 || !symbol.span.encloses(local_bindings[0].span))
                {
                    unresolved(
                        if local_bindings.iter().any(|b| b.kind == BindingKind::Import) {
                            BindingProblem::ImportedAliasDeferred
                        } else {
                            BindingProblem::AmbiguousLocal
                        },
                    )
                } else if !anonymous && local_bindings[0].kind != BindingKind::Local {
                    unresolved(BindingProblem::ImportedAliasDeferred)
                } else if !type_only && !symbol.exported {
                    unresolved(BindingProblem::IncompleteProvider)
                } else {
                    BindingOutcome::Symbol {
                        identity: symbol.identity.clone(),
                    }
                }
            };
            DirectExport {
                file: facts.path.clone(),
                exported,
                type_only: items.iter().any(|i| i.1),
                binding,
                provenance,
            }
        })
        .collect()
}
fn diagnostic(
    file: &RepoPath,
    span: SourceSpan,
    code: &str,
    reason: impl std::fmt::Debug,
) -> ResolutionDiagnostic {
    ResolutionDiagnostic {
        file: file.clone(),
        span,
        code: code.into(),
        reason: format!("{reason:?}"),
    }
}
/// Validate facts against the captured source inventory, then resolve the supported direct layer.
/// Results are binding/lookup evidence, never call edges or a complete compiler resolution claim.
pub fn resolve(
    project: &ProjectKey,
    sources: &SourceSnapshot,
    files: &[FileFacts],
) -> Result<ModuleResolution, ResolutionError> {
    let resolver = ModuleResolver::new(sources.files().map(|s| s.path().clone()))?;
    let mut facts_by_path = BTreeMap::new();
    let mut requests = 0usize;
    for facts in files {
        let source = sources
            .get(&facts.path)
            .ok_or_else(|| ResolutionError("Facts are outside captured inventory".into()))?;
        facts
            .validate(source)
            .map_err(|e| ResolutionError(e.to_string()))?;
        if facts
            .symbols
            .iter()
            .any(|s| s.identity.project() != project)
            || facts_by_path.insert(facts.path.clone(), facts).is_some()
        {
            return Err(ResolutionError(
                "Duplicate facts or cross-project symbol identity".into(),
            ));
        }
        requests = requests
            .saturating_add(facts.imports.len())
            .saturating_add(facts.exports.len());
        if requests > 100_000 {
            return Err(ResolutionError(
                "Module/export request budget exceeds 100000".into(),
            ));
        }
    }
    if facts_by_path.len() != sources.files().count() {
        return Err(ResolutionError(
            "Captured sources require exactly one validated fact record each".into(),
        ));
    }
    let mut direct_exports = Vec::new();
    let mut deferred_exports = Vec::new();
    let mut diagnostics = Vec::new();
    for facts in facts_by_path.values() {
        direct_exports.extend(direct(facts));
        deferred_exports.extend(
            facts
                .exports
                .iter()
                .filter(|e| !matches!(e.kind, ExportKind::Local { .. }))
                .map(|e| DeferredExport {
                    file: facts.path.clone(),
                    kind: e.kind.clone(),
                    span: e.span,
                }),
        );
        if incomplete(facts) {
            diagnostics.push(diagnostic(
                &facts.path,
                sources.get(&facts.path).unwrap().full_span(),
                "incomplete_source",
                facts.parse_status,
            ));
        }
    }
    direct_exports.sort();
    deferred_exports.sort();
    let exports = direct_exports
        .iter()
        .map(|e| ((e.file.clone(), e.exported.clone()), e))
        .collect::<BTreeMap<_, _>>();
    let deferred_files = deferred_exports
        .iter()
        .map(|e| e.file.clone())
        .collect::<BTreeSet<_>>();
    let mut imports = Vec::new();
    let mut dependencies = Vec::new();
    for facts in facts_by_path.values() {
        for import in &facts.imports {
            let lookup = resolver.lookup(&facts.path, &import.specifier);
            dependencies.push(LookupDependency {
                file: facts.path.clone(),
                span: import.span,
                kind: DependencyKind::Import,
                lookup: lookup.clone(),
            });
            if let ModuleOutcome::Unresolved { reason } = &lookup.outcome {
                diagnostics.push(diagnostic(
                    &facts.path,
                    import.span,
                    "module_unresolved",
                    reason,
                ));
            }
            let mut bindings = Vec::new();
            for binding in &import.bindings {
                let mut type_only = binding.type_only;
                let target = if incomplete(facts) {
                    unresolved(BindingProblem::IncompleteImporter)
                } else if matches!(binding.imported, ImportedName::Namespace) {
                    unresolved(BindingProblem::UnsupportedNamespace)
                } else if let ModuleOutcome::Found { path, .. } = &lookup.outcome {
                    let name = match &binding.imported {
                        ImportedName::Named(name) => ExportName::Named(name.clone()),
                        ImportedName::Default => ExportName::Default,
                        ImportedName::Namespace => unreachable!(),
                    };
                    if let Some(export) = exports.get(&(path.clone(), name)) {
                        type_only |= export.type_only;
                        export.binding.clone()
                    } else {
                        unresolved(if incomplete(facts_by_path[path]) {
                            BindingProblem::IncompleteProvider
                        } else if deferred_files.contains(path) {
                            BindingProblem::ReexportDeferred
                        } else {
                            BindingProblem::MissingExport
                        })
                    }
                } else {
                    unresolved(BindingProblem::ModuleUnavailable)
                };
                if let BindingOutcome::Unresolved { reason } = &target {
                    diagnostics.push(diagnostic(
                        &facts.path,
                        binding.span,
                        "binding_unresolved",
                        reason,
                    ));
                }
                bindings.push(ImportBindingResolution {
                    local: binding.local.clone(),
                    imported: binding.imported.clone(),
                    type_only,
                    span: binding.span,
                    binding: target,
                });
            }
            bindings.sort();
            imports.push(ImportResolution {
                file: facts.path.clone(),
                span: import.span,
                lookup,
                bindings,
            });
        }
        for export in &facts.exports {
            if let ExportKind::ReExport { specifier, .. } | ExportKind::Star { specifier, .. } =
                &export.kind
            {
                let lookup = resolver.lookup(&facts.path, specifier);
                if let ModuleOutcome::Unresolved { reason } = &lookup.outcome {
                    diagnostics.push(diagnostic(
                        &facts.path,
                        export.span,
                        "module_unresolved",
                        reason,
                    ));
                }
                dependencies.push(LookupDependency {
                    file: facts.path.clone(),
                    span: export.span,
                    kind: DependencyKind::Reexport,
                    lookup,
                });
                diagnostics.push(diagnostic(
                    &facts.path,
                    export.span,
                    "reexport_deferred",
                    BindingProblem::ReexportDeferred,
                ));
            }
        }
    }
    for export in &direct_exports {
        if let BindingOutcome::Unresolved { reason } = &export.binding {
            diagnostics.push(diagnostic(
                &export.file,
                export.provenance[0],
                "export_unresolved",
                reason,
            ));
        }
    }
    imports.sort();
    dependencies.sort();
    diagnostics.sort();
    diagnostics.dedup();
    let incomplete_resolution = !diagnostics.is_empty()
        || !deferred_exports.is_empty()
        || files.iter().any(|f| !f.diagnostics.is_empty());
    Ok(ModuleResolution {
        resolver_version: RESOLVER_VERSION.into(),
        configuration_evaluated: false,
        direct_exports,
        deferred_exports,
        imports,
        dependencies,
        diagnostics,
        incomplete_resolution,
    })
}
