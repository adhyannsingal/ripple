use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{ContractError, identity::RepoPath};

/// Display coordinates are one-based; byte offsets and columns count UTF-8 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SourcePosition {
    pub byte: usize,
    pub line: usize,
    pub column: usize,
}
impl SourcePosition {
    pub fn at(source: &str, byte: usize) -> Result<Self, ContractError> {
        if !source.is_char_boundary(byte) {
            return Err(ContractError::InvalidSpan(
                "offset outside source or inside UTF-8 character",
            ));
        }
        let before = &source.as_bytes()[..byte];
        let line = before.iter().filter(|&&b| b == b'\n').count() + 1;
        let column = before
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(byte + 1, |i| byte - i);
        Ok(Self { byte, line, column })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "RawSpan")]
pub struct SourceSpan {
    start: SourcePosition,
    end: SourcePosition,
}
#[derive(Deserialize)]
struct RawSpan {
    start: SourcePosition,
    end: SourcePosition,
}
impl TryFrom<RawSpan> for SourceSpan {
    type Error = ContractError;
    fn try_from(value: RawSpan) -> Result<Self, Self::Error> {
        Self::new(value.start, value.end)
    }
}
impl SourceSpan {
    pub fn new(start: SourcePosition, end: SourcePosition) -> Result<Self, ContractError> {
        if start.line == 0
            || end.line == 0
            || start.column == 0
            || end.column == 0
            || start.byte > end.byte
            || start.line > end.line
            || (start.line == end.line && start.column > end.column)
            || (start.byte == end.byte && start != end)
        {
            return Err(ContractError::InvalidSpan("inconsistent range/coordinates"));
        }
        Ok(Self { start, end })
    }
    pub fn from_offsets(source: &str, start: usize, end: usize) -> Result<Self, ContractError> {
        Self::new(
            SourcePosition::at(source, start)?,
            SourcePosition::at(source, end)?,
        )
    }
    pub fn start(self) -> SourcePosition {
        self.start
    }
    pub fn end(self) -> SourcePosition {
        self.end
    }
    pub fn validate_against(self, source: &str) -> Result<(), ContractError> {
        if self != Self::from_offsets(source, self.start.byte, self.end.byte)? {
            return Err(ContractError::InvalidSpan(
                "coordinates disagree with source",
            ));
        }
        Ok(())
    }
    pub fn contains_byte(self, byte: usize) -> bool {
        self.start.byte <= byte && byte < self.end.byte
    }
    pub fn encloses(self, other: Self) -> bool {
        self.start.byte <= other.start.byte && self.end.byte >= other.end.byte
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    path: RepoPath,
    text: String,
}
impl SourceFile {
    pub fn new(path: RepoPath, text: String) -> Self {
        Self { path, text }
    }
    pub fn path(&self) -> &RepoPath {
        &self.path
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn full_span(&self) -> SourceSpan {
        SourceSpan::from_offsets(&self.text, 0, self.text.len())
            .expect("UTF-8 source has valid full span")
    }
}

/// Captured bytes, not a live filesystem handle; capture/verification belongs to index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSnapshot {
    files: BTreeMap<RepoPath, SourceFile>,
}
impl SourceSnapshot {
    pub fn new(files: impl IntoIterator<Item = SourceFile>) -> Result<Self, ContractError> {
        let mut captured = BTreeMap::new();
        for file in files {
            let path = file.path.clone();
            if captured.insert(path.clone(), file).is_some() {
                return Err(ContractError::DuplicateSource(path));
            }
        }
        Ok(Self { files: captured })
    }
    pub fn files(&self) -> impl Iterator<Item = &SourceFile> {
        self.files.values()
    }
    pub fn get(&self, path: &RepoPath) -> Option<&SourceFile> {
        self.files.get(path)
    }
}

/// Fallible provider contract; callers capture inputs once before analysis.
pub trait SourceProvider {
    type Error: std::error::Error + Send + Sync + 'static;
    fn capture(&self) -> Result<SourceSnapshot, Self::Error>;
}
