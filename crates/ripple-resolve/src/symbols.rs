//! Supported lexical calls/references. Receiver inference and flow analysis are unsupported.
use crate::{
    ResolutionError,
    config::EffectiveConfig,
    exports::{self, DependencyKind},
    paths::ConfigOutcome,
    reexports::{ExportBinding, ModuleAnalysis, resolve_modules},
};
use ripple_core::{
    graph::*,
    identity::{ProjectKey, RepoPath, SymbolId, SymbolKind},
    source::{SourceSnapshot, SourceSpan},
    syntax::{BindingKind, FileFacts, ReferenceKind, ReferenceOwner, ScopeId},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub const GRAPH_VERSION: &str = "ripple-graph-v1";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphAnalysis {
    pub graph: GraphSnapshot,
    pub modules: ModuleAnalysis,
    pub incomplete: bool,
}
fn loc(file: &RepoPath, span: SourceSpan) -> SourceLocation {
    SourceLocation {
        file: file.clone(),
        span,
    }
}
fn edge(
    source: NodeId,
    target: NodeId,
    kind: EdgeKind,
    location: SourceLocation,
    rule: &str,
) -> Edge {
    Edge {
        source,
        target,
        kind,
        confidence: Confidence::Resolved,
        provenance: location,
        resolution_rule: rule.into(),
    }
}
fn lexical_target(
    f: &FileFacts,
    r: &ripple_core::syntax::ReferenceFact,
    scopes: &BTreeMap<ScopeId, &ripple_core::syntax::ScopeFact>,
    bindings_by_scope: &BTreeMap<(ScopeId, &str), Vec<&ripple_core::syntax::BindingFact>>,
    imports: &BTreeMap<&str, Vec<&crate::reexports::ImportedBinding>>,
    locals: &BTreeMap<&str, Vec<&ripple_core::syntax::SymbolFact>>,
) -> Result<(SymbolId, &'static str), &'static str> {
    if !matches!(r.kind, ReferenceKind::Call | ReferenceKind::Read) {
        return Err("unsupported receiver/computed/constructor/dynamic target");
    }
    if exports::incomplete(f) {
        return Err("incomplete source cannot prove a lexical target");
    }
    if r.text.is_empty()
        || !r
            .text
            .chars()
            .all(|c| c == '_' || c == '$' || c.is_alphanumeric())
    {
        return Err("unsupported identifier spelling");
    }
    let mut current = r.scope;
    let mut visited = BTreeSet::<ScopeId>::new();
    while let Some(id) = current {
        if !visited.insert(id) {
            return Err("cyclic lexical scope");
        }
        let scope = scopes.get(&id).ok_or("missing lexical scope")?;
        let bindings = bindings_by_scope
            .get(&(id, r.text.as_str()))
            .map(Vec::as_slice)
            .unwrap_or_default();
        if !bindings.is_empty() {
            if bindings.len() != 1 {
                return Err("ambiguous lexical bindings");
            }
            let b = bindings[0];
            return match b.kind {
                BindingKind::Parameter => Err("parameter shadows supported target"),
                BindingKind::Import => {
                    let matches = imports
                        .get(r.text.as_str())
                        .map(Vec::as_slice)
                        .unwrap_or_default();
                    if matches.len() != 1 {
                        return Err("missing or ambiguous import binding");
                    }
                    let import = matches[0];
                    if import.type_only {
                        return Err("type-only import is not runtime evidence");
                    }
                    match &import.binding {
                        ExportBinding::Symbol { identity, .. } => {
                            Ok((identity.clone(), "configured_import_binding"))
                        }
                        _ => Err("import binding is unresolved"),
                    }
                }
                BindingKind::Local => {
                    let symbols = locals
                        .get(r.text.as_str())
                        .into_iter()
                        .flatten()
                        .filter(|s| s.span.encloses(b.span) && s.body_span.is_some())
                        .collect::<Vec<_>>();
                    if symbols.len() != 1 {
                        return Err("local binding has no unique supported declaration");
                    }
                    Ok((symbols[0].identity.clone(), "lexical_local_declaration"))
                }
            };
        }
        current = scope.parent;
    }
    Err("no supported lexical binding")
}
pub fn resolve_graph(
    project: &ProjectKey,
    sources: &SourceSnapshot,
    files: &[FileFacts],
    config: &EffectiveConfig,
    test_files: &BTreeSet<RepoPath>,
) -> Result<GraphAnalysis, ResolutionError> {
    if test_files.iter().any(|p| sources.get(p).is_none()) {
        return Err(ResolutionError(
            "Test classification outside captured source inventory".into(),
        ));
    }
    let modules = resolve_modules(project, sources, files, config)?;
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut unresolved = Vec::new();
    for source in sources.files() {
        nodes.push(GraphNode::file(
            project.clone(),
            source.path().clone(),
            test_files.contains(source.path()),
        ));
    }
    let mut imports_by_file = BTreeMap::<&RepoPath, Vec<&crate::reexports::ImportedBinding>>::new();
    for import in &modules.imports {
        imports_by_file
            .entry(&import.file)
            .or_default()
            .push(import);
    }
    let mut sorted = files.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|f| &f.path);
    for f in sorted {
        let scopes = f
            .scopes
            .iter()
            .map(|s| (s.id, s))
            .collect::<BTreeMap<_, _>>();
        let mut bindings_by_scope =
            BTreeMap::<(ScopeId, &str), Vec<&ripple_core::syntax::BindingFact>>::new();
        for scope in &f.scopes {
            for b in &scope.bindings {
                bindings_by_scope
                    .entry((scope.id, b.name.as_str()))
                    .or_default()
                    .push(b);
            }
        }
        let mut imports = BTreeMap::<&str, Vec<&crate::reexports::ImportedBinding>>::new();
        for import in imports_by_file.get(&f.path).into_iter().flatten() {
            imports
                .entry(import.local.as_str())
                .or_default()
                .push(import);
        }
        let mut locals = BTreeMap::<&str, Vec<&ripple_core::syntax::SymbolFact>>::new();
        for symbol in &f.symbols {
            if symbol.identity.qualified_name().components().len() == 1 {
                locals
                    .entry(&symbol.identity.qualified_name().components()[0])
                    .or_default()
                    .push(symbol);
            }
        }

        let file = NodeId::file(project.clone(), f.path.clone());
        for s in &f.symbols {
            nodes.push(GraphNode::symbol(s.identity.clone(), s.span));
            edges.push(edge(
                file.clone(),
                NodeId::Symbol(s.identity.clone()),
                EdgeKind::Defines,
                loc(&f.path, s.span),
                "syntax_defines",
            ));
        }
        let mut refs = f.references.clone();
        refs.sort();
        refs.dedup();
        for (ordinal, r) in refs.iter().enumerate() {
            let source = match &r.owner {
                ReferenceOwner::File => file.clone(),
                ReferenceOwner::Symbol(s) => NodeId::Symbol(s.clone()),
            };
            let resolved = if config.supported() {
                lexical_target(f, r, &scopes, &bindings_by_scope, &imports, &locals)
            } else {
                Err("unsupported effective configuration")
            };
            match resolved {
                Ok((id, rule))
                    if r.kind != ReferenceKind::Call || id.kind() == SymbolKind::Function =>
                {
                    edges.push(edge(
                        source,
                        NodeId::Symbol(id),
                        if r.kind == ReferenceKind::Call {
                            EdgeKind::Calls
                        } else {
                            EdgeKind::References
                        },
                        loc(&f.path, r.span),
                        rule,
                    ))
                }
                other => {
                    let reason = match other {
                        Err(reason) => reason,
                        Ok(_) => "class invocation is not a supported function call",
                    };
                    let boundary = NodeId::Opaque {
                        project: project.clone(),
                        path: f.path.clone(),
                        reference_key: format!(
                            "reference:{ordinal}:{}:{}",
                            r.span.start().byte,
                            r.span.end().byte
                        ),
                    };
                    nodes.push(GraphNode {
                        id: boundary.clone(),
                        kind: NodeKind::OpaqueBoundary,
                        span: Some(r.span),
                    });
                    unresolved.push(UnresolvedReference {
                        source,
                        boundary,
                        text: r.text.clone(),
                        reference_kind: r.kind,
                        provenance: loc(&f.path, r.span),
                        reason: reason.into(),
                    });
                }
            }
            if nodes.len() + edges.len() + unresolved.len() > 1_000_000 {
                return Err(ResolutionError(
                    "Resolved graph exceeds one million records".into(),
                ));
            }
        }
    }
    for (ordinal, d) in modules.dependencies.iter().enumerate() {
        let source = NodeId::file(project.clone(), d.file.clone());
        if let ConfigOutcome::Found { path, .. } = &d.lookup.outcome {
            edges.push(edge(
                source,
                NodeId::file(project.clone(), path.clone()),
                if d.kind == DependencyKind::Import {
                    EdgeKind::Imports
                } else {
                    EdgeKind::Reexports
                },
                loc(&d.file, d.span),
                "configured_module_lookup",
            ));
        } else {
            let boundary = NodeId::Opaque {
                project: project.clone(),
                path: d.file.clone(),
                reference_key: format!("module:{ordinal}"),
            };
            nodes.push(GraphNode {
                id: boundary.clone(),
                kind: NodeKind::OpaqueBoundary,
                span: Some(d.span),
            });
            unresolved.push(UnresolvedReference {
                source,
                boundary,
                text: if d.lookup.specifier.is_empty() {
                    "module specifier: empty string".into()
                } else {
                    d.lookup.specifier.clone()
                },
                reference_kind: ReferenceKind::Read,
                provenance: loc(&d.file, d.span),
                reason: format!("module lookup: {:?}", d.lookup.outcome),
            });
        }
    }
    if nodes.len() + edges.len() + unresolved.len() > 1_000_000 {
        return Err(ResolutionError(
            "Resolved graph exceeds one million records".into(),
        ));
    }
    edges.sort();
    edges.dedup();
    let incomplete = modules.incomplete || !unresolved.is_empty();
    let graph =
        GraphSnapshot::new(nodes, edges, unresolved).map_err(|e| ResolutionError(e.to_string()))?;
    Ok(GraphAnalysis {
        graph,
        modules,
        incomplete,
    })
}
