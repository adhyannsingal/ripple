#[path = "../../../fixtures/storage/support.rs"]
mod support;
use ripple_core::{canonical::CanonicalState, index::IndexSnapshot};
use ripple_index::storage::{
    InputBytes, MetadataStatus, MetadataSummary, StorageError, Store, input_hash,
};
use rusqlite::Connection;
use std::fs;
fn fixture(generation: u64, value: &str) -> (IndexSnapshot, Vec<InputBytes>) {
    let (snapshot, inputs) = support::fixture(generation, value, input_hash);
    (
        snapshot,
        inputs
            .into_iter()
            .map(|(path, bytes)| InputBytes { path, bytes })
            .collect(),
    )
}
#[test]
fn typed_generation_roundtrips_after_reopen_with_all_input_roles_and_uncertainty() {
    let temp = tempfile::tempdir().unwrap();
    let (snapshot, inputs) = fixture(1, "1");
    {
        let mut store = Store::open(temp.path(), support::versions()).unwrap();
        assert!(store.load().unwrap().is_none());
        assert_eq!(store.next_generation().unwrap(), 1);
        let receipt = store.publish(&snapshot, &inputs).unwrap();
        assert_eq!(receipt.generation, 1);
        assert!(matches!(
            receipt.metadata,
            MetadataStatus::Updated { generation: 1 }
        ));
    }
    let mut store = Store::open(temp.path(), support::versions()).unwrap();
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(
        CanonicalState::from_snapshot(&snapshot),
        CanonicalState::from_snapshot(&loaded.snapshot)
    );
    for input in &inputs {
        assert!(loaded.inputs.contains(input));
    }
    assert_eq!(loaded.snapshot.graph().unresolved().len(), 1);
    assert_eq!(loaded.snapshot.graph().nodes().len(), 4);
    assert_eq!(store.next_generation().unwrap(), 2);
    assert!(store.load_generation(99).unwrap().is_none());
}
#[test]
fn generations_are_monotonic_immutable_and_old_snapshots_remain_loadable() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Store::open(temp.path(), support::versions()).unwrap();
    let (first, bytes) = fixture(1, "1");
    store.publish(&first, &bytes).unwrap();
    let first_inputs = bytes.clone();
    let (second, bytes) = fixture(2, "22");
    store.publish(&second, &bytes).unwrap();
    assert!(store.publish(&first, &first_inputs).is_err());
    assert_eq!(
        store
            .load()
            .unwrap()
            .unwrap()
            .snapshot
            .metadata()
            .generation,
        2
    );
    assert_eq!(
        CanonicalState::from_snapshot(&store.load_generation(1).unwrap().unwrap().snapshot),
        CanonicalState::from_snapshot(&first)
    );
    assert_ne!(
        CanonicalState::from_snapshot(&first),
        CanonicalState::from_snapshot(&second)
    );
}
#[test]
fn injected_mid_write_failure_rolls_back_every_table_and_pointer() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Store::open(temp.path(), support::versions()).unwrap();
    let (first, bytes) = fixture(1, "1");
    store.publish(&first, &bytes).unwrap();
    let raw = Connection::open(temp.path().join("index.db")).unwrap();
    raw.execute_batch("CREATE TRIGGER injected_failure BEFORE INSERT ON edges BEGIN SELECT RAISE(ABORT,'injected write failure'); END;").unwrap();
    let metadata_before = fs::read(temp.path().join("metadata.json")).unwrap();
    let (second, bytes) = fixture(2, "2");
    assert!(store.publish(&second, &bytes).is_err());
    assert_eq!(
        metadata_before,
        fs::read(temp.path().join("metadata.json")).unwrap()
    );
    assert_eq!(
        raw.query_row("SELECT count(*) FROM generations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
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
    assert!(store.load_generation(2).unwrap().is_none());
    assert_eq!(
        CanonicalState::from_snapshot(&store.load().unwrap().unwrap().snapshot),
        CanonicalState::from_snapshot(&first)
    );
}
#[test]
fn foreign_keys_reject_dangling_and_cross_generation_endpoints() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Store::open(temp.path(), support::versions()).unwrap();
    let (snapshot, inputs) = fixture(1, "1");
    store.publish(&snapshot, &inputs).unwrap();
    let raw = Connection::open(temp.path().join("index.db")).unwrap();
    raw.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    assert!(
        raw.execute(
            "INSERT INTO edges VALUES(1,99,'missing','missing','CALLS','RESOLVED','{}')",
            []
        )
        .is_err()
    );
    assert!(
        raw.execute("INSERT INTO current_generation VALUES(2,1)", [])
            .is_err()
    );
    raw.execute("INSERT INTO generations VALUES(2,'{}','fake',0)", [])
        .unwrap();
    assert!(
        raw.execute(
            "UPDATE current_generation SET generation_id=2 WHERE singleton=1",
            []
        )
        .is_err()
    );
}
#[test]
fn concurrent_writer_is_actionable_and_does_not_publish_a_partial_generation() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Store::open(temp.path(), support::versions()).unwrap();
    let (one, inputs) = fixture(1, "1");
    store.publish(&one, &inputs).unwrap();
    let raw = Connection::open(temp.path().join("index.db")).unwrap();
    raw.execute_batch("BEGIN IMMEDIATE;").unwrap();
    let (two, inputs) = fixture(2, "2");
    assert!(matches!(
        store.publish(&two, &inputs),
        Err(StorageError::Busy)
    ));
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
    raw.execute_batch("ROLLBACK;").unwrap();
    store.publish(&two, &inputs).unwrap();
    assert_eq!(
        store
            .load()
            .unwrap()
            .unwrap()
            .snapshot
            .metadata()
            .generation,
        2
    );
}
#[test]
fn metadata_missing_corrupt_or_stale_is_rebuilt_from_the_database() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Store::open(temp.path(), support::versions()).unwrap();
    let (snapshot, inputs) = fixture(1, "1");
    store.publish(&snapshot, &inputs).unwrap();
    assert!(matches!(
        store.recover_metadata().unwrap(),
        MetadataStatus::Current { generation: 1 }
    ));
    for bytes in [None, Some(b"garbage".as_slice()), Some(b"{}".as_slice())] {
        let path = temp.path().join("metadata.json");
        if let Some(bytes) = bytes {
            fs::write(&path, bytes).unwrap();
        } else {
            fs::remove_file(&path).unwrap();
        }
        assert!(matches!(
            store.recover_metadata().unwrap(),
            MetadataStatus::Updated { generation: 1 }
        ));
        let summary: MetadataSummary = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(summary.metadata, *snapshot.metadata());
        assert_eq!(summary.node_count, 4);
    }
    let old = fs::read(temp.path().join("metadata.json")).unwrap();
    let (second, inputs) = fixture(2, "2");
    store.publish(&second, &inputs).unwrap();
    fs::write(temp.path().join("metadata.json"), old).unwrap();
    assert_eq!(
        store
            .load()
            .unwrap()
            .unwrap()
            .snapshot
            .metadata()
            .generation,
        2
    );
    assert!(matches!(
        store.recover_metadata().unwrap(),
        MetadataStatus::Updated { generation: 2 }
    ));
    let summary: MetadataSummary =
        serde_json::from_slice(&fs::read(temp.path().join("metadata.json")).unwrap()).unwrap();
    assert_eq!(summary.metadata.generation, 2);
}
#[test]
fn metadata_failure_after_database_commit_is_reported_as_committed_and_recoverable() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Store::open(temp.path(), support::versions()).unwrap();
    fs::create_dir(temp.path().join("metadata.json")).unwrap();
    let (snapshot, inputs) = fixture(1, "1");
    let receipt = store.publish(&snapshot, &inputs).unwrap();
    assert!(matches!(receipt.metadata, MetadataStatus::Pending { .. }));
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
    fs::remove_dir(temp.path().join("metadata.json")).unwrap();
    assert!(matches!(
        store.recover_metadata().unwrap(),
        MetadataStatus::Updated { generation: 1 }
    ));
}
#[test]
fn source_config_and_ignore_bytes_must_match_the_captured_manifest() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Store::open(temp.path(), support::versions()).unwrap();
    let (snapshot, inputs) = fixture(1, "1");
    for index in 0..inputs.len() {
        let mut bad = inputs.clone();
        bad[index].bytes.push(b' ');
        assert!(store.publish(&snapshot, &bad).is_err());
        assert!(store.load().unwrap().is_none());
    }
    assert!(
        store
            .publish(&snapshot, &inputs[..inputs.len() - 1])
            .is_err()
    );
    let mut duplicate = inputs.clone();
    duplicate.push(inputs[0].clone());
    assert!(store.publish(&snapshot, &duplicate).is_err());
    store.publish(&snapshot, &inputs).unwrap();
}
#[test]
fn stored_bytes_and_semantic_record_corruption_are_detected_on_load() {
    for statement in [
        "UPDATE captured_inputs SET bytes=zeroblob(byte_len) WHERE role='ignore_policy'",
        "DELETE FROM edges WHERE kind='IMPORTS'",
        "UPDATE nodes SET kind='FILE' WHERE kind='TEST_FILE'",
        "DELETE FROM current_generation",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let mut store = Store::open(temp.path(), support::versions()).unwrap();
        let (snapshot, inputs) = fixture(1, "1");
        store.publish(&snapshot, &inputs).unwrap();
        let raw = Connection::open(temp.path().join("index.db")).unwrap();
        raw.execute_batch(statement).unwrap();
        assert!(store.load().is_err(), "{statement}");
    }
}
#[test]
fn schema_analyzer_and_migration_incompatibility_never_silently_reuses_state() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Store::open(temp.path(), support::versions()).unwrap();
    let (snapshot, inputs) = fixture(1, "1");
    store.publish(&snapshot, &inputs).unwrap();
    let mut changed = support::versions();
    changed.analyzer_version = "new-analyzer".into();
    let mut reader = Store::open(temp.path(), changed).unwrap();
    assert!(matches!(reader.load(), Err(StorageError::Incompatible(_))));
    drop(reader);
    drop(store);
    let raw = Connection::open(temp.path().join("index.db")).unwrap();
    raw.execute_batch("PRAGMA user_version=999;").unwrap();
    drop(raw);
    assert!(matches!(
        Store::open(temp.path(), support::versions()),
        Err(StorageError::Incompatible(_))
    ));
    let other = tempfile::tempdir().unwrap();
    let store = Store::open(other.path(), support::versions()).unwrap();
    drop(store);
    let raw = Connection::open(other.path().join("index.db")).unwrap();
    raw.execute("UPDATE schema_migrations SET checksum='bad'", [])
        .unwrap();
    drop(raw);
    assert!(matches!(
        Store::open(other.path(), support::versions()),
        Err(StorageError::Incompatible(_))
    ));
}
#[test]
fn unrecognized_database_is_not_reinitialized() {
    let temp = tempfile::tempdir().unwrap();
    let raw = Connection::open(temp.path().join("index.db")).unwrap();
    raw.execute_batch("CREATE TABLE unrelated(value TEXT); INSERT INTO unrelated VALUES('keep');")
        .unwrap();
    assert!(Store::open(temp.path(), support::versions()).is_err());
    assert_eq!(
        raw.query_row("SELECT value FROM unrelated", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "keep"
    );
}
#[cfg(unix)]
#[test]
fn observed_symlink_database_and_directory_are_rejected() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    symlink(target.path(), temp.path().join("index")).unwrap();
    assert!(Store::open(temp.path().join("index"), support::versions()).is_err());
    fs::write(target.path().join("keep"), b"keep").unwrap();
    symlink(target.path().join("keep"), temp.path().join("index.db")).unwrap();
    assert!(Store::open(temp.path(), support::versions()).is_err());
    assert_eq!(fs::read(target.path().join("keep")).unwrap(), b"keep");
}
#[test]
fn stale_writer_counters_cannot_overwrite_a_newer_complete_generation() {
    let temp = tempfile::tempdir().unwrap();
    let mut first = Store::open(temp.path(), support::versions()).unwrap();
    let (one, inputs) = fixture(1, "1");
    first.publish(&one, &inputs).unwrap();
    let mut second = Store::open(temp.path(), support::versions()).unwrap();
    assert_eq!(first.next_generation().unwrap(), 2);
    assert_eq!(second.next_generation().unwrap(), 2);
    let (two, inputs) = fixture(2, "2");
    first.publish(&two, &inputs).unwrap();
    let (stale, inputs) = fixture(2, "99");
    let error = second.publish(&stale, &inputs).unwrap_err();
    assert!(error.to_string().contains("generation conflict"));
    assert_eq!(
        CanonicalState::from_snapshot(&second.load().unwrap().unwrap().snapshot),
        CanonicalState::from_snapshot(&two)
    );
}
#[test]
fn oversized_capture_and_stored_blob_fail_explicitly_without_partial_output() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Store::open(temp.path(), support::versions()).unwrap();
    let (snapshot, inputs) = fixture(1, "1");
    let mut oversized = inputs.clone();
    oversized[0].bytes = vec![b'x'; 16 * 1024 * 1024 + 1];
    assert!(
        store
            .publish(&snapshot, &oversized)
            .unwrap_err()
            .to_string()
            .contains("16 MiB")
    );
    assert!(store.load().unwrap().is_none());
    drop(oversized);
    store.publish(&snapshot, &inputs).unwrap();
    let raw = Connection::open(temp.path().join("index.db")).unwrap();
    raw.execute(
        "UPDATE captured_inputs SET byte_len=?1,bytes=zeroblob(?1) WHERE role='ignore_policy'",
        [16 * 1024 * 1024 + 1],
    )
    .unwrap();
    assert!(
        store
            .load()
            .unwrap_err()
            .to_string()
            .contains("budget exceeded")
    );
}
