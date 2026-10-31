use clap::ValueEnum;
use ripple_core::{
    identity::RepoPath,
    source::SourceSpan,
    syntax::{
        ExportFact, ImportFact, Language, ParseStatus, ReferenceFact, ReferenceOwner, ScopeFact,
        Severity, SymbolFact, VersionedHash,
    },
};
use serde::Serialize;
use std::fmt::{self, Write};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Format {
    Human,
    Json,
}
#[derive(Debug, Serialize)]
pub struct Report<T> {
    pub format_version: u32,
    pub command: &'static str,
    pub data: T,
    pub diagnostics: Vec<ReportDiagnostic>,
    pub incomplete_analysis: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ReportDiagnostic {
    pub path: RepoPath,
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub span: Option<SourceSpan>,
}
#[derive(Debug, Serialize)]
pub struct InspectData {
    pub analysis_stage: &'static str,
    pub resolution_available: bool,
    pub analyzer_version: &'static str,
    pub path: RepoPath,
    pub language: Language,
    pub is_test: bool,
    pub parse_status: ParseStatus,
    pub symbols: Vec<SymbolFact>,
    pub imports: Vec<ImportFact>,
    pub exports: Vec<ExportFact>,
    pub scopes: Vec<ScopeFact>,
    pub references: Vec<ReferenceFact>,
}
fn location(span: SourceSpan) -> String {
    format!(
        "{}:{}-{}:{}",
        span.start().line,
        span.start().column,
        span.end().line,
        span.end().column
    )
}
fn hash(hash: &Option<VersionedHash>) -> String {
    hash.as_ref().map_or_else(
        || "unavailable".into(),
        |h| format!("v{}:{}", h.version, h.digest),
    )
}
fn yes(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
// serde_json escapes ASCII controls, but leaves Unicode controls/direction markers raw.
// Human snippets use the same non-printable policy as Rust's debug strings while
// retaining valid JSON escapes and readable ordinary Unicode.
fn terminal_json(value: &impl Serialize) -> Result<String, fmt::Error> {
    let json = serde_json::to_string(value).map_err(|_| fmt::Error)?;
    let mut out = String::with_capacity(json.len());
    for ch in json.chars() {
        if !ch.is_ascii() && ch.escape_debug().next() == Some('\\') {
            let mut units = [0u16; 2];
            for unit in ch.encode_utf16(&mut units) {
                write!(out, "\\u{unit:04x}")?;
            }
        } else {
            out.push(ch);
        }
    }
    Ok(out)
}
impl Report<InspectData> {
    pub fn render(&self, format: Format) -> Result<Vec<u8>, String> {
        match format {
            Format::Json => {
                let mut bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
                bytes.push(b'\n');
                Ok(bytes)
            }
            Format::Human => self
                .human()
                .map(String::into_bytes)
                .map_err(|e| e.to_string()),
        }
    }
    fn human(&self) -> Result<String, fmt::Error> {
        let data = &self.data;
        let mut out = String::new();
        writeln!(out, "File: {:?}", data.path.as_str())?;
        writeln!(out, "Language: {:?}", data.language)?;
        writeln!(out, "Test file: {}", yes(data.is_test))?;
        writeln!(out, "Parse status: {:?}", data.parse_status)?;
        writeln!(
            out,
            "Analysis: syntax only (dependency resolution unavailable)"
        )?;
        writeln!(
            out,
            "Incomplete analysis: {}",
            yes(self.incomplete_analysis)
        )?;
        writeln!(
            out,
            "Diagnostics: {} (details on stderr)",
            self.diagnostics.len()
        )?;
        writeln!(out, "\nSymbols ({}):", data.symbols.len())?;
        for symbol in &data.symbols {
            writeln!(
                out,
                "  {:?} {:?} exported={} @ {}",
                symbol.identity.kind(),
                symbol.identity.qualified_name().display_name(),
                yes(symbol.exported),
                location(symbol.span)
            )?;
            writeln!(out, "    identity: {:?}", symbol.identity.semantic_key())?;
            writeln!(out, "    signature hash: {}", hash(&symbol.signature_hash))?;
            writeln!(out, "    body hash: {}", hash(&symbol.body_hash))?;
        }
        writeln!(out, "\nImports ({}):", data.imports.len())?;
        for import in &data.imports {
            writeln!(out, "  {:?} @ {}", import.specifier, location(import.span))?;
            for binding in &import.bindings {
                // JSON encodes tagged names without allowing source text to control the terminal.
                writeln!(
                    out,
                    "    {} as {:?} type_only={} @ {}",
                    terminal_json(&binding.imported)?,
                    binding.local,
                    yes(binding.type_only),
                    location(binding.span)
                )?;
            }
        }
        writeln!(out, "\nExports ({}):", data.exports.len())?;
        for export in &data.exports {
            writeln!(
                out,
                "  {} @ {}",
                terminal_json(&export.kind)?,
                location(export.span)
            )?;
        }
        writeln!(out, "\nScopes ({}):", data.scopes.len())?;
        for scope in &data.scopes {
            writeln!(
                out,
                "  {} parent={} @ {}",
                scope.id.0,
                scope
                    .parent
                    .map_or_else(|| "none".into(), |p| p.0.to_string()),
                location(scope.span)
            )?;
            for binding in &scope.bindings {
                writeln!(
                    out,
                    "    {:?} {:?} @ {}",
                    binding.kind,
                    binding.name,
                    location(binding.span)
                )?;
            }
        }
        writeln!(out, "\nReferences ({}):", data.references.len())?;
        for reference in &data.references {
            let owner = match &reference.owner {
                ReferenceOwner::File => "file".into(),
                ReferenceOwner::Symbol(id) => format!("symbol {:?}", id.semantic_key()),
            };
            writeln!(
                out,
                "  {:?} {:?} @ {}",
                reference.kind,
                reference.text,
                location(reference.span)
            )?;
            writeln!(
                out,
                "    owner: {owner}; scope: {}",
                reference
                    .scope
                    .map_or_else(|| "none".into(), |s| s.0.to_string())
            )?;
        }
        Ok(out)
    }
    pub fn stderr(&self) -> Result<String, fmt::Error> {
        let mut out = String::new();
        for d in &self.diagnostics {
            let at = d
                .span
                .map_or_else(String::new, |s| format!(":{}", location(s)));
            writeln!(
                out,
                "{:?}[{}] {:?}{}: {:?}",
                d.severity,
                d.code,
                d.path.as_str(),
                at,
                d.message
            )?;
        }
        Ok(out)
    }
}
