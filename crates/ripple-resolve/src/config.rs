//! Bounded, pure evaluation of captured repository-local configuration inputs.
use crate::{ResolutionError, config_json};
use ripple_core::{identity::RepoPath, index::HASH_VERSION, syntax::VersionedHash};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const CONFIG_VERSION: &str = "ripple-config-v1-legacy-baseurl";
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigInput {
    pub path: RepoPath,
    pub bytes: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConfigDependency {
    pub path: RepoPath,
    /// None records a missing probe, also needed for later invalidation.
    pub content_hash: Option<VersionedHash>,
    pub byte_len: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ConfigDiagnostic {
    pub file: RepoPath,
    pub code: String,
    pub detail: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PathMapping {
    pub declared_in: RepoPath,
    pub targets: Vec<String>,
}
/// Constructed only through captured evaluation; serialized values are transport evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EffectiveConfig {
    root: RepoPath,
    pub(crate) base_url: Option<String>,
    pub(crate) paths: BTreeMap<String, PathMapping>,
    inputs: Vec<ConfigDependency>,
    diagnostics: Vec<ConfigDiagnostic>,
    fingerprint: VersionedHash,
}
impl EffectiveConfig {
    pub fn root(&self) -> &RepoPath {
        &self.root
    }
    pub fn base_url(&self) -> Option<&str> {
        self.base_url.as_deref()
    }
    pub fn paths(&self) -> &BTreeMap<String, PathMapping> {
        &self.paths
    }
    pub fn inputs(&self) -> &[ConfigDependency] {
        &self.inputs
    }
    pub fn diagnostics(&self) -> &[ConfigDiagnostic] {
        &self.diagnostics
    }
    pub fn fingerprint(&self) -> &VersionedHash {
        &self.fingerprint
    }
    pub fn supported(&self) -> bool {
        self.diagnostics.is_empty()
    }
}
#[derive(Debug)]
pub struct CapturedConfigs {
    inputs: BTreeMap<RepoPath, ConfigInput>,
}
fn hash(bytes: &[u8]) -> VersionedHash {
    VersionedHash {
        version: HASH_VERSION,
        digest: Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    }
}
pub(crate) fn directory(path: &RepoPath) -> &str {
    path.as_str().rsplit_once('/').map_or("", |(dir, _)| dir)
}
/// A normalized repository directory, with the root represented by the empty string.
pub(crate) fn join(base: &str, relative: &str) -> Result<String, &'static str> {
    if relative.contains("${")
        || relative.len() > 4096
        || relative.starts_with('/')
        || relative.contains(['\\', ':', '?', '#'])
        || relative.chars().any(char::is_control)
    {
        return Err("unsupported or oversized local path");
    }
    let mut parts = base
        .split('/')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>();
    for part in relative.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err("local path escapes repository");
                }
            }
            p => parts.push(p),
        }
    }
    let result = parts.join("/");
    if result.len() > 4096 {
        return Err("normalized local path exceeds 4096 bytes");
    }
    Ok(result)
}
type Options = BTreeMap<String, (Value, RepoPath)>;
struct Evaluation<'a> {
    captured: &'a CapturedConfigs,
    inputs: BTreeMap<RepoPath, ConfigDependency>,
    diagnostics: Vec<ConfigDiagnostic>,
    active: BTreeSet<RepoPath>,
}
impl Evaluation<'_> {
    fn diagnostic(&mut self, file: &RepoPath, code: &str, detail: impl Into<String>) {
        if self.diagnostics.len() < 128 {
            self.diagnostics.push(ConfigDiagnostic {
                file: file.clone(),
                code: code.into(),
                detail: detail.into(),
            });
        } else if self.diagnostics.len() == 128 {
            self.diagnostics.push(ConfigDiagnostic {
                file: file.clone(),
                code: "config_diagnostics_truncated".into(),
                detail: "additional unsupported configuration remains".into(),
            });
        }
    }
    fn observe(&mut self, path: &RepoPath) -> Option<&[u8]> {
        let input = self.captured.inputs.get(path);
        self.inputs
            .entry(path.clone())
            .or_insert_with(|| ConfigDependency {
                path: path.clone(),
                content_hash: input.map(|i| hash(&i.bytes)),
                byte_len: input.map(|i| i.bytes.len() as u64),
            });
        input.map(|i| i.bytes.as_slice())
    }
    fn visit(&mut self, path: &RepoPath, depth: usize) -> Options {
        if depth >= 32 {
            self.diagnostic(
                path,
                "config_depth_limit",
                "inheritance exceeds 32 configurations",
            );
            return Options::new();
        }
        if !self.active.insert(path.clone()) {
            self.diagnostic(
                path,
                "inheritance_cycle",
                "configuration already active in inheritance chain",
            );
            return Options::new();
        }
        let mut result = Options::new();
        let parsed = self.observe(path).map(config_json::parse);
        match parsed {
            None => self.diagnostic(path, "missing_config", "configuration was not captured"),
            Some(Err(reason)) => self.diagnostic(path, "malformed_config", reason),
            Some(Ok(Value::Object(mut object))) => {
                if let Some(extends) = object.remove("extends") {
                    if let Some(parent) = extends
                        .as_str()
                        .filter(|s| s.starts_with("./") || s.starts_with("../"))
                    {
                        match join(directory(path), parent)
                            .ok()
                            .filter(|p| !p.is_empty())
                            .and_then(|p| RepoPath::new(p).ok())
                        {
                            Some(parent) => {
                                // Exact captured file, then .json; no package/directory fallback.
                                let parent = if self.observe(&parent).is_none()
                                    && !parent.as_str().ends_with(".json")
                                {
                                    join("", &format!("{}.json", parent.as_str()))
                                        .ok()
                                        .and_then(|p| RepoPath::new(p).ok())
                                } else {
                                    Some(parent)
                                };
                                if let Some(parent) = parent {
                                    result = self.visit(&parent, depth + 1);
                                } else {
                                    self.diagnostic(
                                        path,
                                        "unsupported_extends",
                                        "suffixed configuration path exceeds its bound",
                                    );
                                }
                            }
                            None => self.diagnostic(
                                path,
                                "unsupported_extends",
                                "inheritance path is invalid or outside repository",
                            ),
                        }
                    } else {
                        self.diagnostic(
                            path,
                            "unsupported_extends",
                            "only one relative repository-local extends string is supported",
                        );
                    }
                }
                if let Some(options) = object.remove("compilerOptions") {
                    if let Value::Object(options) = options {
                        for (key, value) in options {
                            result.insert(key, (value, path.clone()));
                        }
                    } else {
                        self.diagnostic(
                            path,
                            "invalid_compiler_options",
                            "compilerOptions must be an object",
                        );
                    }
                }
                for key in object.keys() {
                    if key != "$schema" && key != "compileOnSave" {
                        self.diagnostic(
                            path,
                            "unsupported_config_field",
                            key.chars().take(128).collect::<String>(),
                        );
                    }
                }
            }
            Some(Ok(_)) => self.diagnostic(
                path,
                "malformed_config",
                "configuration root must be an object",
            ),
        }
        self.active.remove(path);
        result
    }
}
impl CapturedConfigs {
    pub fn new(inputs: impl IntoIterator<Item = ConfigInput>) -> Result<Self, ResolutionError> {
        let mut result = BTreeMap::new();
        let mut folded = BTreeSet::new();
        let mut total = 0usize;
        for input in inputs {
            total = total.saturating_add(input.bytes.len());
            if input.path.as_str().len() > 4096
                || result.len() >= 128
                || input.bytes.len() > 1024 * 1024
                || total > 16 * 1024 * 1024
            {
                return Err(ResolutionError(
                    "Configuration inventory exceeds 4096-byte paths, 128 files, 1 MiB/file or 16 MiB total".into(),
                ));
            }
            if !folded.insert(input.path.as_str().to_lowercase())
                || result.insert(input.path.clone(), input).is_some()
            {
                return Err(ResolutionError(
                    "Duplicate or case-colliding captured configuration path".into(),
                ));
            }
        }
        Ok(Self { inputs: result })
    }
    pub fn get(&self, path: &RepoPath) -> Option<&ConfigInput> {
        self.inputs.get(path)
    }
    pub fn evaluate(&self, root: &RepoPath) -> Result<EffectiveConfig, ResolutionError> {
        if root.as_str().len() > 4096 {
            return Err(ResolutionError(
                "Configuration root path exceeds 4096 bytes".into(),
            ));
        }
        let mut evaluation = Evaluation {
            captured: self,
            inputs: BTreeMap::new(),
            diagnostics: Vec::new(),
            active: BTreeSet::new(),
        };
        let mut options = evaluation.visit(root, 0);
        let base_url = options.remove("baseUrl").and_then(|(value, origin)| {
            match value
                .as_str()
                .and_then(|s| join(directory(&origin), s).ok())
                .filter(|s| !s.contains('*'))
            {
                Some(path) => Some(path),
                None => {
                    evaluation.diagnostic(
                        &origin,
                        "invalid_base_url",
                        "baseUrl must be a bounded repository-local path string",
                    );
                    None
                }
            }
        });
        let mut paths = BTreeMap::new();
        if let Some((value, origin)) = options.remove("paths") {
            if let Some(map) = value.as_object().filter(|m| m.len() <= 256) {
                for (pattern, targets) in map {
                    let targets = targets
                        .as_array()
                        .filter(|a| !a.is_empty() && a.len() <= 16)
                        .and_then(|a| {
                            a.iter()
                                .map(|v| v.as_str().map(str::to_string))
                                .collect::<Option<Vec<_>>>()
                        });
                    let valid_pattern = !pattern.is_empty()
                        && pattern.len() <= 4096
                        && pattern.matches('*').count() <= 1
                        && !pattern.starts_with('.')
                        && join("", pattern).is_ok();
                    if let Some(targets) = targets.filter(|targets| {
                        valid_pattern
                            && targets.iter().all(|t| {
                                !t.is_empty()
                                    && t.matches('*').count() <= usize::from(pattern.contains('*'))
                                    && (base_url.is_some()
                                        || t.starts_with("./")
                                        || t.starts_with("../"))
                                    && join(base_url.as_deref().unwrap_or(directory(&origin)), t)
                                        .is_ok()
                            })
                    }) {
                        paths.insert(
                            pattern.clone(),
                            PathMapping {
                                declared_in: origin.clone(),
                                targets,
                            },
                        );
                    } else {
                        evaluation.diagnostic(
                            &origin,
                            "invalid_paths_mapping",
                            pattern.chars().take(128).collect::<String>(),
                        );
                    }
                }
            } else {
                evaluation.diagnostic(
                    &origin,
                    "invalid_paths",
                    "paths must be an object with at most 256 mappings",
                );
            }
        }
        for (key, (value, origin)) in options {
            let mode = value.as_str().map(str::to_ascii_lowercase);
            let supported = match key.as_str() {
                "moduleResolution" => matches!(mode.as_deref(), Some("node" | "node10")),
                "module" => matches!(
                    mode.as_deref(),
                    Some("commonjs" | "esnext" | "es2015" | "es2020" | "es2022")
                ),
                // Retained in raw input fingerprints; not interpreted as compiler validation.
                "strict"
                | "noImplicitAny"
                | "strictNullChecks"
                | "skipLibCheck"
                | "noEmit"
                | "target"
                | "lib"
                | "esModuleInterop"
                | "allowSyntheticDefaultImports"
                | "forceConsistentCasingInFileNames"
                | "jsx" => true,
                _ => false,
            };
            if !supported {
                evaluation.diagnostic(
                    &origin,
                    "unsupported_compiler_option",
                    key.chars().take(128).collect::<String>(),
                );
            }
        }
        evaluation.diagnostics.sort();
        evaluation.diagnostics.dedup();
        let inputs = evaluation.inputs.into_values().collect::<Vec<_>>();
        let fingerprint_bytes = serde_json::to_vec(&(
            CONFIG_VERSION,
            root,
            &base_url,
            &paths,
            &inputs,
            &evaluation.diagnostics,
        ))
        .map_err(|e| ResolutionError(e.to_string()))?;
        Ok(EffectiveConfig {
            root: root.clone(),
            base_url,
            paths,
            inputs,
            diagnostics: evaluation.diagnostics,
            fingerprint: hash(&fingerprint_bytes),
        })
    }
}
