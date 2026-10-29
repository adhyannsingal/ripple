use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ContractError;

/// A normalized lexical path relative to a repository; never an absolute OS path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RepoPath(String);

impl RepoPath {
    pub fn new(value: impl AsRef<str>) -> Result<Self, ContractError> {
        let original = value.as_ref();
        let portable = original.replace('\\', "/");
        if portable.starts_with('/')
            || portable.contains(':')
            || portable.chars().any(char::is_control)
        {
            return Err(ContractError::InvalidPath(original.into()));
        }
        let mut parts = Vec::new();
        for part in portable.split('/') {
            match part {
                "" | "." => {}
                ".." => return Err(ContractError::InvalidPath(original.into())),
                _ => parts.push(part),
            }
        }
        if parts.is_empty() {
            return Err(ContractError::InvalidPath(original.into()));
        }
        Ok(Self(parts.join("/")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for RepoPath {
    type Error = ContractError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<RepoPath> for String {
    fn from(value: RepoPath) -> Self {
        value.0
    }
}
impl fmt::Display for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProjectKey(String);
impl ProjectKey {
    pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        if value.is_empty() || value.chars().any(char::is_control) {
            return Err(ContractError::InvalidIdentity(
                "project key must be nonempty without control characters",
            ));
        }
        Ok(Self(value))
    }
    /// Persist this repository-local key; do not derive it from a machine path/ref.
    pub fn single_project() -> Self {
        Self("default".into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for ProjectKey {
    type Error = ContractError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<ProjectKey> for String {
    fn from(value: ProjectKey) -> Self {
        value.0
    }
}

/// Structured components avoid collisions between a method name and scope separators.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "Vec<String>", into = "Vec<String>")]
pub struct QualifiedName(Vec<String>);
impl QualifiedName {
    pub fn new(parts: impl IntoIterator<Item = String>) -> Result<Self, ContractError> {
        let parts: Vec<_> = parts.into_iter().collect();
        if parts.is_empty()
            || parts
                .iter()
                .any(|p| p.is_empty() || p.chars().any(char::is_control))
        {
            return Err(ContractError::InvalidIdentity(
                "qualified name requires nonempty components",
            ));
        }
        Ok(Self(parts))
    }
    pub fn components(&self) -> &[String] {
        &self.0
    }
    pub fn display_name(&self) -> String {
        self.0.join(".")
    }
}
impl TryFrom<Vec<String>> for QualifiedName {
    type Error = ContractError;
    fn try_from(value: Vec<String>) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<QualifiedName> for Vec<String> {
    fn from(value: QualifiedName) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SymbolKind {
    Function,
    Class,
    Method,
}
impl SymbolKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Function => "FUNCTION",
            Self::Class => "CLASS",
            Self::Method => "METHOD",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "RawSymbolId")]
pub struct SymbolId {
    project: ProjectKey,
    path: RepoPath,
    qualified_name: QualifiedName,
    kind: SymbolKind,
    disambiguator: String,
}
#[derive(Deserialize)]
struct RawSymbolId {
    project: ProjectKey,
    path: RepoPath,
    qualified_name: QualifiedName,
    kind: SymbolKind,
    disambiguator: String,
}
impl TryFrom<RawSymbolId> for SymbolId {
    type Error = ContractError;
    fn try_from(raw: RawSymbolId) -> Result<Self, Self::Error> {
        Self::new(raw.project, raw.path, raw.qualified_name, raw.kind)
            .with_disambiguator(raw.disambiguator)
    }
}
impl SymbolId {
    pub fn new(
        project: ProjectKey,
        path: RepoPath,
        qualified_name: QualifiedName,
        kind: SymbolKind,
    ) -> Self {
        Self {
            project,
            path,
            qualified_name,
            kind,
            disambiguator: String::new(),
        }
    }
    pub fn with_disambiguator(mut self, value: String) -> Result<Self, ContractError> {
        if value.chars().any(char::is_control) {
            return Err(ContractError::InvalidIdentity("invalid disambiguator"));
        }
        self.disambiguator = value;
        Ok(self)
    }
    pub fn path(&self) -> &RepoPath {
        &self.path
    }
    pub fn project(&self) -> &ProjectKey {
        &self.project
    }
    pub fn qualified_name(&self) -> &QualifiedName {
        &self.qualified_name
    }
    pub fn kind(&self) -> SymbolKind {
        self.kind
    }
    /// Versioned, unambiguous UTF-8 byte-length framing; display strings are not IDs.
    pub fn semantic_key(&self) -> String {
        let mut key = "symbol:v1:".to_owned();
        let mut fields = vec![
            self.project.as_str(),
            self.path.as_str(),
            self.kind.as_str(),
            self.disambiguator.as_str(),
        ];
        fields.extend(self.qualified_name.components().iter().map(String::as_str));
        for field in fields {
            key.push_str(&format!("{}:{}", field.len(), field));
        }
        key
    }
}
