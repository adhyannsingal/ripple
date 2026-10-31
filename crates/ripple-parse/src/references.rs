//! Syntax-only imports, exports and lexical reference evidence. No resolved edges.
use crate::{
    parser::{ParsedSource, span},
    symbols,
};
use ripple_core::{
    ContractError,
    identity::ProjectKey,
    syntax::{
        BindingFact, BindingKind, Diagnostic, ExportFact, ExportKind, ExportName, FileFacts,
        ImportBinding, ImportFact, ImportedName, ReferenceFact, ReferenceKind, ReferenceOwner,
        ScopeFact, ScopeId, Severity,
    },
};
use std::collections::BTreeSet;
use tree_sitter::Node;

const MAX_DIAGNOSTICS: usize = 1024;
const MAX_DEPTH: usize = 256;
const MAX_SCOPES: usize = 8192;
const MAX_REFERENCES: usize = 100_000;
const MAX_REFERENCE_TEXT_BYTES: usize = 8 * 1024 * 1024;
fn children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}
fn token(node: Node<'_>, kind: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|n| !n.is_named() && n.kind() == kind)
}
fn function(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "function_declaration"
            | "generator_function_declaration"
            | "function_expression"
            | "generator_function"
            | "arrow_function"
            | "method_definition"
    )
}
fn declaration(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "function_declaration"
            | "generator_function_declaration"
            | "class_declaration"
            | "abstract_class_declaration"
            | "enum_declaration"
            | "internal_module"
    )
}
fn type_syntax(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "type_annotation"
            | "type_arguments"
            | "type_parameters"
            | "type_alias_declaration"
            | "interface_declaration"
            | "implements_clause"
            | "asserts_annotation"
            | "type_predicate_annotation"
            | "type_query"
            | "function_signature"
            | "method_signature"
            | "abstract_method_signature"
    )
}
struct Context<'a> {
    parsed: &'a ParsedSource,
    facts: FileFacts,
    binding_nodes: BTreeSet<usize>,
    reference_text_bytes: usize,
    text_budget_exceeded: bool,
}
impl Context<'_> {
    fn text(&self, node: Node<'_>) -> &str {
        &self.parsed.source().text()[node.byte_range()]
    }
    fn warn(&mut self, node: Node<'_>, code: &str, reason: &str) {
        if code == "reference_resource_limit" || self.facts.diagnostics.len() < MAX_DIAGNOSTICS {
            self.facts.diagnostics.push(Diagnostic {
                code: code.into(),
                severity: Severity::Warning,
                message: reason.into(),
                span: Some(span(node)),
            });
        } else if !self
            .facts
            .diagnostics
            .iter()
            .any(|d| d.code == "reference_diagnostics_truncated")
        {
            self.facts.diagnostics.push(Diagnostic {
                code: "reference_diagnostics_truncated".into(),
                severity: Severity::Warning,
                message: "Further reference extraction diagnostics omitted at the limit".into(),
                span: None,
            });
        }
    }
    fn identifier(&mut self, node: Node<'_>) -> Option<String> {
        let text = self.text(node);
        if !matches!(
            node.kind(),
            "identifier" | "type_identifier" | "shorthand_property_identifier_pattern"
        ) || text.contains('\\')
        {
            self.warn(
                node,
                "unsupported_binding_name",
                "Escaped or non-identifier binding/export names remain unsupported",
            );
            None
        } else {
            Some(text.into())
        }
    }
    fn literal(&mut self, node: Node<'_>) -> Option<String> {
        let text = self.text(node);
        if node.kind() != "string" || text.len() < 2 || text.contains('\\') {
            self.warn(
                node,
                "unsupported_module_literal",
                "Only unescaped string module specifiers are extracted",
            );
            return None;
        }
        Some(text[1..text.len() - 1].into())
    }
    fn export_name(&mut self, node: Node<'_>) -> Option<ExportName> {
        let name = self.identifier(node)?;
        Some(if name == "default" {
            ExportName::Default
        } else {
            ExportName::Named(name)
        })
    }
    fn import(&mut self, node: Node<'_>) {
        let Some(source) = node.child_by_field_name("source") else {
            self.warn(
                node,
                "unsupported_import_equals",
                "Import-equals/require bindings are not ES import facts",
            );
            return;
        };
        let Some(specifier) = self.literal(source) else {
            return;
        };
        let mut bindings = Vec::new();
        let type_only = token(node, "type");
        for child in children(node) {
            match child.kind() {
                "import_clause" => for part in children(child) {
                    match part.kind() {
                        "identifier" => if let Some(local) = self.identifier(part) {
                            bindings.push(ImportBinding { imported: ImportedName::Default, local, type_only, span: span(part) });
                        },
                        "namespace_import" => {
                            if let Some(name) = children(part).into_iter().find(|n| n.kind()=="identifier")
                                && let Some(local) = self.identifier(name) {
                                bindings.push(ImportBinding { imported: ImportedName::Namespace, local, type_only, span: span(part) });
                            }
                            self.warn(part, "unsupported_namespace_import", "Namespace imports are recorded; receiver/member resolution remains unsupported");
                        },
                        "named_imports" => for item in children(part).into_iter().filter(|n| n.kind()=="import_specifier") {
                            let Some(name) = item.child_by_field_name("name") else { continue; };
                            let Some(imported) = self.identifier(name) else { continue; };
                            let local_node = item.child_by_field_name("alias").unwrap_or(name);
                            if let Some(local) = self.identifier(local_node) {
                                bindings.push(ImportBinding {
                                    imported: if imported=="default" { ImportedName::Default } else { ImportedName::Named(imported) },
                                    local, type_only: type_only || token(item,"type"), span: span(item),
                                });
                            }
                        },
                        _ => {},
                    }
                },
                "import_attribute" => self.warn(child, "unsupported_import_attributes", "Import attributes are retained in source but their resolution semantics are unsupported"),
                _ => {},
            }
        }
        if type_only && bindings.is_empty() {
            self.warn(node, "unsupported_empty_type_import", "Empty type-only imports have no binding-level type marker; no runtime import fact emitted");
            return;
        }
        bindings.sort();
        self.facts.imports.push(ImportFact {
            specifier,
            span: span(node),
            bindings,
        });
    }
    fn export(&mut self, node: Node<'_>) {
        let source = node.child_by_field_name("source");
        let specifier = match source {
            Some(s) => match self.literal(s) {
                Some(s) => Some(s),
                None => return,
            },
            None => None,
        };
        let type_only = token(node, "type");
        let parts = children(node);
        if parts.iter().any(|n| n.kind() == "namespace_export") {
            self.warn(node, "unsupported_namespace_export", "Namespace re-exports require a namespace target outside the current export contract");
            return;
        }
        if let Some(clause) = parts.iter().find(|n| n.kind() == "export_clause") {
            if specifier.is_some()
                && !children(*clause)
                    .iter()
                    .any(|n| n.kind() == "export_specifier")
            {
                self.warn(node,"unsupported_empty_reexport","Empty re-exports load a module but have no binding in the current export contract");
            }
            for item in children(*clause)
                .into_iter()
                .filter(|n| n.kind() == "export_specifier")
            {
                let Some(name) = item.child_by_field_name("name") else {
                    continue;
                };
                let Some(exported) =
                    self.export_name(item.child_by_field_name("alias").unwrap_or(name))
                else {
                    continue;
                };
                let Some(local) = self.identifier(name) else {
                    continue;
                };
                let type_only = type_only || token(item, "type");
                let kind = if let Some(specifier) = &specifier {
                    ExportKind::ReExport {
                        specifier: specifier.clone(),
                        imported: if local == "default" {
                            ExportName::Default
                        } else {
                            ExportName::Named(local)
                        },
                        exported,
                        type_only,
                    }
                } else {
                    ExportKind::Local {
                        local,
                        exported,
                        type_only,
                    }
                };
                self.facts.exports.push(ExportFact {
                    kind,
                    span: span(item),
                });
            }
        } else if let Some(specifier) = specifier {
            self.facts.exports.push(ExportFact {
                kind: ExportKind::Star {
                    specifier,
                    type_only,
                },
                span: span(node),
            });
        } else if token(node, "default") {
            let value = node
                .child_by_field_name("declaration")
                .or_else(|| node.child_by_field_name("value"));
            if let Some(mut value) = value {
                while value.kind() == "parenthesized_expression" {
                    let parts = children(value)
                        .into_iter()
                        .filter(|n| n.kind() != "comment")
                        .collect::<Vec<_>>();
                    if parts.len() != 1 {
                        break;
                    }
                    value = parts[0];
                }
                let local = if value.kind() == "identifier" {
                    self.identifier(value)
                } else if function(value) || matches!(value.kind(), "class" | "class_declaration") {
                    value
                        .child_by_field_name("name")
                        .map_or_else(|| Some("default".into()), |n| self.identifier(n))
                } else {
                    None
                };
                if let Some(local) = local {
                    self.facts.exports.push(ExportFact {
                        kind: ExportKind::Local {
                            local,
                            exported: ExportName::Default,
                            type_only: false,
                        },
                        span: span(node),
                    });
                } else {
                    self.warn(node,"unsupported_export_expression","Default expression remains file-level evidence without a fabricated local export target");
                }
            }
        } else if let Some(decl) = node.child_by_field_name("declaration") {
            let type_only = matches!(
                decl.kind(),
                "type_alias_declaration" | "interface_declaration"
            );
            let names = if matches!(decl.kind(), "lexical_declaration" | "variable_declaration") {
                children(decl)
                    .into_iter()
                    .filter(|n| n.kind() == "variable_declarator")
                    .filter_map(|n| n.child_by_field_name("name"))
                    .flat_map(pattern_names)
                    .collect::<Vec<_>>()
            } else {
                decl.child_by_field_name("name").into_iter().collect()
            };
            if names.is_empty() {
                self.warn(
                    node,
                    "unsupported_export_declaration",
                    "Export declaration has no supported local binding",
                );
            }
            for name in names {
                // Type declaration names are type_identifier nodes, not runtime references.
                let local = if name.kind() == "type_identifier" {
                    Some(self.text(name).to_string())
                } else {
                    self.identifier(name)
                };
                if let Some(local) = local {
                    self.facts.exports.push(ExportFact {
                        kind: ExportKind::Local {
                            exported: ExportName::Named(local.clone()),
                            local,
                            type_only,
                        },
                        span: span(name),
                    });
                }
            }
        } else {
            self.warn(
                node,
                "unsupported_export_syntax",
                "Unmodeled export syntax remains explicit instead of an ES export fact",
            );
        }
    }
}
/// Only binding positions are descended; property keys, types and default values are excluded.
fn pattern_names(root: Node<'_>) -> Vec<Node<'_>> {
    let mut result = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        match node.kind() {
            "identifier" | "shorthand_property_identifier_pattern" => result.push(node),
            "assignment_pattern" => stack.extend(node.child_by_field_name("left")),
            "pair_pattern" => stack.extend(node.child_by_field_name("value")),
            "object_assignment_pattern" => stack.extend(node.child_by_field_name("left")),
            "array_pattern" | "object_pattern" | "rest_pattern" => {
                stack.extend(children(node).into_iter().rev())
            }
            _ => {}
        }
    }
    result
}
impl Context<'_> {
    fn bind(&mut self, scope: ScopeId, node: Node<'_>, kind: BindingKind) {
        for name in pattern_names(node) {
            self.binding_nodes.insert(name.id());
            let text = self.text(name).to_string();
            if text.contains('\\') {
                self.warn(
                    name,
                    "unsupported_binding_name",
                    "Escaped binding names cannot be safely compared without identifier decoding",
                );
            }
            self.facts.scopes[scope.0 as usize]
                .bindings
                .push(BindingFact {
                    name: text,
                    kind,
                    span: span(name),
                });
        }
    }
    fn bind_name(&mut self, scope: ScopeId, name: Node<'_>, kind: BindingKind) {
        if name.kind() == "type_identifier" {
            self.binding_nodes.insert(name.id());
            let text = self.text(name).to_string();
            self.facts.scopes[scope.0 as usize]
                .bindings
                .push(BindingFact {
                    name: text,
                    kind,
                    span: span(name),
                });
        } else {
            self.bind(scope, name, kind);
        }
    }
    fn new_scope(&mut self, node: Node<'_>, parent: Option<ScopeId>) -> ScopeId {
        let id = ScopeId(self.facts.scopes.len() as u32);
        self.facts.scopes.push(ScopeFact {
            id,
            parent,
            span: span(node),
            bindings: Vec::new(),
        });
        id
    }
    fn reference(&mut self, node: Node<'_>, scope: ScopeId, kind: ReferenceKind) {
        let bytes = node.end_byte() - node.start_byte();
        if bytes > MAX_REFERENCE_TEXT_BYTES.saturating_sub(self.reference_text_bytes) {
            self.text_budget_exceeded = true;
            return;
        }
        self.reference_text_bytes += bytes;
        self.facts.references.push(ReferenceFact {
            text: self.text(node).into(),
            kind,
            span: span(node),
            owner: ReferenceOwner::File,
            scope: Some(scope),
        });
    }
    fn call(&mut self, node: Node<'_>, scope: ScopeId, callees: &mut BTreeSet<usize>) {
        let constructor = node.kind() == "new_expression";
        let Some(mut target) = node.child_by_field_name(if constructor {
            "constructor"
        } else {
            "function"
        }) else {
            return;
        };
        if target.kind() == "import" {
            self.reference(node, scope, ReferenceKind::DynamicImport);
            self.warn(node,"unsupported_dynamic_import","Dynamic imports retain syntax evidence without static module resolution, including literal arguments");
            return;
        }
        while target.kind() == "parenthesized_expression" {
            let parts = children(target)
                .into_iter()
                .filter(|n| n.kind() != "comment")
                .collect::<Vec<_>>();
            if parts.len() != 1 {
                break;
            }
            target = parts[0];
        }
        let direct = target.kind() == "identifier"
            && !constructor
            && !token(node, "?.")
            && !children(node).iter().any(|n| n.kind() == "optional_chain")
            && !self.text(target).contains('\\')
            && !matches!(self.text(target), "eval" | "require");
        if direct {
            self.reference(target, scope, ReferenceKind::Call);
            callees.insert(target.id());
        } else {
            self.reference(target, scope, ReferenceKind::ComputedCall);
            self.warn(node,"unsupported_call_target","Receiver, constructor, optional, computed, eval/require or wrapped calls are not direct identifier calls");
        }
    }
    fn walk(&mut self, root: Node<'_>) {
        let module = self.new_scope(root, None);
        let mut stack = vec![(root, module, module, 0usize)];
        let mut callees = BTreeSet::new();
        while let Some((node, parent_scope, parent_var_scope, depth)) = stack.pop() {
            if self.text_budget_exceeded
                || depth > MAX_DEPTH
                || self.facts.scopes.len() > MAX_SCOPES
                || self.facts.references.len() > MAX_REFERENCES
            {
                self.facts.imports.clear();
                self.facts.exports.clear();
                self.facts.scopes.clear();
                self.facts.references.clear();
                self.warn(node,"reference_resource_limit","Reference extraction exceeds depth/scope/reference/text budget; supplemental facts withheld");
                return;
            }
            if node.is_error() || node.is_missing() || node.kind() == "comment" {
                continue;
            }
            if type_syntax(node) {
                continue;
            }
            if node.kind() == "import_statement" {
                if parent_scope != module {
                    self.warn(
                        node,
                        "unsupported_nested_module_statement",
                        "Static imports outside module scope cannot publish module import facts",
                    );
                } else if node.has_error() {
                    self.warn(
                        node,
                        "incomplete_import",
                        "Recovered import syntax does not publish complete import facts",
                    );
                } else {
                    self.import(node);
                }
                // Retain shadowing even if a module literal/import form is unsupported.
                for part in children(node) {
                    if part.kind() == "import_clause" {
                        for item in children(part) {
                            match item.kind() {
                                "identifier" => self.bind(parent_scope, item, BindingKind::Import),
                                "namespace_import" => {
                                    for n in children(item)
                                        .into_iter()
                                        .filter(|n| n.kind() == "identifier")
                                    {
                                        self.bind(parent_scope, n, BindingKind::Import);
                                    }
                                }
                                "named_imports" => {
                                    for n in children(item)
                                        .into_iter()
                                        .filter(|n| n.kind() == "import_specifier")
                                    {
                                        if let Some(name) = n
                                            .child_by_field_name("alias")
                                            .or_else(|| n.child_by_field_name("name"))
                                        {
                                            self.bind(parent_scope, name, BindingKind::Import);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    } else if part.kind() == "import_require_clause"
                        && let Some(name) = children(part)
                            .into_iter()
                            .find(|n| n.kind() == "identifier")
                    {
                        self.bind(parent_scope, name, BindingKind::Import);
                    }
                }
                continue;
            }
            if matches!(
                node.kind(),
                "ambient_declaration" | "internal_module" | "module"
            ) {
                if let Some(name) = node.child_by_field_name("name") {
                    self.bind_name(parent_scope, name, BindingKind::Local);
                }
                self.warn(
                    node,
                    "unsupported_runtime_scope",
                    "Ambient/namespace bodies are not analyzed as ordinary lexical runtime scopes",
                );
                continue;
            }
            if declaration(node)
                && let Some(name) = node.child_by_field_name("name")
            {
                self.bind_name(parent_scope, name, BindingKind::Local);
            }
            if node.kind() == "export_statement" {
                if parent_scope != module {
                    self.warn(
                        node,
                        "unsupported_nested_module_statement",
                        "Exports outside module scope cannot publish module export facts",
                    );
                } else if node.has_error() {
                    self.warn(
                        node,
                        "incomplete_export",
                        "Recovered export statement does not publish complete export facts",
                    );
                } else {
                    self.export(node);
                }
            }
            if matches!(node.kind(), "export_clause" | "namespace_export") {
                continue;
            }
            let is_function = function(node);
            let is_class = matches!(
                node.kind(),
                "class" | "class_declaration" | "abstract_class_declaration"
            );
            let creates_scope = is_function
                || is_class
                || matches!(
                    node.kind(),
                    "statement_block"
                        | "catch_clause"
                        | "for_statement"
                        | "for_in_statement"
                        | "switch_body"
                        | "class_static_block"
                );
            let scope = if creates_scope {
                self.new_scope(node, Some(parent_scope))
            } else {
                parent_scope
            };
            let function_body =
                node.kind() == "statement_block" && node.parent().is_some_and(function);
            // Parameter defaults cannot see body var declarations; nested blocks still hoist into the body.
            let var_scope = if is_function || function_body || node.kind() == "class_static_block" {
                scope
            } else {
                parent_var_scope
            };
            if is_function {
                if node.kind() != "arrow_function" {
                    self.facts.scopes[scope.0 as usize]
                        .bindings
                        .push(BindingFact {
                            name: "arguments".into(),
                            kind: BindingKind::Local,
                            span: span(node),
                        });
                }
                if let Some(name) = node.child_by_field_name("name")
                    && node.kind() != "method_definition"
                {
                    self.bind_name(scope, name, BindingKind::Local);
                }
                if let Some(parameter) = node.child_by_field_name("parameter") {
                    self.bind(scope, parameter, BindingKind::Parameter);
                }
                if let Some(parameters) = node.child_by_field_name("parameters") {
                    for parameter in children(parameters) {
                        if let Some(pattern) = parameter
                            .child_by_field_name("pattern")
                            .or_else(|| parameter.child_by_field_name("name"))
                        {
                            self.bind(scope, pattern, BindingKind::Parameter);
                        }
                    }
                }
            }
            if is_class && let Some(name) = node.child_by_field_name("name") {
                self.bind_name(scope, name, BindingKind::Local);
            }
            match node.kind() {
                "variable_declarator" => {
                    if let Some(name) = node.child_by_field_name("name") {
                        let hoisted = node
                            .parent()
                            .is_some_and(|p| p.kind() == "variable_declaration");
                        self.bind(
                            if hoisted { var_scope } else { scope },
                            name,
                            BindingKind::Local,
                        );
                    }
                }
                "catch_clause" => {
                    if let Some(parameter) = node.child_by_field_name("parameter") {
                        self.bind(scope, parameter, BindingKind::Parameter);
                    }
                }
                "for_in_statement" => {
                    if let Some(kind) = node.child_by_field_name("kind")
                        && let Some(left) = node.child_by_field_name("left")
                    {
                        self.bind(
                            if self.text(kind) == "var" {
                                var_scope
                            } else {
                                scope
                            },
                            left,
                            BindingKind::Local,
                        );
                    }
                }
                "call_expression" | "new_expression" => {
                    if node.has_error() {
                        self.warn(
                            node,
                            "incomplete_call",
                            "Recovered call syntax does not publish a supported direct call",
                        );
                        continue;
                    }
                    self.call(node, scope, &mut callees);
                }
                "with_statement" => {
                    self.warn(
                        node,
                        "unsupported_dynamic_scope",
                        "Dynamic with scope cannot be represented by static lexical bindings",
                    );
                    continue;
                }
                "identifier" | "shorthand_property_identifier" => {
                    let jsx = node.parent().is_some_and(|p| {
                        matches!(
                            p.kind(),
                            "jsx_opening_element"
                                | "jsx_self_closing_element"
                                | "jsx_closing_element"
                        )
                    });
                    let jsx_close = node
                        .parent()
                        .is_some_and(|p| p.kind() == "jsx_closing_element");
                    if !self.binding_nodes.contains(&node.id())
                        && !callees.contains(&node.id())
                        && !jsx_close
                        && (!jsx
                            || self
                                .text(node)
                                .chars()
                                .next()
                                .is_some_and(char::is_uppercase))
                    {
                        if self.text(node).contains('\\') {
                            self.warn(node,"unsupported_reference_name","Escaped identifier references require decoding and remain unsupported");
                        } else {
                            self.reference(node, scope, ReferenceKind::Read);
                        }
                    }
                }
                _ => {}
            }
            // Type assertions contain a runtime expression and type syntax; never read the type as a value.
            let mut parts = children(node);
            if matches!(node.kind(), "as_expression" | "satisfies_expression") {
                parts = parts
                    .into_iter()
                    .filter(|n| n.kind() != "comment")
                    .take(1)
                    .collect();
            } else if node.kind() == "type_assertion" {
                parts = parts
                    .into_iter()
                    .filter(|n| n.kind() != "comment")
                    .rev()
                    .take(1)
                    .collect();
            }
            for child in parts.into_iter().rev() {
                stack.push((child, scope, var_scope, depth + 1));
            }
        }
        // Also check the last visited leaf, which may have no next iteration.
        if self.text_budget_exceeded
            || self.facts.scopes.len() > MAX_SCOPES
            || self.facts.references.len() > MAX_REFERENCES
        {
            self.facts.imports.clear();
            self.facts.exports.clear();
            self.facts.scopes.clear();
            self.facts.references.clear();
            self.warn(
                root,
                "reference_resource_limit",
                "Reference extraction exceeds scope/reference/text budget; supplemental facts withheld",
            );
            return;
        }
        for scope in &mut self.facts.scopes {
            scope.bindings.sort();
            scope.bindings.dedup();
        }
    }
}
/// Extract complete supported syntax facts from captured bytes; resolution is a later stage.
pub fn extract(parsed: &ParsedSource, project: ProjectKey) -> Result<FileFacts, ContractError> {
    let facts = symbols::extract(parsed, project)?;
    if parsed.tree().is_none()
        || facts
            .diagnostics
            .iter()
            .any(|d| d.code == "extraction_resource_limit")
    {
        return Ok(facts);
    }
    let mut cx = Context {
        parsed,
        facts,
        binding_nodes: BTreeSet::new(),
        reference_text_bytes: 0,
        text_budget_exceeded: false,
    };
    // Symbol extraction already preflights every tree node against the shared budget.
    cx.walk(parsed.tree().unwrap().root_node());
    cx.facts.imports.sort();
    cx.facts.imports.dedup();
    cx.facts.exports.sort();
    cx.facts.exports.dedup();
    assign_owners(&mut cx.facts);
    cx.facts.references.sort();
    cx.facts.references.dedup();
    cx.facts.diagnostics.sort();
    cx.facts.diagnostics.dedup();
    cx.facts.validate(parsed.source())?;
    Ok(cx.facts)
}

// Sweep source-ordered intervals rather than scanning every symbol for every reference.
fn assign_owners(facts: &mut FileFacts) {
    let mut symbols = facts.symbols.iter().collect::<Vec<_>>();
    symbols.sort_by_key(|s| (s.span.start().byte, s.span.end().byte));
    facts
        .references
        .sort_by_key(|r| (r.span.start().byte, r.span.end().byte));
    let mut next = 0;
    let mut active = Vec::new();
    for reference in &mut facts.references {
        while next < symbols.len() && symbols[next].span.start().byte <= reference.span.start().byte
        {
            active.push(symbols[next]);
            next += 1;
        }
        active.retain(|s| s.span.end().byte >= reference.span.start().byte);
        reference.owner = active
            .iter()
            .filter(|s| s.span.encloses(reference.span))
            .min_by(|a, b| {
                (a.span.end().byte - a.span.start().byte, &a.identity)
                    .cmp(&(b.span.end().byte - b.span.start().byte, &b.identity))
            })
            .map_or(ReferenceOwner::File, |s| {
                ReferenceOwner::Symbol(s.identity.clone())
            });
    }
}
