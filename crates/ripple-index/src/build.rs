//! Cold build and verified-worktree queries; no stat-only freshness shortcut.
use crate::{
    build_capture::{Capture, capture},
    storage::{LoadedGeneration, PublishReceipt, Store},
};
use ripple_core::{
    identity::ProjectKey,
    index::{HASH_VERSION, IndexMetadata, IndexVersions, SCHEMA_VERSION, SnapshotData},
    syntax::FileFacts,
};
use ripple_resolve::{
    config::CONFIG_VERSION,
    reexports::{MODULES_VERSION, resolve_modules},
    symbols::{GRAPH_VERSION, resolve_graph},
};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Debug, Serialize)]
pub struct Counts {
    pub files: usize,
    pub symbols: usize,
    pub edges: usize,
    pub unresolved: usize,
}
#[derive(Debug, Serialize)]
pub struct IndexOutcome {
    pub discovery_diagnostics: Vec<String>,
    pub config_diagnostics: Vec<ripple_resolve::config::ConfigDiagnostic>,
    pub generation: u64,
    pub attempts: usize,
    pub counts: Counts,
    pub incomplete: bool,
    pub metadata: String,
    pub module_cache: String,
}
#[derive(Debug)]
pub struct CurrentIndex {
    pub discovery_diagnostics: Vec<String>,
    pub config_diagnostics: Vec<ripple_resolve::config::ConfigDiagnostic>,
    pub loaded: LoadedGeneration,
    pub incomplete: bool,
    pub counts: Counts,
}
pub fn versions() -> IndexVersions {
    IndexVersions {
        schema_version: SCHEMA_VERSION,
        hash_version: HASH_VERSION,
        analyzer_version: format!(
            "cold-v1|package-v1|{}|{MODULES_VERSION}|{CONFIG_VERSION}|{GRAPH_VERSION}",
            ripple_parse::hashes::EXTRACTOR_VERSION
        ),
    }
}
pub fn counts(snapshot: &ripple_core::index::IndexSnapshot) -> Counts {
    Counts {
        files: snapshot.files().len(),
        symbols: snapshot.files().iter().map(|f| f.symbols.len()).sum(),
        edges: snapshot.graph().edges().len(),
        unresolved: snapshot.graph().unresolved().len(),
    }
}
fn facts(c: &Capture) -> Result<Vec<FileFacts>, String> {
    c.discovery
        .sources
        .files()
        .map(|source| {
            let parsed =
                ripple_parse::parser::parse(source.path().clone(), source.text().as_bytes())
                    .map_err(|e| e.to_string())?;
            ripple_parse::extract(&parsed, ProjectKey::single_project()).map_err(|e| e.to_string())
        })
        .collect()
}
#[derive(Clone, Copy)]
enum Phase {
    BeforeVerification,
    AfterVerification,
}
pub type Provenance = fn(&Path) -> Result<ripple_core::revision::SnapshotOrigin, String>;
pub fn cold_index(root: &Path, provenance: Provenance) -> Result<IndexOutcome, String> {
    cold_inner(root, provenance, |_, _| {})
}
fn cold_inner(
    root: &Path,
    provenance: Provenance,
    mut hook: impl FnMut(usize, Phase),
) -> Result<IndexOutcome, String> {
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    // Validate provenance before creating local state.
    provenance(&root)?;
    let mut store = Store::open(root.join(".ripple"), versions()).map_err(|e| e.to_string())?;
    let mut failure = String::new();
    for attempt in 1..=3 {
        let captured = match capture(&root, provenance) {
            Ok(c) => c,
            Err(e) => {
                failure = e;
                continue;
            }
        };
        let files = facts(&captured)?;
        let tests = captured
            .discovery
            .files
            .iter()
            .filter(|f| f.is_test)
            .map(|f| f.path.clone())
            .collect::<BTreeSet<_>>();
        let analysis = resolve_graph(
            &ProjectKey::single_project(),
            &captured.discovery.sources,
            &files,
            &captured.config,
            &tests,
        )
        .map_err(|e| e.to_string())?;
        hook(attempt, Phase::BeforeVerification);
        let fresh = match capture(&root, provenance) {
            Ok(c) => c,
            Err(e) => {
                failure = e;
                continue;
            }
        };
        if !captured.matches(&fresh) {
            failure = "captured source/config/ignore/HEAD membership or bytes changed".into();
            continue;
        }
        hook(attempt, Phase::AfterVerification);
        let generation = store.next_generation().map_err(|e| e.to_string())?;
        let snapshot = SnapshotData {
            metadata: IndexMetadata {
                project: ProjectKey::single_project(),
                origin: captured.origin,
                versions: versions(),
                effective_config_hash: Some(captured.config.fingerprint().clone()),
                generation,
                created_at: format!(
                    "unix:{}",
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_err(|e| e.to_string())?
                        .as_secs()
                ),
            },
            inputs: captured.manifest,
            files,
            graph: analysis.graph,
        }
        .validate(&captured.discovery.sources)
        .map_err(|e| e.to_string())?;
        let PublishReceipt { metadata, .. } = store
            .publish(&snapshot, &captured.inputs)
            .map_err(|e| e.to_string())?;
        let module_cache = (|| -> Result<(), String> {
            let bytes = serde_json::to_vec(&analysis.modules).map_err(|e| e.to_string())?;
            if bytes.len() > 256 * 1024 * 1024 {
                return Err("module artifact exceeds 256 MiB".into());
            }
            let path = root
                .join(".ripple")
                .join(format!("modules-{generation}.json"));
            if let Ok(m) = fs::symlink_metadata(&path)
                && (!m.is_file() || m.file_type().is_symlink())
            {
                return Err("module cache path is not a regular file".into());
            }
            let mut temp =
                tempfile::NamedTempFile::new_in(root.join(".ripple")).map_err(|e| e.to_string())?;
            temp.write_all(&bytes).map_err(|e| e.to_string())?;
            temp.as_file().sync_all().map_err(|e| e.to_string())?;
            temp.persist(path).map_err(|e| e.to_string())?;
            Ok(())
        })();
        return Ok(IndexOutcome {
            discovery_diagnostics: captured
                .discovery
                .diagnostics
                .iter()
                .map(|d| format!("{}: {}", d.path, d.code))
                .collect(),
            config_diagnostics: captured.config.diagnostics().to_vec(),
            generation,
            attempts: attempt,
            counts: counts(&snapshot),
            incomplete: analysis.incomplete || !captured.discovery.diagnostics.is_empty(),
            metadata: format!("{metadata:?}"),
            module_cache: module_cache
                .err()
                .map_or("written".into(), |e| format!("pending: {e}")),
        });
    }
    Err(format!(
        "concurrent modification or capture failure after three attempts; prior generation preserved: {failure}"
    ))
}
pub fn load_current(root: &Path, provenance: Provenance) -> Result<CurrentIndex, String> {
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    if !root.join(".ripple/index.db").exists() {
        return Err("no index; run ripple index .".into());
    }
    let mut store=Store::open(root.join(".ripple"),versions()).map_err(|e|format!("{e}; safely archive incompatible .ripple state while no writers run, then ripple index ."))?;
    let loaded = store
        .load()
        .map_err(|e| e.to_string())?
        .ok_or("no complete index; run ripple index .")?;
    let a = capture(&root, provenance)?;
    let b = capture(&root, provenance)?;
    if !a.matches(&b)
        || loaded.snapshot.inputs() != &a.manifest
        || loaded.snapshot.metadata().origin != a.origin
        || loaded.snapshot.metadata().effective_config_hash.as_ref() != Some(a.config.fingerprint())
    {
        return Err("stale index or changing inputs; run ripple index .".into());
    }
    let modules = resolve_modules(
        &ProjectKey::single_project(),
        &loaded.sources,
        loaded.snapshot.files(),
        &a.config,
    )
    .map_err(|e| e.to_string())?;
    let incomplete = modules.incomplete
        || !loaded.snapshot.graph().unresolved().is_empty()
        || !a.discovery.diagnostics.is_empty();
    let counts = counts(&loaded.snapshot);
    Ok(CurrentIndex {
        discovery_diagnostics: a
            .discovery
            .diagnostics
            .iter()
            .map(|d| format!("{}: {}", d.path, d.code))
            .collect(),
        config_diagnostics: a.config.diagnostics().to_vec(),
        loaded,
        incomplete,
        counts,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use ripple_core::canonical::CanonicalState;
    use std::process::Command;
    fn cold_index(root: &Path) -> Result<IndexOutcome, String> {
        super::cold_index(root, ripple_git::worktree::observe)
    }
    fn load_current(root: &Path) -> Result<CurrentIndex, String> {
        super::load_current(root, ripple_git::worktree::observe)
    }
    fn cold_inner(root: &Path, hook: impl FnMut(usize, Phase)) -> Result<IndexOutcome, String> {
        super::cold_inner(root, ripple_git::worktree::observe, hook)
    }
    fn git(root: &Path, args: &[&str]) {
        let mut c = Command::new("git");
        for (k, _) in std::env::vars_os() {
            if k.to_string_lossy().starts_with("GIT_") {
                c.env_remove(k);
            }
        }
        let status = c
            .args(["-c", "core.hooksPath=/dev/null", "-C"])
            .arg(root)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            )
            .output()
            .unwrap();
        assert!(
            status.status.success(),
            "{:?}",
            String::from_utf8_lossy(&status.stderr)
        );
    }
    fn repo() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("main.ts"),
            "export function f() { return 0; }",
        )
        .unwrap();
        fs::write(temp.path().join(".gitignore"), ".ripple/\n").unwrap();
        git(temp.path(), &["init", "-q"]);
        git(temp.path(), &["config", "user.name", "Ripple Fixture"]);
        git(
            temp.path(),
            &["config", "user.email", "fixture@ripple.invalid"],
        );
        git(temp.path(), &["-c", "commit.gpgsign=false", "add", "."]);
        git(
            temp.path(),
            &["-c", "commit.gpgsign=false", "commit", "-qm", "fixture"],
        );
        temp
    }
    #[test]
    fn cold_rebuild_roundtrips_and_is_canonically_deterministic() {
        let temp = repo();
        let first = cold_index(temp.path()).unwrap();
        assert_eq!(first.generation, 1);
        let a = load_current(temp.path()).unwrap();
        assert!(!a.incomplete);
        let c = CanonicalState::from_snapshot(&a.loaded.snapshot);
        let second = cold_index(temp.path()).unwrap();
        assert_eq!(second.generation, 2);
        let b = load_current(temp.path()).unwrap();
        assert_eq!(c, CanonicalState::from_snapshot(&b.loaded.snapshot));
        assert!(temp.path().join(".ripple/modules-2.json").is_file());
    }
    #[test]
    fn source_add_delete_config_ignore_and_head_races_preserve_prior_generation() {
        for kind in 0..5 {
            let temp = repo();
            cold_index(temp.path()).unwrap();
            let result = cold_inner(temp.path(), |attempt, phase| {
                if !matches!(phase, Phase::BeforeVerification) {
                    return;
                }
                match kind {
                    0 => fs::write(
                        temp.path().join("main.ts"),
                        format!("export function f() {{return {attempt};}} "),
                    )
                    .unwrap(),
                    1 => {
                        let p = temp.path().join("added.ts");
                        if p.exists() {
                            fs::remove_file(p).unwrap();
                        } else {
                            fs::write(p, "export function added() {} ").unwrap();
                        }
                    }
                    2 => fs::write(
                        temp.path().join("tsconfig.json"),
                        format!("{{\"compilerOptions\":{{\"strict\":true}}}} // {attempt}"),
                    )
                    .unwrap(),
                    3 => fs::write(
                        temp.path().join(".gitignore"),
                        format!(".ripple/\n# {attempt}\n"),
                    )
                    .unwrap(),
                    _ => git(
                        temp.path(),
                        &[
                            "-c",
                            "commit.gpgsign=false",
                            "commit",
                            "--allow-empty",
                            "-qm",
                            &format!("race-{attempt}"),
                        ],
                    ),
                }
            });
            assert!(result.is_err(), "race {kind}");
            let mut store = Store::open(temp.path().join(".ripple"), versions()).unwrap();
            assert_eq!(
                store
                    .load()
                    .unwrap()
                    .unwrap()
                    .snapshot
                    .metadata()
                    .generation,
                1
            );
        }
    }
    #[test]
    fn one_concurrent_edit_retries_and_post_verification_edits_are_detectably_stale() {
        let temp = repo();
        let result = cold_inner(temp.path(), |attempt, phase| {
            if attempt == 1 && matches!(phase, Phase::BeforeVerification) {
                fs::write(
                    temp.path().join("main.ts"),
                    "export function f() { return 3; }",
                )
                .unwrap();
            }
        })
        .unwrap();
        assert_eq!(result.attempts, 2);
        let result = cold_inner(temp.path(), |_, phase| {
            if matches!(phase, Phase::AfterVerification) {
                fs::write(
                    temp.path().join("main.ts"),
                    "export function f() { return 4; }",
                )
                .unwrap();
            }
        })
        .unwrap();
        assert_eq!(result.generation, 2);
        assert!(load_current(temp.path()).unwrap_err().contains("stale"));
    }
    #[test]
    fn inherited_configs_and_lockfile_bytes_are_verified_after_reopen() {
        let temp = repo();
        fs::create_dir(temp.path().join("configs")).unwrap();
        fs::write(
            temp.path().join("tsconfig.json"),
            "{\"extends\":\"./configs/base\"}",
        )
        .unwrap();
        fs::write(
            temp.path().join("configs/base.json"),
            "{\"compilerOptions\":{\"baseUrl\":\"..\"}}",
        )
        .unwrap();
        fs::write(temp.path().join("bun.lockb"), [0, 255, 1]).unwrap();
        cold_index(temp.path()).unwrap();
        let current = load_current(temp.path()).unwrap();
        assert!(
            current
                .loaded
                .inputs
                .iter()
                .any(|i| i.path.as_str() == "bun.lockb" && i.bytes == [0, 255, 1])
        );
        fs::write(temp.path().join("configs/base.json"), "{} ").unwrap();
        assert!(load_current(temp.path()).unwrap_err().contains("stale"));
    }
    #[test]
    fn metadata_and_module_cache_failures_do_not_falsely_report_rollback() {
        let temp = repo();
        fs::create_dir_all(temp.path().join(".ripple/metadata.json")).unwrap();
        fs::create_dir(temp.path().join(".ripple/modules-1.json")).unwrap();
        let out = cold_index(temp.path()).unwrap();
        assert!(out.metadata.contains("Pending"));
        assert!(out.module_cache.starts_with("pending:"));
        assert_eq!(
            load_current(temp.path())
                .unwrap()
                .loaded
                .snapshot
                .metadata()
                .generation,
            1
        );
    }
    #[test]
    fn repository_external_filters_are_rejected_before_any_script_execution() {
        let temp = repo();
        git(
            temp.path(),
            &["config", "filter.pwn.clean", "echo SHOULD_NOT_RUN"],
        );
        fs::write(temp.path().join(".gitattributes"), "*.ts filter=pwn\n").unwrap();
        let err = cold_index(temp.path()).unwrap_err();
        assert!(err.contains("filters are unsupported"));
        assert!(!temp.path().join(".ripple").exists());
    }
    #[cfg(unix)]
    #[test]
    fn symlink_configuration_never_reads_outside_the_project() {
        use std::os::unix::fs::symlink;
        let temp = repo();
        cold_index(temp.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.json"), "{}").unwrap();
        symlink(
            outside.path().join("secret.json"),
            temp.path().join("tsconfig.json"),
        )
        .unwrap();
        assert!(cold_index(temp.path()).is_err());
        let mut store = Store::open(temp.path().join(".ripple"), versions()).unwrap();
        assert_eq!(
            store
                .load()
                .unwrap()
                .unwrap()
                .snapshot
                .metadata()
                .generation,
            1
        );
    }
}
#[cfg(test)]
mod package_tests {
    #[test]
    fn package_policy_rejects_unmodeled_entries_and_duplicate_keys() {
        let root = ripple_core::identity::RepoPath::new("tsconfig.json").unwrap();
        let path = ripple_core::identity::RepoPath::new("package.json").unwrap();
        for bytes in [
            br#"{"type":"module"}"#.as_slice(),
            br#"{"main":"entry.ts"}"#.as_slice(),
            br#"{"type":"commonjs","type":"module"}"#.as_slice(),
        ] {
            let config = ripple_resolve::config::EffectiveConfig::implicit(root.clone())
                .unwrap()
                .with_package_manifests([(&path, bytes)])
                .unwrap();
            assert!(!config.supported());
        }
        let config = ripple_resolve::config::EffectiveConfig::implicit(root)
            .unwrap()
            .with_package_manifests([(
                &path,
                br#"{"name":"fixture","scripts":{"test":"NEVER EXECUTED"}}"#.as_slice(),
            )])
            .unwrap();
        assert!(config.supported());
    }
}
