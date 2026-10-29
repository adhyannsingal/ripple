//! Syntax facts carry lexical evidence only; no resolved target or graph edge lives here.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    ContractError,
    identity::{RepoPath, SymbolId},
    source::{SourceFile, SourceSpan},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Language {
    TypeScript,
    Tsx,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ParseStatus {
    Clean,
    Recovered,
    Failed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Severity {
    Warning,
    Error,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub span: Option<SourceSpan>,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct VersionedHash {
    pub version: u32,
    pub digest: String,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SymbolFact {
    pub identity: SymbolId,
    pub span: SourceSpan,
    pub signature_span: Option<SourceSpan>,
    pub body_span: Option<SourceSpan>,
    pub signature_hash: Option<VersionedHash>,
    pub body_hash: Option<VersionedHash>,
    pub exported: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "name", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ImportedName {
    Named(String),
    Default,
    Namespace,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ImportBinding {
    pub imported: ImportedName,
    pub local: String,
    pub type_only: bool,
    pub span: SourceSpan,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ImportFact {
    pub specifier: String,
    pub span: SourceSpan,
    pub bindings: Vec<ImportBinding>,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "name", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExportName {
    Named(String),
    Default,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExportKind {
    Local {
        local: String,
        exported: ExportName,
        type_only: bool,
    },
    ReExport {
        specifier: String,
        imported: ExportName,
        exported: ExportName,
        type_only: bool,
    },
    Star {
        specifier: String,
        type_only: bool,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExportFact {
    pub kind: ExportKind,
    pub span: SourceSpan,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ScopeId(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BindingKind {
    Parameter,
    Local,
    Import,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BindingFact {
    pub name: String,
    pub kind: BindingKind,
    pub span: SourceSpan,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ScopeFact {
    pub id: ScopeId,
    pub parent: Option<ScopeId>,
    pub span: SourceSpan,
    pub bindings: Vec<BindingFact>,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "symbol", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReferenceOwner {
    File,
    Symbol(SymbolId),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReferenceKind {
    Call,
    Read,
    ComputedCall,
    DynamicImport,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ReferenceFact {
    pub text: String,
    pub kind: ReferenceKind,
    pub span: SourceSpan,
    pub owner: ReferenceOwner,
    pub scope: Option<ScopeId>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileFacts {
    pub path: RepoPath,
    pub language: Language,
    pub parse_status: ParseStatus,
    pub symbols: Vec<SymbolFact>,
    pub imports: Vec<ImportFact>,
    pub exports: Vec<ExportFact>,
    pub scopes: Vec<ScopeFact>,
    pub references: Vec<ReferenceFact>,
    pub diagnostics: Vec<Diagnostic>,
}
impl FileFacts {
    pub fn empty(path: RepoPath, language: Language) -> Self {
        Self {
            path,
            language,
            parse_status: ParseStatus::Clean,
            symbols: vec![],
            imports: vec![],
            exports: vec![],
            scopes: vec![],
            references: vec![],
            diagnostics: vec![],
        }
    }
    /// Validate cached/extracted facts against captured source before downstream use.
    pub fn validate(&self, source: &SourceFile) -> Result<(), ContractError> {
        if &self.path != source.path() {
            return Err(ContractError::InvalidFacts("source path mismatch"));
        }
        let check = |span: SourceSpan| span.validate_against(source.text());
        let mut symbols = BTreeMap::new();
        for symbol in &self.symbols {
            if symbol.identity.path() != &self.path
                || symbols.insert(&symbol.identity, symbol).is_some()
            {
                return Err(ContractError::InvalidFacts(
                    "duplicate or cross-file symbol identity",
                ));
            }
            check(symbol.span)?;
            for part in [symbol.signature_span, symbol.body_span]
                .into_iter()
                .flatten()
            {
                check(part)?;
                if !symbol.span.encloses(part) {
                    return Err(ContractError::InvalidFacts("symbol subspan outside owner"));
                }
            }
            for hash in [&symbol.signature_hash, &symbol.body_hash]
                .into_iter()
                .flatten()
            {
                if hash.version == 0
                    || hash.digest.len() != 64
                    || !hash
                        .digest
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err(ContractError::InvalidFacts("invalid versioned hash"));
                }
            }
        }
        if self.parse_status == ParseStatus::Failed
            && (!self.symbols.is_empty()
                || !self.imports.is_empty()
                || !self.exports.is_empty()
                || !self.scopes.is_empty()
                || !self.references.is_empty())
        {
            return Err(ContractError::InvalidFacts(
                "failed parse cannot publish syntax facts",
            ));
        }
        if self.parse_status != ParseStatus::Clean && self.diagnostics.is_empty() {
            return Err(ContractError::InvalidFacts(
                "incomplete parse requires diagnostics",
            ));
        }
        for diagnostic in &self.diagnostics {
            if diagnostic.code.is_empty() || diagnostic.message.is_empty() {
                return Err(ContractError::InvalidFacts(
                    "diagnostic requires code and reason",
                ));
            }
            if let Some(span) = diagnostic.span {
                check(span)?;
            }
        }
        for import in &self.imports {
            check(import.span)?;
            for binding in &import.bindings {
                check(binding.span)?;
                if !import.span.encloses(binding.span) {
                    return Err(ContractError::InvalidFacts(
                        "import binding outside statement",
                    ));
                }
            }
        }
        for export in &self.exports {
            check(export.span)?;
        }
        let mut scopes = BTreeMap::new();
        for scope in &self.scopes {
            check(scope.span)?;
            if scopes.insert(scope.id, scope).is_some() {
                return Err(ContractError::InvalidFacts("duplicate lexical scope"));
            }
            for binding in &scope.bindings {
                check(binding.span)?;
                if !scope.span.encloses(binding.span) {
                    return Err(ContractError::InvalidFacts("binding outside lexical scope"));
                }
            }
        }
        for scope in &self.scopes {
            let mut ancestors = BTreeSet::from([scope.id]);
            let mut child = scope;
            while let Some(parent) = child.parent {
                let parent = scopes
                    .get(&parent)
                    .ok_or(ContractError::InvalidFacts("missing parent scope"))?;
                if !ancestors.insert(parent.id) {
                    return Err(ContractError::InvalidFacts("cyclic lexical scopes"));
                }
                if !parent.span.encloses(child.span) {
                    return Err(ContractError::InvalidFacts("scope outside parent"));
                }
                child = parent;
            }
        }
        for reference in &self.references {
            check(reference.span)?;
            if let ReferenceOwner::Symbol(id) = &reference.owner {
                let owner = symbols
                    .get(id)
                    .ok_or(ContractError::InvalidFacts("reference owner missing"))?;
                if !owner.span.encloses(reference.span) {
                    return Err(ContractError::InvalidFacts(
                        "reference outside symbol owner",
                    ));
                }
            }
            if let Some(scope) = reference.scope {
                let scope = scopes
                    .get(&scope)
                    .ok_or(ContractError::InvalidFacts("reference scope missing"))?;
                if !scope.span.encloses(reference.span) {
                    return Err(ContractError::InvalidFacts(
                        "reference outside lexical scope",
                    ));
                }
            }
        }
        Ok(())
    }
}
