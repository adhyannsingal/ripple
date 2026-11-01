//! Deterministic source-inventory lookup. No filesystem, package or compiler calls.
use crate::ResolutionError;
use ripple_core::identity::RepoPath;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ModuleProblem {
    UnknownImporter,
    NonRelative,
    InvalidSpecifier,
    OutsideRoot,
    UnsupportedExtension,
    SpecifierTooLong,
    Missing,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LookupRule {
    ExplicitSource,
    Extension,
    JavaScriptSubstitution,
    DirectoryIndex,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ModuleOutcome {
    Found { path: RepoPath, rule: LookupRule },
    Unresolved { reason: ModuleProblem },
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LookupAttempt {
    pub path: RepoPath,
    pub present: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ModuleLookup {
    pub importer: RepoPath,
    pub specifier: String,
    /// Ordered probes, including missing higher-priority candidates and the winner.
    pub attempts: Vec<LookupAttempt>,
    pub outcome: ModuleOutcome,
}
#[derive(Debug)]
pub struct ModuleResolver {
    paths: BTreeSet<RepoPath>,
}
impl ModuleResolver {
    pub fn new(paths: impl IntoIterator<Item = RepoPath>) -> Result<Self, ResolutionError> {
        let mut set = BTreeSet::new();
        let mut folded = BTreeSet::new();
        for path in paths {
            if set.len() >= 100_000
                || !(path.as_str().ends_with(".ts") || path.as_str().ends_with(".tsx"))
            {
                return Err(ResolutionError(
                    "Source inventory exceeds its supported file count or language scope".into(),
                ));
            }
            if !folded.insert(path.as_str().to_lowercase()) || !set.insert(path) {
                return Err(ResolutionError(
                    "Duplicate or case-colliding source inventory path".into(),
                ));
            }
        }
        Ok(Self { paths: set })
    }
    pub fn lookup(&self, importer: &RepoPath, specifier: &str) -> ModuleLookup {
        let mut lookup = ModuleLookup {
            importer: importer.clone(),
            specifier: specifier.into(),
            attempts: Vec::new(),
            outcome: ModuleOutcome::Unresolved {
                reason: ModuleProblem::Missing,
            },
        };
        let candidates = if !self.paths.contains(importer) {
            Err(ModuleProblem::UnknownImporter)
        } else {
            candidates(importer, specifier)
        };
        let candidates = match candidates {
            Ok(c) => c,
            Err(reason) => {
                lookup.outcome = ModuleOutcome::Unresolved { reason };
                return lookup;
            }
        };
        for (path, rule) in candidates {
            let present = self.paths.contains(&path);
            lookup.attempts.push(LookupAttempt {
                path: path.clone(),
                present,
            });
            if present {
                lookup.outcome = ModuleOutcome::Found { path, rule };
                break;
            }
        }
        lookup
    }
}
fn candidates(
    importer: &RepoPath,
    specifier: &str,
) -> Result<Vec<(RepoPath, LookupRule)>, ModuleProblem> {
    if specifier.len() > 4096 {
        return Err(ModuleProblem::SpecifierTooLong);
    }
    if specifier.is_empty()
        || specifier.contains(['\\', ':', '?', '#'])
        || specifier.chars().any(char::is_control)
        || specifier.starts_with('/')
    {
        return Err(ModuleProblem::InvalidSpecifier);
    }
    if !(specifier == "."
        || specifier == ".."
        || specifier.starts_with("./")
        || specifier.starts_with("../"))
    {
        return Err(ModuleProblem::NonRelative);
    }
    let mut parts = importer.as_str().split('/').collect::<Vec<_>>();
    parts.pop();
    for part in specifier.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(ModuleProblem::OutsideRoot);
                }
            }
            part => parts.push(part),
        }
    }
    let base = parts.join("/");
    let directory =
        specifier.ends_with('/') || matches!(specifier.rsplit('/').next(), Some("." | ".."));
    let mut result = Vec::new();
    let mut add = |path: String, rule| -> Result<(), ModuleProblem> {
        result.push((
            RepoPath::new(path).map_err(|_| ModuleProblem::InvalidSpecifier)?,
            rule,
        ));
        Ok(())
    };
    if !directory && (base.ends_with(".ts") || base.ends_with(".tsx")) {
        add(base, LookupRule::ExplicitSource)?;
    } else if !directory && base.ends_with(".js") {
        let stem = base.rsplit_once('.').unwrap().0;
        for suffix in [".ts", ".tsx", ".d.ts"] {
            add(
                format!("{stem}{suffix}"),
                LookupRule::JavaScriptSubstitution,
            )?;
        }
    } else {
        if !directory && parts.last().is_some_and(|p| p.contains('.')) {
            return Err(ModuleProblem::UnsupportedExtension);
        }
        if !directory && !base.is_empty() {
            for suffix in [".ts", ".tsx", ".d.ts"] {
                add(format!("{base}{suffix}"), LookupRule::Extension)?;
            }
        }
        let prefix = if base.is_empty() {
            String::new()
        } else {
            format!("{base}/")
        };
        for suffix in [".ts", ".tsx", ".d.ts"] {
            add(format!("{prefix}index{suffix}"), LookupRule::DirectoryIndex)?;
        }
    }
    Ok(result)
}
