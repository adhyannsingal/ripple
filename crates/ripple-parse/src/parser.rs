//! Bounded syntax parsing of captured UTF-8 input. No semantic resolution.
use ripple_core::{
    identity::RepoPath,
    source::{SourceFile, SourcePosition, SourceSpan},
    syntax::{Diagnostic, Language, ParseStatus, Severity},
};
use std::{
    ops::ControlFlow,
    time::{Duration, Instant},
};
use tree_sitter::{Node, ParseOptions, Parser, Tree};

pub const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
#[derive(Debug, Clone, Copy)]
pub struct ParseLimits {
    pub max_bytes: usize,
    pub timeout: Duration,
}
impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            max_bytes: MAX_SOURCE_BYTES,
            timeout: Duration::from_secs(5),
        }
    }
}
#[derive(Debug, thiserror::Error)]
#[error("parse input: {0}")]
pub struct ParseError(pub String);
#[derive(Debug)]
pub struct ParsedSource {
    source: SourceFile,
    language: Language,
    tree: Option<Tree>,
    status: ParseStatus,
    diagnostics: Vec<Diagnostic>,
}
impl ParsedSource {
    pub fn source(&self) -> &SourceFile {
        &self.source
    }
    pub fn language(&self) -> Language {
        self.language
    }
    pub fn tree(&self) -> Option<&Tree> {
        self.tree.as_ref()
    }
    pub fn status(&self) -> ParseStatus {
        self.status
    }
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}
/// Tree-sitter and core both count UTF-8 bytes, with LF row advancement.
pub fn span(node: Node<'_>) -> SourceSpan {
    let a = node.start_position();
    let b = node.end_position();
    SourceSpan::new(
        SourcePosition {
            byte: node.start_byte(),
            line: a.row + 1,
            column: a.column + 1,
        },
        SourcePosition {
            byte: node.end_byte(),
            line: b.row + 1,
            column: b.column + 1,
        },
    )
    .expect("parser coordinates form a valid span")
}
pub fn parse(path: RepoPath, bytes: &[u8]) -> Result<ParsedSource, ParseError> {
    parse_with_limits(path, bytes, ParseLimits::default())
}
pub fn parse_with_limits(
    path: RepoPath,
    bytes: &[u8],
    limits: ParseLimits,
) -> Result<ParsedSource, ParseError> {
    if bytes.len() > limits.max_bytes.min(MAX_SOURCE_BYTES) {
        return Err(ParseError(format!(
            "{path}: source exceeds configured byte limit"
        )));
    }
    let text =
        std::str::from_utf8(bytes).map_err(|_| ParseError(format!("{path}: invalid UTF-8")))?;
    let language = if path.as_str().ends_with(".tsx") {
        Language::Tsx
    } else if path.as_str().ends_with(".ts") {
        Language::TypeScript
    } else {
        return Err(ParseError(format!("{path}: expected .ts or .tsx source")));
    };
    let source = SourceFile::new(path, text.into());
    let grammar = match language {
        Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        Language::Tsx => tree_sitter_typescript::LANGUAGE_TSX,
    };
    let mut parser = Parser::new();
    parser
        .set_language(&grammar.into())
        .map_err(|e| ParseError(e.to_string()))?;
    let start = Instant::now();
    let mut progress = |_: &tree_sitter::ParseState| {
        if start.elapsed() >= limits.timeout {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    };
    let tree = if limits.timeout.is_zero() {
        None
    } else {
        parser.parse_with_options(
            &mut |i, _| bytes.get(i..).unwrap_or_default(),
            None,
            Some(ParseOptions::new().progress_callback(&mut progress)),
        )
    };
    let Some(tree) = tree else {
        return Ok(ParsedSource {
            source,
            language,
            tree: None,
            status: ParseStatus::Failed,
            diagnostics: vec![Diagnostic {
                code: "parse_resource_limit".into(),
                severity: Severity::Error,
                message: "Parser stopped at its resource limit; no syntax facts are available"
                    .into(),
                span: None,
            }],
        });
    };
    let mut diagnostics = Vec::new();
    let mut cursor = tree.walk();
    loop {
        let node = cursor.node();
        if node.is_error() || node.is_missing() {
            diagnostics.push(Diagnostic {
                code: if node.is_missing() {
                    "parse_missing"
                } else {
                    "parse_error"
                }
                .into(),
                severity: Severity::Error,
                message: if node.is_missing() {
                    format!("Missing syntax token: {}", node.kind())
                } else {
                    "Unrecognized or malformed syntax; facts may be incomplete".into()
                },
                span: Some(span(node)),
            });
            if diagnostics.len() == 1024 {
                diagnostics.push(Diagnostic {
                    code: "parse_diagnostics_truncated".into(),
                    severity: Severity::Warning,
                    message: "Further parse diagnostics omitted after 1024 entries".into(),
                    span: None,
                });
                break;
            }
        }
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                diagnostics.sort();
                diagnostics.dedup();
                let status = if tree.root_node().has_error() {
                    ParseStatus::Recovered
                } else {
                    ParseStatus::Clean
                };
                drop(cursor);
                return Ok(ParsedSource {
                    source,
                    language,
                    tree: Some(tree),
                    status,
                    diagnostics,
                });
            }
        }
    }
    drop(cursor);
    diagnostics.sort();
    diagnostics.dedup();
    Ok(ParsedSource {
        source,
        language,
        tree: Some(tree),
        status: ParseStatus::Recovered,
        diagnostics,
    })
}
