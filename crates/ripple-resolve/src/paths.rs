//! Conditional config-aware matches over captured source inventory, never graph confidence.
use crate::{
    ResolutionError,
    config::{EffectiveConfig, directory, join},
    modules::{
        LookupAttempt, LookupRule, ModuleLookup, ModuleOutcome, ModuleProblem, ModuleResolver,
    },
};
use ripple_core::{identity::RepoPath, syntax::VersionedHash};
use serde::Serialize;
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ConfigLookupProblem {
    UnsupportedConfiguration,
    AmbiguousAlias,
    Module(ModuleProblem),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ConfigOutcome {
    Found { path: RepoPath, rule: LookupRule },
    Unresolved { reason: ConfigLookupProblem },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ConfigRoute {
    Relative,
    Paths {
        pattern: String,
        declared_in: RepoPath,
    },
    BaseUrl,
    Unmapped,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConfiguredLookup {
    pub importer: RepoPath,
    pub specifier: String,
    pub config_hash: VersionedHash,
    pub routes: Vec<ConfigRoute>,
    pub attempts: Vec<LookupAttempt>,
    pub outcome: ConfigOutcome,
}
#[derive(Debug)]
pub struct ConfiguredResolver<'a> {
    config: &'a EffectiveConfig,
    relative: ModuleResolver,
}
impl<'a> ConfiguredResolver<'a> {
    pub fn new(
        config: &'a EffectiveConfig,
        inventory: impl IntoIterator<Item = RepoPath>,
    ) -> Result<Self, ResolutionError> {
        Ok(Self {
            config,
            relative: ModuleResolver::new(inventory)?,
        })
    }
    pub fn lookup(&self, importer: &RepoPath, specifier: &str) -> ConfiguredLookup {
        let mut result = ConfiguredLookup {
            importer: importer.clone(),
            specifier: specifier.into(),
            config_hash: self.config.fingerprint().clone(),
            routes: vec![],
            attempts: vec![],
            outcome: ConfigOutcome::Unresolved {
                reason: ConfigLookupProblem::Module(ModuleProblem::Missing),
            },
        };
        if !self.config.supported() {
            result.outcome = ConfigOutcome::Unresolved {
                reason: ConfigLookupProblem::UnsupportedConfiguration,
            };
            return result;
        }
        let relative = self.relative.lookup(importer, specifier);
        if !matches!(
            relative.outcome,
            ModuleOutcome::Unresolved {
                reason: ModuleProblem::NonRelative
            }
        ) {
            result.routes.push(
                if matches!(specifier, "." | "..")
                    || specifier.starts_with("./")
                    || specifier.starts_with("../")
                {
                    ConfigRoute::Relative
                } else {
                    ConfigRoute::Unmapped
                },
            );
            result.append(relative);
            return result;
        }
        let mut matches = Vec::new();
        if let Some(mapping) = self.config.paths.get(specifier) {
            matches.push((specifier, mapping, "", usize::MAX));
        } else {
            for (pattern, mapping) in &self.config.paths {
                if let Some((prefix, suffix)) = pattern.split_once('*')
                    && let Some(middle) = specifier
                        .strip_prefix(prefix)
                        .and_then(|s| s.strip_suffix(suffix))
                {
                    matches.push((pattern.as_str(), mapping, middle, prefix.len()));
                }
            }
        }
        if let Some(longest) = matches.iter().map(|m| m.3).max() {
            matches.retain(|m| m.3 == longest);
            if matches.len() != 1 {
                result.outcome = ConfigOutcome::Unresolved {
                    reason: ConfigLookupProblem::AmbiguousAlias,
                };
                return result;
            }
            let (pattern, mapping, middle, _) = matches[0];
            result.routes.push(ConfigRoute::Paths {
                pattern: pattern.into(),
                declared_in: mapping.declared_in.clone(),
            });
            for target in &mapping.targets {
                let target = target.replace('*', middle);
                let base = self
                    .config
                    .base_url
                    .as_deref()
                    .unwrap_or(directory(&mapping.declared_in));
                let lookup = self.rooted(importer, base, &target);
                if result.append(lookup) {
                    return result;
                }
                if !matches!(
                    result.outcome,
                    ConfigOutcome::Unresolved {
                        reason: ConfigLookupProblem::Module(ModuleProblem::Missing)
                    }
                ) {
                    return result;
                }
            }
        }
        if let Some(base) = &self.config.base_url {
            result.routes.push(ConfigRoute::BaseUrl);
            result.append(self.rooted(importer, base, specifier));
        } else if result.routes.is_empty() {
            result.routes.push(ConfigRoute::Unmapped);
            result.outcome = ConfigOutcome::Unresolved {
                reason: ConfigLookupProblem::Module(ModuleProblem::NonRelative),
            };
        }
        result
    }
    fn rooted(&self, importer: &RepoPath, base: &str, target: &str) -> ModuleLookup {
        match join(base, target) {
            Ok(path) => {
                let directory_request =
                    target.ends_with('/') || matches!(target.rsplit('/').next(), Some("." | ".."));
                let mut relative = "../".repeat(
                    directory(importer)
                        .split('/')
                        .filter(|s| !s.is_empty())
                        .count(),
                );
                relative.push_str(&path);
                if directory_request || path.is_empty() {
                    relative.push('/');
                }
                if !relative.starts_with("../") {
                    relative.insert_str(0, "./");
                }
                self.relative.lookup(importer, &relative)
            }
            Err(_) => ModuleLookup {
                importer: importer.clone(),
                specifier: target.into(),
                attempts: vec![],
                outcome: ModuleOutcome::Unresolved {
                    reason: ModuleProblem::OutsideRoot,
                },
            },
        }
    }
}
impl ConfiguredLookup {
    fn append(&mut self, lookup: ModuleLookup) -> bool {
        self.attempts.extend(lookup.attempts);
        self.outcome = match lookup.outcome {
            ModuleOutcome::Found { path, rule } => ConfigOutcome::Found { path, rule },
            ModuleOutcome::Unresolved { reason } => ConfigOutcome::Unresolved {
                reason: ConfigLookupProblem::Module(reason),
            },
        };
        matches!(self.outcome, ConfigOutcome::Found { .. })
    }
}
