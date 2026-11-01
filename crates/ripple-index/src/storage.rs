//! Authoritative immutable SQLite generations. Live input verification is a caller obligation.
use ripple_core::{
    canonical::CanonicalState,
    graph::{Edge, GraphNode, GraphSnapshot, NodeId, UnresolvedReference},
    identity::RepoPath,
    index::{
        CapturedInput, HASH_VERSION, IndexMetadata, IndexSnapshot, IndexVersions, InputManifest,
        InputRole, SCHEMA_VERSION, SnapshotData,
    },
    source::{SourceFile, SourceSnapshot},
    syntax::{FileFacts, SymbolFact, VersionedHash},
};
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, TransactionBehavior, config::DbConfig, params,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fmt::{self, Write as _},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

const APPLICATION_ID: i64 = 0x5249504c;
const MIGRATION: &str = include_str!("../migrations/0001-generations.sql");
const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_TOTAL_INPUT_BYTES: usize = 256 * 1024 * 1024;
const MAX_RECORD_BYTES: usize = 64 * 1024 * 1024;
const MAX_RECORDS: usize = 1_000_000;
#[derive(Debug)]
pub enum StorageError {
    Busy,
    Incompatible(String),
    Invalid(String),
    Database(rusqlite::Error),
    Io(std::io::Error),
    Json(serde_json::Error),
}
impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => {
                f.write_str("index writer is busy; retry after the current writer finishes")
            }
            Self::Incompatible(s) => write!(
                f,
                "incompatible index: {s}; rebuild with a supported version"
            ),
            Self::Invalid(s) => write!(f, "invalid index/capture: {s}"),
            Self::Database(e) => write!(f, "SQLite storage: {e}"),
            Self::Io(e) => write!(f, "index filesystem: {e}"),
            Self::Json(e) => write!(f, "index JSON: {e}"),
        }
    }
}
impl std::error::Error for StorageError {}
impl From<rusqlite::Error> for StorageError {
    fn from(e: rusqlite::Error) -> Self {
        match e.sqlite_error_code() {
            Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => {
                Self::Busy
            }
            _ => Self::Database(e),
        }
    }
}
impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for StorageError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
fn invalid(s: impl Into<String>) -> StorageError {
    StorageError::Invalid(s.into())
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputBytes {
    pub path: RepoPath,
    pub bytes: Vec<u8>,
}
#[derive(Debug)]
pub struct LoadedGeneration {
    pub snapshot: IndexSnapshot,
    pub inputs: Vec<InputBytes>,
    pub sources: SourceSnapshot,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataStatus {
    Current { generation: u64 },
    Updated { generation: u64 },
    Pending { reason: String },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishReceipt {
    pub generation: u64,
    pub metadata: MetadataStatus,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataSummary {
    pub metadata: IndexMetadata,
    pub input_count: u64,
    pub file_count: u64,
    pub node_count: u64,
    pub edge_count: u64,
    pub unresolved_count: u64,
}
pub struct Store {
    connection: Connection,
    directory: PathBuf,
    expected: IndexVersions,
}
fn digest(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut result, "{byte:02x}").expect("String formatting is infallible");
    }
    result
}
/// Raw captured source/config/ignore bytes use SHA-256, distinct from framed syntax hashes.
pub fn input_hash(bytes: &[u8]) -> VersionedHash {
    VersionedHash {
        version: HASH_VERSION,
        digest: digest(bytes),
    }
}
fn json(value: &impl Serialize) -> Result<String, StorageError> {
    let text = serde_json::to_string(value)?;
    if text.len() > MAX_RECORD_BYTES {
        return Err(invalid("serialized record exceeds 64 MiB"));
    }
    Ok(text)
}
fn decode<T: DeserializeOwned>(text: &str) -> Result<T, StorageError> {
    if text.len() > MAX_RECORD_BYTES {
        return Err(invalid("serialized record exceeds 64 MiB"));
    }
    Ok(serde_json::from_str(text)?)
}
fn tag(value: &impl Serialize) -> Result<String, StorageError> {
    serde_json::to_value(value)?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| invalid("expected an enum tag"))
}
fn integer(n: u64) -> Result<i64, StorageError> {
    i64::try_from(n).map_err(|_| invalid("generation/count exceeds SQLite integer range"))
}
fn current(c: &Connection) -> Result<Option<i64>, StorageError> {
    let id: Option<i64> = c
        .query_row(
            "SELECT generation_id FROM current_generation WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if id.is_some_and(|id| id <= 0) {
        return Err(invalid("active generation is invalid"));
    }
    if id.is_none()
        && c.query_row(
            "SELECT EXISTS(SELECT 1 FROM generations WHERE complete=1)",
            [],
            |r| r.get::<_, bool>(0),
        )?
    {
        return Err(invalid(
            "complete generations exist but the active pointer is missing",
        ));
    }
    Ok(id)
}
fn reject_special(path: &Path) -> Result<(), StorageError> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() || !m.is_file() => Err(invalid(format!(
            "{} is not a regular non-symlink file",
            path.display()
        ))),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
fn captured(
    snapshot: &IndexSnapshot,
    inputs: &[InputBytes],
) -> Result<SourceSnapshot, StorageError> {
    let mut by_path = BTreeMap::new();
    let mut total = 0usize;
    for input in inputs {
        total = total
            .checked_add(input.bytes.len())
            .ok_or_else(|| invalid("captured byte size overflow"))?;
        if input.bytes.len() > MAX_INPUT_BYTES || total > MAX_TOTAL_INPUT_BYTES {
            return Err(invalid(
                "captured input exceeds 16 MiB/file or 256 MiB/generation",
            ));
        }
        if by_path.insert(&input.path, &input.bytes).is_some() {
            return Err(invalid("duplicate captured input path"));
        }
    }
    if by_path.len() != snapshot.inputs().entries().len() {
        return Err(invalid("captured bytes do not cover the manifest"));
    }
    let mut sources = Vec::new();
    for input in snapshot.inputs().entries() {
        let bytes = by_path
            .get(&input.path)
            .ok_or_else(|| invalid("missing captured bytes"))?;
        if bytes.len() as u64 != input.byte_len || input_hash(bytes) != input.content_hash {
            return Err(invalid(format!(
                "captured byte length/hash mismatch: {}",
                input.path
            )));
        }
        if input.role == InputRole::Source {
            sources.push(SourceFile::new(
                input.path.clone(),
                std::str::from_utf8(bytes)
                    .map_err(|_| invalid("captured source is not UTF-8"))?
                    .into(),
            ));
        }
    }
    let sources = SourceSnapshot::new(sources).map_err(|e| invalid(e.to_string()))?;
    snapshot
        .data()
        .clone()
        .validate(&sources)
        .map_err(|e| invalid(e.to_string()))?;
    Ok(sources)
}
fn seal(snapshot: &IndexSnapshot) -> Result<String, StorageError> {
    let data = serde_json::to_vec(&(
        "ripple.storage.seal.v1",
        snapshot.metadata(),
        CanonicalState::from_snapshot(snapshot),
    ))?;
    if data.len() > MAX_TOTAL_INPUT_BYTES {
        return Err(invalid("canonical generation exceeds 256 MiB"));
    }
    Ok(digest(&data))
}
impl Store {
    /// The directory is explicit and caller-owned; this does not select a project root.
    pub fn open(
        directory: impl AsRef<Path>,
        expected: IndexVersions,
    ) -> Result<Self, StorageError> {
        expected.validate().map_err(|e| invalid(e.to_string()))?;
        if expected.schema_version != SCHEMA_VERSION || expected.hash_version != HASH_VERSION {
            return Err(StorageError::Incompatible(
                "unsupported schema/hash version".into(),
            ));
        }
        let directory = directory.as_ref();
        if fs::symlink_metadata(directory).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(invalid("index directory must not be a symlink"));
        }
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(directory)?;
        let directory = fs::canonicalize(directory)?;
        for name in ["index.db", "index.db-wal", "index.db-shm"] {
            reject_special(&directory.join(name))?;
        }
        let path = directory.join("index.db");
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => {
                file.sync_all()?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
        let mut connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?;
        connection.busy_timeout(Duration::ZERO)?;
        connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF;")?;
        connection.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let application: i64 =
            connection.pragma_query_value(None, "application_id", |r| r.get(0))?;
        if version == 0 && application == 0 {
            let count: i64 = connection.query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
                [],
                |r| r.get(0),
            )?;
            if count != 0 {
                return Err(StorageError::Incompatible(
                    "unrecognized database; not initializing over existing tables".into(),
                ));
            }
            connection.pragma_update(None, "journal_mode", "WAL")?;
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            // Recheck after acquiring the migration writer slot.
            let version: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
            let application: i64 = tx.pragma_query_value(None, "application_id", |r| r.get(0))?;
            let tables: i64 = tx.query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
                [],
                |r| r.get(0),
            )?;
            if version != 0 || application != 0 || tables != 0 {
                return Err(StorageError::Incompatible(
                    "database changed during initialization; reopen".into(),
                ));
            }
            tx.execute_batch(MIGRATION)?;
            tx.execute(
                "INSERT INTO schema_migrations(version,checksum) VALUES(1,?1)",
                [digest(MIGRATION.as_bytes())],
            )?;
            tx.pragma_update(None, "application_id", APPLICATION_ID)?;
            tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            tx.commit()?;
        } else if version != i64::from(SCHEMA_VERSION) || application != APPLICATION_ID {
            return Err(StorageError::Incompatible(format!(
                "database application/schema {application}/{version}"
            )));
        }
        let checksum: String = connection.query_row(
            "SELECT checksum FROM schema_migrations WHERE version=1",
            [],
            |r| r.get(0),
        )?;
        let count: i64 =
            connection.query_row("SELECT count(*) FROM schema_migrations", [], |r| r.get(0))?;
        if checksum != digest(MIGRATION.as_bytes()) || count != 1 {
            return Err(StorageError::Incompatible(
                "migration history/checksum mismatch".into(),
            ));
        }
        let journal: String = connection.pragma_query_value(None, "journal_mode", |r| r.get(0))?;
        if journal.to_lowercase() != "wal" {
            return Err(StorageError::Incompatible(
                "WAL journal mode required".into(),
            ));
        }
        connection.pragma_update(None, "synchronous", "FULL")?;
        Ok(Self {
            connection,
            directory,
            expected,
        })
    }
    pub fn next_generation(&self) -> Result<u64, StorageError> {
        let id = current(&self.connection)?
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| invalid("generation exhausted"))?;
        Ok(id as u64)
    }
    pub fn publish(
        &mut self,
        snapshot: &IndexSnapshot,
        inputs: &[InputBytes],
    ) -> Result<PublishReceipt, StorageError> {
        snapshot
            .ensure_compatible(&self.expected)
            .map_err(|e| StorageError::Incompatible(e.to_string()))?;
        captured(snapshot, inputs)?;
        let id = integer(snapshot.metadata().generation)?;
        let checksum = seal(snapshot)?;
        let metadata = json(snapshot.metadata())?;
        let count = snapshot.inputs().entries().len()
            + snapshot.files().len()
            + snapshot
                .files()
                .iter()
                .map(|f| f.symbols.len())
                .sum::<usize>()
            + snapshot.graph().nodes().len()
            + snapshot.graph().edges().len()
            + snapshot.graph().unresolved().len();
        if count > MAX_RECORDS {
            return Err(invalid("generation exceeds one million records"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if id
            != current(&tx)?
                .unwrap_or(0)
                .checked_add(1)
                .ok_or_else(|| invalid("generation exhausted"))?
        {
            return Err(invalid(
                "generation conflict; refresh the generation counter before publishing",
            ));
        }
        tx.execute(
            "INSERT INTO generations(id,metadata_json,seal,complete) VALUES(?1,?2,?3,0)",
            params![id, metadata, checksum],
        )?;
        let bytes = inputs
            .iter()
            .map(|i| (&i.path, &i.bytes))
            .collect::<BTreeMap<_, _>>();
        for input in snapshot.inputs().entries() {
            tx.execute(
                "INSERT INTO captured_inputs VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    id,
                    input.path.as_str(),
                    tag(&input.role)?,
                    integer(input.byte_len)?,
                    input.content_hash.version,
                    input.content_hash.digest,
                    bytes[&input.path]
                ],
            )?;
        }
        for node in snapshot.graph().nodes() {
            tx.execute(
                "INSERT INTO nodes VALUES(?1,?2,?3,?4)",
                params![id, node.id.semantic_key(), tag(&node.kind)?, json(node)?],
            )?;
        }
        for facts in snapshot.files() {
            let mut detail = facts.clone();
            detail.symbols.clear();
            tx.execute(
                "INSERT INTO files VALUES(?1,?2,?3,?4)",
                params![
                    id,
                    facts.path.as_str(),
                    NodeId::file(snapshot.metadata().project.clone(), facts.path.clone())
                        .semantic_key(),
                    json(&detail)?
                ],
            )?;
            for symbol in &facts.symbols {
                tx.execute(
                    "INSERT INTO symbols VALUES(?1,?2,?3,?4)",
                    params![
                        id,
                        symbol.identity.semantic_key(),
                        facts.path.as_str(),
                        json(symbol)?
                    ],
                )?;
            }
        }
        for (ordinal, edge) in snapshot.graph().edges().iter().enumerate() {
            tx.execute(
                "INSERT INTO edges VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    id,
                    ordinal as i64,
                    edge.source.semantic_key(),
                    edge.target.semantic_key(),
                    tag(&edge.kind)?,
                    tag(&edge.confidence)?,
                    json(edge)?
                ],
            )?;
        }
        for (ordinal, item) in snapshot.graph().unresolved().iter().enumerate() {
            tx.execute(
                "INSERT INTO unresolved_references VALUES(?1,?2,?3,?4,?5)",
                params![
                    id,
                    ordinal as i64,
                    item.source.semantic_key(),
                    item.boundary.semantic_key(),
                    json(item)?
                ],
            )?;
        }
        tx.execute("UPDATE generations SET complete=1 WHERE id=?1", [id])?;
        tx.execute("INSERT INTO current_generation VALUES(1,?1) ON CONFLICT(singleton) DO UPDATE SET generation_id=excluded.generation_id",[id])?;
        tx.commit()?;
        let metadata = self
            .recover_metadata()
            .unwrap_or_else(|e| MetadataStatus::Pending {
                reason: e.to_string(),
            });
        Ok(PublishReceipt {
            generation: id as u64,
            metadata,
        })
    }
    pub fn load(&mut self) -> Result<Option<LoadedGeneration>, StorageError> {
        self.read(None)
    }
    pub fn load_generation(
        &mut self,
        generation: u64,
    ) -> Result<Option<LoadedGeneration>, StorageError> {
        if generation == 0 {
            return Err(invalid("generation must be positive"));
        }
        self.read(Some(integer(generation)?))
    }
    fn read(&mut self, generation: Option<i64>) -> Result<Option<LoadedGeneration>, StorageError> {
        self.read_with_hook(generation, || Ok(()))
    }
    fn read_with_hook(
        &mut self,
        generation: Option<i64>,
        hook: impl FnOnce() -> Result<(), StorageError>,
    ) -> Result<Option<LoadedGeneration>, StorageError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)?;
        let generation = match generation.or(current(&tx)?) {
            Some(id) => id,
            None => {
                tx.commit()?;
                return Ok(None);
            }
        };
        hook()?;
        let loaded = load(&tx, generation, &self.expected)?;
        tx.commit()?;
        Ok(loaded)
    }
    /// Rebuild the latest derived summary while holding the writer slot; it is never authoritative.
    pub fn recover_metadata(&mut self) -> Result<MetadataStatus, StorageError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let id = current(&tx)?.ok_or_else(|| invalid("no complete generation available"))?;
        let loaded = load(&tx, id, &self.expected)?
            .ok_or_else(|| invalid("active generation is missing"))?;
        let count = |table: &str| -> Result<u64, StorageError> {
            Ok(tx.query_row(
                &format!("SELECT count(*) FROM {table} WHERE generation_id=?1"),
                [id],
                |r| r.get::<_, i64>(0),
            )? as u64)
        };
        let summary = MetadataSummary {
            metadata: loaded.snapshot.metadata().clone(),
            input_count: count("captured_inputs")?,
            file_count: count("files")?,
            node_count: count("nodes")?,
            edge_count: count("edges")?,
            unresolved_count: count("unresolved_references")?,
        };
        let mut expected = serde_json::to_vec_pretty(&summary)?;
        expected.push(b'\n');
        if expected.len() > MAX_RECORD_BYTES {
            return Err(invalid("metadata summary exceeds 64 MiB"));
        }
        let path = self.directory.join("metadata.json");
        reject_special(&path)?;
        let existing = match fs::File::open(&path) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take(MAX_RECORD_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)?;
                Some(bytes)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        let status = if existing.as_deref() == Some(expected.as_slice()) {
            MetadataStatus::Current {
                generation: id as u64,
            }
        } else {
            let mut temporary = tempfile::Builder::new()
                .prefix(".metadata-")
                .tempfile_in(&self.directory)?;
            temporary.write_all(&expected)?;
            temporary.as_file().sync_all()?;
            temporary
                .persist(&path)
                .map_err(|e| StorageError::Io(e.error))?;
            #[cfg(unix)]
            fs::File::open(&self.directory)?.sync_all()?;
            MetadataStatus::Updated {
                generation: id as u64,
            }
        };
        tx.commit()?;
        Ok(status)
    }
}
fn header(c: &Connection, id: i64) -> Result<Option<(IndexMetadata, String)>, StorageError> {
    let sizes:Option<(i64,i64,i64)>=c.query_row("SELECT length(CAST(metadata_json AS BLOB)),length(seal),complete FROM generations WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    let Some((length, seal_length, complete)) = sizes else {
        return Ok(None);
    };
    if length < 0 || length as usize > MAX_RECORD_BYTES || seal_length != 64 || complete != 1 {
        return Err(invalid("generation header is incomplete or oversized"));
    }
    let (text, checksum): (String, String) = c.query_row(
        "SELECT metadata_json,seal FROM generations WHERE id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let metadata: IndexMetadata = decode(&text)?;
    if metadata.generation != id as u64 {
        return Err(invalid("generation header identity mismatch"));
    }
    Ok(Some((metadata, checksum)))
}
fn load(
    c: &Connection,
    id: i64,
    expected: &IndexVersions,
) -> Result<Option<LoadedGeneration>, StorageError> {
    let Some((metadata, checksum)) = header(c, id)? else {
        return Ok(None);
    };
    if &metadata.versions != expected {
        return Err(StorageError::Incompatible(
            "stored analyzer/schema/hash differs from this reader".into(),
        ));
    }
    let mut count = 0usize;
    let mut json_bytes = 0usize;
    for (table, column) in [
        ("files", "facts_json"),
        ("symbols", "record_json"),
        ("nodes", "record_json"),
        ("edges", "record_json"),
        ("unresolved_references", "record_json"),
    ] {
        let (rows,max,sum):(i64,i64,i64)=c.query_row(&format!("SELECT count(*),COALESCE(max(length(CAST({column} AS BLOB))),0),COALESCE(sum(length(CAST({column} AS BLOB))),0) FROM {table} WHERE generation_id=?1"),[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        count = count.saturating_add(rows as usize);
        json_bytes = json_bytes.saturating_add(sum as usize);
        if count > MAX_RECORDS
            || max < 0
            || max as usize > MAX_RECORD_BYTES
            || json_bytes > MAX_TOTAL_INPUT_BYTES
        {
            return Err(invalid("stored record budget exceeded"));
        }
    }
    let (rows,max,sum):(i64,i64,i64)=c.query_row("SELECT count(*),COALESCE(max(length(bytes)),0),COALESCE(sum(length(bytes)),0) FROM captured_inputs WHERE generation_id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    if rows as usize + count > MAX_RECORDS
        || max < 0
        || max as usize > MAX_INPUT_BYTES
        || sum < 0
        || sum as usize > MAX_TOTAL_INPUT_BYTES
    {
        return Err(invalid("stored captured-input budget exceeded"));
    }
    let mut inputs = Vec::new();
    let mut manifest = Vec::new();
    let mut sources = Vec::new();
    {
        let mut stmt=c.prepare("SELECT path,role,byte_len,hash_version,digest,bytes FROM captured_inputs WHERE generation_id=?1 ORDER BY path")?;
        let mut rows = stmt.query([id])?;
        while let Some(row) = rows.next()? {
            let path: String = row.get(0)?;
            let normalized = RepoPath::new(&path).map_err(|e| invalid(e.to_string()))?;
            if normalized.as_str() != path {
                return Err(invalid("noncanonical stored path"));
            }
            let role: InputRole = serde_json::from_value(serde_json::Value::String(row.get(1)?))?;
            let input = CapturedInput {
                path: normalized.clone(),
                role,
                byte_len: u64::try_from(row.get::<_, i64>(2)?)
                    .map_err(|_| invalid("negative stored byte length"))?,
                content_hash: VersionedHash {
                    version: row.get(3)?,
                    digest: row.get(4)?,
                },
            };
            let bytes: Vec<u8> = row.get(5)?;
            if bytes.len() as u64 != input.byte_len || input_hash(&bytes) != input.content_hash {
                return Err(invalid("stored captured bytes/hash disagree"));
            }
            if role == InputRole::Source {
                sources.push(SourceFile::new(
                    normalized.clone(),
                    std::str::from_utf8(&bytes)
                        .map_err(|_| invalid("stored source is not UTF-8"))?
                        .into(),
                ));
            }
            manifest.push(input);
            inputs.push(InputBytes {
                path: normalized,
                bytes,
            });
        }
    }
    let mut files = BTreeMap::new();
    {
        let mut stmt = c.prepare(
            "SELECT path,node_id,facts_json FROM files WHERE generation_id=?1 ORDER BY path",
        )?;
        let mut rows = stmt.query([id])?;
        while let Some(row) = rows.next()? {
            let path: String = row.get(0)?;
            let key: String = row.get(1)?;
            let facts: FileFacts = decode(&row.get::<_, String>(2)?)?;
            if facts.path.as_str() != path
                || !facts.symbols.is_empty()
                || NodeId::file(metadata.project.clone(), facts.path.clone()).semantic_key() != key
            {
                return Err(invalid("stored file detail/identity mismatch"));
            }
            files.insert(facts.path.clone(), facts);
        }
    }
    {
        let mut stmt=c.prepare("SELECT node_id,file_path,record_json FROM symbols WHERE generation_id=?1 ORDER BY node_id")?;
        let mut rows = stmt.query([id])?;
        while let Some(row) = rows.next()? {
            let key: String = row.get(0)?;
            let path: String = row.get(1)?;
            let symbol: SymbolFact = decode(&row.get::<_, String>(2)?)?;
            if symbol.identity.semantic_key() != key || symbol.identity.path().as_str() != path {
                return Err(invalid("stored symbol detail/identity mismatch"));
            }
            files
                .get_mut(symbol.identity.path())
                .ok_or_else(|| invalid("symbol file detail missing"))?
                .symbols
                .push(symbol);
        }
    }
    let mut nodes = Vec::new();
    {
        let mut stmt = c.prepare(
            "SELECT node_id,kind,record_json FROM nodes WHERE generation_id=?1 ORDER BY node_id",
        )?;
        let mut rows = stmt.query([id])?;
        while let Some(row) = rows.next()? {
            let key: String = row.get(0)?;
            let kind: String = row.get(1)?;
            let node: GraphNode = decode(&row.get::<_, String>(2)?)?;
            if node.id.semantic_key() != key || tag(&node.kind)? != kind {
                return Err(invalid("stored node kind/identity mismatch"));
            }
            nodes.push(node);
        }
    }
    let mut edges = Vec::new();
    {
        let mut stmt=c.prepare("SELECT source_node_id,target_node_id,kind,confidence,record_json FROM edges WHERE generation_id=?1 ORDER BY ordinal")?;
        let mut rows = stmt.query([id])?;
        while let Some(row) = rows.next()? {
            let source: String = row.get(0)?;
            let target: String = row.get(1)?;
            let kind: String = row.get(2)?;
            let confidence: String = row.get(3)?;
            let edge: Edge = decode(&row.get::<_, String>(4)?)?;
            if edge.source.semantic_key() != source
                || edge.target.semantic_key() != target
                || tag(&edge.kind)? != kind
                || tag(&edge.confidence)? != confidence
            {
                return Err(invalid("stored edge columns/provenance mismatch"));
            }
            edges.push(edge);
        }
    }
    let mut unresolved = Vec::new();
    {
        let mut stmt=c.prepare("SELECT source_node_id,boundary_node_id,record_json FROM unresolved_references WHERE generation_id=?1 ORDER BY ordinal")?;
        let mut rows = stmt.query([id])?;
        while let Some(row) = rows.next()? {
            let source: String = row.get(0)?;
            let boundary: String = row.get(1)?;
            let item: UnresolvedReference = decode(&row.get::<_, String>(2)?)?;
            if item.source.semantic_key() != source || item.boundary.semantic_key() != boundary {
                return Err(invalid("stored unresolved columns/provenance mismatch"));
            }
            unresolved.push(item);
        }
    }
    let sources = SourceSnapshot::new(sources).map_err(|e| invalid(e.to_string()))?;
    let data = SnapshotData {
        metadata,
        inputs: InputManifest::new(manifest).map_err(|e| invalid(e.to_string()))?,
        files: files.into_values().collect(),
        graph: GraphSnapshot::new(nodes, edges, unresolved).map_err(|e| invalid(e.to_string()))?,
    };
    let snapshot = data
        .validate(&sources)
        .map_err(|e| invalid(e.to_string()))?;
    snapshot
        .ensure_compatible(expected)
        .map_err(|e| StorageError::Incompatible(e.to_string()))?;
    if seal(&snapshot)? != checksum {
        return Err(invalid(
            "generation integrity seal mismatch; rebuild required",
        ));
    }
    Ok(Some(LoadedGeneration {
        snapshot,
        inputs,
        sources,
    }))
}
#[cfg(test)]
#[path = "../../../fixtures/storage/support.rs"]
mod fixture;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_reader_pinned_before_publication_loads_one_complete_old_generation() {
        let temp = tempfile::tempdir().unwrap();
        let mut writer = Store::open(temp.path(), fixture::versions()).unwrap();
        let (one, inputs) = fixture::fixture(1, "1", input_hash);
        let inputs = inputs
            .into_iter()
            .map(|(path, bytes)| InputBytes { path, bytes })
            .collect::<Vec<_>>();
        writer.publish(&one, &inputs).unwrap();
        let mut reader = Store::open(temp.path(), fixture::versions()).unwrap();
        let (two, inputs) = fixture::fixture(2, "222", input_hash);
        let inputs = inputs
            .into_iter()
            .map(|(path, bytes)| InputBytes { path, bytes })
            .collect::<Vec<_>>();
        let old = reader
            .read_with_hook(None, || {
                writer.publish(&two, &inputs)?;
                Ok(())
            })
            .unwrap()
            .unwrap();
        assert_eq!(old.snapshot.metadata().generation, 1);
        assert_eq!(
            CanonicalState::from_snapshot(&old.snapshot),
            CanonicalState::from_snapshot(&one)
        );
        let new = reader.load().unwrap().unwrap();
        assert_eq!(new.snapshot.metadata().generation, 2);
        assert_eq!(
            CanonicalState::from_snapshot(&new.snapshot),
            CanonicalState::from_snapshot(&two)
        );
    }
}
