//! Persistent cold and incremental indexing. Storage implementation is not present yet.

#[cfg(test)]
mod bootstrap {
    #[test]
    fn bundled_sqlite_opens_without_an_external_server() {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        let version: String = connection
            .query_row("SELECT sqlite_version()", [], |row| row.get(0))
            .unwrap();
        assert!(!version.is_empty());
    }
}
