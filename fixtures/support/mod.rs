//! Shared, read-only fixture catalog for integration tests; no analyzer-generated goldens.

use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Deserialize)]
struct Manifest {
    schema_version: u32,
    cases: Vec<Entry>,
}
#[derive(Debug, Deserialize)]
struct Entry {
    id: String,
    root: String,
    expected: String,
    files: Vec<String>,
}
#[derive(Debug)]
pub struct FixtureCase {
    pub id: String,
    pub root: PathBuf,
    pub files: Vec<String>,
    pub expected: Value,
}

pub fn checked_join(root: &Path, relative: &str) -> Result<PathBuf, String> {
    if relative.is_empty()
        || relative.contains(['\\', ':'])
        || relative.chars().any(char::is_control)
        || Path::new(relative)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(format!(
            "not a portable relative fixture path: {relative:?}"
        ));
    }
    Ok(root.join(relative))
}

pub fn load_cases(workspace: &Path) -> Result<Vec<FixtureCase>, String> {
    let fixtures = workspace.join("fixtures");
    let manifest: Manifest = serde_json::from_str(
        &fs::read_to_string(fixtures.join("manifest.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if manifest.schema_version != 1 {
        return Err("unsupported fixture manifest version".into());
    }
    let mut ids = BTreeSet::new();
    let mut cases = Vec::new();
    for entry in manifest.cases {
        if !ids.insert(entry.id.clone()) {
            return Err("duplicate fixture ID".into());
        }
        let root = checked_join(&fixtures, &entry.root)?;
        let expected: Value = serde_json::from_str(
            &fs::read_to_string(checked_join(&fixtures, &entry.expected)?)
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if expected.get("schema_version").and_then(Value::as_u64) != Some(1) {
            return Err("unsupported fixture expectation version".into());
        }
        let mut seen = BTreeSet::new();
        for file in &entry.files {
            if !seen.insert(file.clone()) || !checked_join(&root, file)?.is_file() {
                return Err(format!("missing/duplicate fixture file: {file}"));
            }
        }
        if entry.files.is_empty() {
            return Err("empty fixture".into());
        }
        cases.push(FixtureCase {
            id: entry.id,
            root,
            files: entry.files,
            expected,
        });
    }
    cases.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(cases)
}
