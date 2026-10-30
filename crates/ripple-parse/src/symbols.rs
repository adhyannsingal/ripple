//! Supported declaration facts only. Imports/references, lexical scope extraction
//! and semantic resolution are separate tasks.
use crate::{
    hashes::{MAX_SYNTAX_NODES, fingerprint},
    parser::{ParsedSource, span},
};
use ripple_core::{
    ContractError,
    identity::{ProjectKey, QualifiedName, SymbolId, SymbolKind},
    source::SourceSpan,
    syntax::{Diagnostic, FileFacts, ReferenceOwner, Severity},
};
use std::collections::{BTreeMap, BTreeSet};
use tree_sitter::Node;

const MAX_DIAGNOSTICS: usize = 1024;
struct Context<'a> {
    parsed: &'a ParsedSource,
    project: ProjectKey,
    facts: FileFacts,
    truncated: bool,
    export_shape_complete: bool,
}
impl Context<'_> {
    fn text(&self, node: Node<'_>) -> &str {
        &self.parsed.source().text()[node.byte_range()]
    }
    fn warn(&mut self, node: Node<'_>, code: &str, message: &str) {
        if self.facts.diagnostics.len() < MAX_DIAGNOSTICS {
            self.facts.diagnostics.push(Diagnostic {
                code: code.into(),
                severity: Severity::Warning,
                message: message.into(),
                span: Some(span(node)),
            });
        } else if !self.truncated {
            self.truncated = true;
            self.facts.diagnostics.push(Diagnostic {
                code: "extraction_diagnostics_truncated".into(),
                severity: Severity::Warning,
                message: "Further extraction diagnostics omitted at the diagnostic limit".into(),
                span: None,
            });
        }
    }
    fn name(&mut self, node: Node<'_>) -> Option<String> {
        let text = self.text(node);
        let value = match node.kind() {
            "identifier"
            | "type_identifier"
            | "property_identifier"
            | "private_property_identifier"
                if !text.contains('\\') =>
            {
                Some(text)
            }
            "string" if !text.contains('\\') && text.len() >= 2 => Some(&text[1..text.len() - 1]),
            _ => None,
        };
        if let Some(value) = value.filter(|s| !s.is_empty() && !s.chars().any(char::is_control)) {
            return Some(value.into());
        }
        self.warn(
            node,
            "unsupported_symbol_name",
            "Computed, numeric, escaped or empty declaration/export names are unsupported",
        );
        None
    }
    fn id(
        &self,
        names: Vec<String>,
        kind: SymbolKind,
        role: &str,
    ) -> Result<SymbolId, ContractError> {
        SymbolId::new(
            self.project.clone(),
            self.facts.path.clone(),
            QualifiedName::new(names)?,
            kind,
        )
        .with_disambiguator(role.into())
    }
}
fn children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}
fn unparenthesize(mut node: Node<'_>) -> Node<'_> {
    while node.kind() == "parenthesized_expression" {
        let mut expressions = children(node).into_iter().filter(|n| n.kind() != "comment");
        match (expressions.next(), expressions.next()) {
            (Some(expression), None) => node = expression,
            _ => break,
        }
    }
    node
}
/// Look through type-only wrappers for diagnosis, never to invent a supported target.
fn asserted_literal(mut node: Node<'_>) -> Option<Node<'_>> {
    let mut asserted = false;
    loop {
        node = unparenthesize(node);
        let child = match node.kind() {
            "as_expression" | "satisfies_expression" | "non_null_expression" => {
                children(node).into_iter().find(|n| n.kind() != "comment")
            }
            "type_assertion" => children(node)
                .into_iter()
                .rev()
                .find(|n| n.kind() != "comment"),
            _ => break,
        }?;
        asserted = true;
        node = child;
    }
    (asserted && (is_function(node) || is_class(node))).then_some(node)
}
fn token(node: Node<'_>, kind: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|child| !child.is_named() && child.kind() == kind)
}
fn is_class(node: Node<'_>) -> bool {
    node.is_named()
        && matches!(
            node.kind(),
            "class" | "class_declaration" | "abstract_class_declaration"
        )
}
fn is_function(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "function_declaration"
            | "generator_function_declaration"
            | "function_expression"
            | "generator_function"
            | "arrow_function"
            | "function_signature"
    )
}
fn role(node: Node<'_>) -> String {
    ["static", "get", "set"]
        .into_iter()
        .filter(|t| token(node, t))
        .collect::<Vec<_>>()
        .join(":")
}
fn export_role(alias: &str, type_only: bool) -> String {
    format!(
        "{}:{}",
        if type_only { "type" } else { "runtime" },
        if alias == "default" {
            "default".into()
        } else {
            format!("named:{alias}")
        }
    )
}
fn local_exports(cx: &mut Context<'_>, tops: &[Node<'_>]) -> BTreeMap<String, BTreeSet<String>> {
    let mut result = BTreeMap::<String, BTreeSet<String>>::new();
    for &top in tops {
        if top.kind() != "export_statement" || top.child_by_field_name("source").is_some() {
            continue;
        }
        if token(top, "=") || token(top, "namespace") {
            cx.export_shape_complete = false;
            cx.warn(
                top,
                "unsupported_export_form",
                "CommonJS and namespace export forms are outside supported local ES export shape",
            );
            continue;
        }
        if let Some(value) = top
            .child_by_field_name("value")
            .map(unparenthesize)
            .filter(|n| n.kind() == "identifier")
        {
            match cx.name(value) {
                Some(name) => {
                    result
                        .entry(name)
                        .or_default()
                        .insert(export_role("default", false));
                }
                None => cx.export_shape_complete = false,
            }
        }
        for clause in children(top)
            .into_iter()
            .filter(|n| n.kind() == "export_clause")
        {
            for spec in children(clause)
                .into_iter()
                .filter(|n| n.kind() == "export_specifier")
            {
                let Some(name_node) = spec.child_by_field_name("name") else {
                    continue;
                };
                let Some(name) = cx.name(name_node) else {
                    cx.export_shape_complete = false;
                    continue;
                };
                let Some(alias) = cx.name(spec.child_by_field_name("alias").unwrap_or(name_node))
                else {
                    cx.export_shape_complete = false;
                    result.entry(name).or_default().insert(export_role(
                        "unsupported-alias",
                        token(top, "type") || token(spec, "type"),
                    ));
                    continue;
                };
                result.entry(name).or_default().insert(export_role(
                    &alias,
                    token(top, "type") || token(spec, "type"),
                ));
            }
        }
    }
    result
}
#[derive(Clone)]
struct Declaration<'a> {
    id: SymbolId,
    syntax: Node<'a>,
    callable: Node<'a>,
    extent: Node<'a>,
    exports: BTreeSet<String>,
    ordinal: usize,
}
fn top_declarations<'a>(
    cx: &mut Context<'_>,
    tops: &[Node<'a>],
    aliases: &BTreeMap<String, BTreeSet<String>>,
) -> Result<Vec<Declaration<'a>>, ContractError> {
    let mut result = Vec::new();
    for (ordinal, &top) in tops.iter().enumerate() {
        let exported = top.kind() == "export_statement";
        let default = exported && token(top, "default");
        let node = if exported {
            match top
                .child_by_field_name("declaration")
                .or_else(|| top.child_by_field_name("value"))
            {
                Some(n) => n,
                None => continue,
            }
        } else {
            top
        };
        let node = unparenthesize(node);
        if is_class(node) || is_function(node) {
            let kind = if is_class(node) {
                SymbolKind::Class
            } else {
                SymbolKind::Function
            };
            let (name, role) = match node.child_by_field_name("name") {
                Some(n) => match cx.name(n) {
                    Some(s) => (s, String::new()),
                    None => {
                        if exported {
                            cx.export_shape_complete = false;
                        }
                        continue;
                    }
                },
                None if default => ("default".into(), "default-export".into()),
                _ => {
                    cx.warn(
                        node,
                        "unsupported_anonymous_declaration",
                        "Only anonymous default exports have supported stable declaration roles",
                    );
                    continue;
                }
            };
            let mut exports = aliases.get(&name).cloned().unwrap_or_default();
            if exported {
                exports.insert(export_role(if default { "default" } else { &name }, false));
            }
            result.push(Declaration {
                id: cx.id(vec![name], kind, &role)?,
                syntax: top,
                callable: node,
                extent: top,
                exports,
                ordinal,
            });
        } else if matches!(node.kind(), "lexical_declaration" | "variable_declaration") {
            let constant = token(node, "const");
            for declarator in children(node)
                .into_iter()
                .filter(|n| n.kind() == "variable_declarator")
            {
                let Some(value) = declarator.child_by_field_name("value") else {
                    continue;
                };
                let value = unparenthesize(value);
                if let Some(literal) = asserted_literal(value) {
                    let exposed = exported
                        || declarator.child_by_field_name("name").is_some_and(|name| {
                            name.kind() == "identifier" && aliases.contains_key(cx.text(name))
                        });
                    if exposed {
                        cx.export_shape_complete = false;
                    }
                    let code = if is_class(literal) {
                        "unsupported_class_binding"
                    } else {
                        "unsupported_function_binding"
                    };
                    cx.warn(declarator, code, "Assertion/non-null wrappers around function or class literals are not supported declaration targets");
                    continue;
                }
                if is_function(value) {
                    let Some(name_node) = declarator.child_by_field_name("name") else {
                        continue;
                    };
                    if !constant || name_node.kind() != "identifier" {
                        if exported {
                            cx.export_shape_complete = false;
                        }
                        cx.warn(declarator,"unsupported_function_binding","Only top-level const identifier bindings to function/arrow expressions are supported");
                        continue;
                    }
                    let Some(name) = cx.name(name_node) else {
                        if exported {
                            cx.export_shape_complete = false;
                        }
                        continue;
                    };
                    let mut exports = aliases.get(&name).cloned().unwrap_or_default();
                    if exported {
                        exports.insert(export_role(&name, false));
                    }
                    result.push(Declaration {
                        id: cx.id(vec![name], SymbolKind::Function, "")?,
                        syntax: declarator,
                        callable: value,
                        extent: declarator,
                        exports,
                        ordinal,
                    });
                } else if is_class(value) {
                    if exported {
                        cx.export_shape_complete = false;
                    }
                    cx.warn(declarator,"unsupported_class_binding","Class expressions in variable bindings are outside initial declaration support");
                }
            }
        } else if default && node.kind() != "identifier" {
            cx.export_shape_complete = false;
            cx.warn(top,"unsupported_default_export","Default expressions without a supported declaration or identifier remain file-level syntax");
        } else if matches!(
            node.kind(),
            "ambient_declaration"
                | "internal_module"
                | "module"
                | "enum_declaration"
                | "interface_declaration"
                | "type_alias_declaration"
        ) {
            if exported {
                cx.export_shape_complete = false;
            }
            cx.warn(node,"unsupported_declaration","Ambient, namespace, enum and type-only declarations are not executable symbol targets");
        }
    }
    Ok(result)
}
fn methods<'a>(
    cx: &mut Context<'_>,
    class: &Declaration<'a>,
) -> Result<Vec<Declaration<'a>>, ContractError> {
    let Some(body) = class.callable.child_by_field_name("body") else {
        return Ok(vec![]);
    };
    let mut result = Vec::new();
    for (ordinal, node) in children(body)
        .into_iter()
        .filter(|n| n.kind() != "comment")
        .enumerate()
    {
        if matches!(
            node.kind(),
            "method_definition" | "method_signature" | "abstract_method_signature"
        ) {
            let Some(name_node) = node.child_by_field_name("name") else {
                continue;
            };
            let Some(name) = cx.name(name_node) else {
                continue;
            };
            let mut names = class.id.qualified_name().components().to_vec();
            names.push(name.clone());
            let private = name.starts_with('#')
                || children(node).iter().any(|n| {
                    n.kind() == "accessibility_modifier"
                        && matches!(cx.text(*n), "private" | "protected")
                });
            result.push(Declaration {
                id: cx.id(names, SymbolKind::Method, &role(node))?,
                syntax: node,
                callable: node,
                extent: node,
                exports: if private {
                    BTreeSet::new()
                } else {
                    class.exports.clone()
                },
                ordinal,
            });
        } else if node.kind() == "public_field_definition"
            && node.child_by_field_name("value").is_some()
        {
            cx.warn(node,"unsupported_field_initializer","Field initializers are retained in class fingerprints; separate field/function targets are unsupported");
        } else if matches!(node.kind(), "class_static_block" | "index_signature") {
            cx.warn(node,"unsupported_class_member","Static initialization and index signatures are retained conservatively in class fingerprints");
        }
    }
    Ok(result)
}
fn class_body_exclusions(
    class: Node<'_>,
    members: &[ripple_core::syntax::SymbolFact],
) -> BTreeSet<usize> {
    let supported = members
        .iter()
        .filter_map(|m| m.body_span)
        .map(|s| (s.start().byte, s.end().byte))
        .collect::<BTreeSet<_>>();
    class
        .child_by_field_name("body")
        .into_iter()
        .flat_map(children)
        .filter(|n| n.kind() == "method_definition")
        .filter_map(|n| n.child_by_field_name("body"))
        .filter(|n| supported.contains(&(n.start_byte(), n.end_byte())))
        .map(|n| n.id())
        .collect()
}
fn emit(cx: &mut Context<'_>, declarations: Vec<Declaration<'_>>) -> Result<(), ContractError> {
    let mut groups = BTreeMap::<SymbolId, Vec<Declaration<'_>>>::new();
    for declaration in declarations {
        groups
            .entry(declaration.id.clone())
            .or_default()
            .push(declaration);
    }
    for (id, mut group) in groups {
        group.sort_by_key(|d| d.extent.start_byte());
        let class = id.kind() == SymbolKind::Class;
        let implementations = group
            .iter()
            .filter(|d| d.callable.child_by_field_name("body").is_some())
            .collect::<Vec<_>>();
        if implementations.len() != 1 || (class && group.len() != 1) {
            let code = if implementations.is_empty() {
                "unsupported_ambient_declaration"
            } else {
                "ambiguous_declaration"
            };
            cx.warn(group[0].extent, code, "A supported logical declaration requires exactly one implementation; duplicate/ambient-only groups have no invented target");
            continue;
        }
        let implementation = (*implementations[0]).clone();
        let body = implementation
            .callable
            .child_by_field_name("body")
            .expect("implementation body");
        let last = group.last().expect("nonempty group");
        let contiguous = group.windows(2).all(|p| p[1].ordinal == p[0].ordinal + 1);
        if group.len() > 1
            && (!contiguous
                || last.callable.id() != implementation.callable.id()
                || group[..group.len() - 1].iter().any(|d| {
                    !matches!(
                        d.callable.kind(),
                        "function_signature" | "method_signature" | "abstract_method_signature"
                    )
                }))
        {
            cx.warn(
                group[0].extent,
                "unsupported_overload_group",
                "Overload signatures must be consecutive and precede their implementation",
            );
            continue;
        }
        if group.iter().any(|d| d.syntax.has_error()) {
            cx.warn(
                implementation.extent,
                "unhashable_declaration",
                "Recovered declaration syntax is not published as a supported symbol",
            );
            continue;
        }
        let exports = group
            .iter()
            .flat_map(|d| d.exports.iter().cloned())
            .collect::<BTreeSet<_>>();
        let roots = group.iter().map(|d| d.syntax).collect::<Vec<_>>();
        let member_start = cx.facts.symbols.len();
        if class {
            let members = methods(cx, &implementation)?;
            emit(cx, members)?;
        }
        let excluded = if class {
            class_body_exclusions(implementation.callable, &cx.facts.symbols[member_start..])
        } else {
            BTreeSet::from([body.id()])
        };
        let metadata = exports
            .iter()
            .cloned()
            .chain(std::iter::once(format!(
                "role:{}",
                role(implementation.callable)
            )))
            .collect::<Vec<_>>();
        let signature_hash = if cx.export_shape_complete {
            fingerprint(
                "signature",
                cx.parsed.language(),
                cx.parsed.source().text(),
                &roots,
                &excluded,
                &metadata,
            )
        } else {
            None
        };
        let empty_exclusions = BTreeSet::new();
        let body_hash = fingerprint(
            if class { "class_shape" } else { "body" },
            cx.parsed.language(),
            cx.parsed.source().text(),
            &[body],
            if class { &excluded } else { &empty_exclusions },
            &[],
        );
        if signature_hash.is_none() || body_hash.is_none() {
            cx.warn(implementation.extent, "unhashable_declaration", "Unsupported export shape, tree input or traversal limit prevents a complete fingerprint");
        }
        let location = SourceSpan::new(
            span(group[0].extent).start(),
            span(implementation.extent).end(),
        )?;
        let signature_span = SourceSpan::new(location.start(), span(body).start())?;
        cx.facts.symbols.push(ripple_core::syntax::SymbolFact {
            identity: id,
            span: location,
            signature_span: Some(signature_span),
            body_span: Some(span(body)),
            signature_hash,
            body_hash,
            exported: exports.iter().any(|s| s.starts_with("runtime:")),
        });
    }
    Ok(())
}
/// Return the smallest supported enclosing symbol; nested callbacks introduce no new symbol IDs.
pub fn owner_for_span(facts: &FileFacts, location: SourceSpan) -> ReferenceOwner {
    facts
        .symbols
        .iter()
        .filter(|s| s.span.encloses(location))
        .min_by(|a, b| {
            (a.span.end().byte - a.span.start().byte)
                .cmp(&(b.span.end().byte - b.span.start().byte))
                .then_with(|| a.identity.cmp(&b.identity))
        })
        .map_or(ReferenceOwner::File, |s| {
            ReferenceOwner::Symbol(s.identity.clone())
        })
}
/// Produce symbol-only FileFacts. M1-T04 adds imports, exports, scopes and references.
pub fn extract(parsed: &ParsedSource, project: ProjectKey) -> Result<FileFacts, ContractError> {
    let mut facts = FileFacts::empty(parsed.source().path().clone(), parsed.language());
    facts.parse_status = parsed.status();
    facts.diagnostics = parsed.diagnostics().to_vec();
    let mut cx = Context {
        parsed,
        project,
        facts,
        truncated: false,
        export_shape_complete: parsed.status() == ripple_core::syntax::ParseStatus::Clean,
    };
    if let Some(tree) = parsed.tree() {
        let mut cursor = tree.walk();
        let mut visited = 0;
        let mut classes = Vec::new();
        'tree: loop {
            let node = cursor.node();
            visited += 1;
            if visited > MAX_SYNTAX_NODES {
                cx.facts.symbols.clear();
                cx.warn(
                    node,
                    "extraction_resource_limit",
                    "Syntax traversal exceeds the supported node budget; no symbols published",
                );
                cx.facts.diagnostics.sort();
                cx.facts.diagnostics.dedup();
                cx.facts.validate(parsed.source())?;
                return Ok(cx.facts);
            }
            if node.kind() == "comment"
                && ["@jsx", "@ts-", "<reference", "<amd-"]
                    .iter()
                    .any(|marker| cx.text(node).contains(marker))
            {
                cx.export_shape_complete = false;
                cx.warn(node, "unsupported_compiler_directive", "Directive-like comments may affect compiler behavior; complete external fingerprints are withheld");
            }
            if node.kind() == "decorator" {
                cx.warn(
                    node,
                    "unsupported_decorator",
                    "Decorator syntax is retained but hidden runtime bindings are not modeled",
                );
            }
            if is_class(node) {
                classes.push(node);
            }
            if cursor.goto_first_child() {
                continue;
            }
            loop {
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    break 'tree;
                }
            }
        }
        let tops = children(tree.root_node())
            .into_iter()
            .filter(|n| n.kind() != "comment")
            .collect::<Vec<_>>();
        let aliases = local_exports(&mut cx, &tops);
        let declarations = top_declarations(&mut cx, &tops, &aliases)?;
        let represented = declarations
            .iter()
            .filter(|d| d.id.kind() == SymbolKind::Class)
            .map(|d| d.callable.id())
            .collect::<BTreeSet<_>>();
        for class in classes
            .into_iter()
            .filter(|n| !represented.contains(&n.id()))
        {
            cx.warn(
                class,
                "unsupported_nested_class",
                "Only top-level class declarations and default class exports are symbol targets",
            );
        }
        emit(&mut cx, declarations)?;
    }
    cx.facts.symbols.sort_by(|a, b| a.identity.cmp(&b.identity));
    cx.facts.diagnostics.sort();
    cx.facts.diagnostics.dedup();
    cx.facts.validate(parsed.source())?;
    Ok(cx.facts)
}
