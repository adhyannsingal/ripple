//! Atomic generation storage and verified cold/incremental captured-input indexing.
pub mod storage;

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

pub mod build;
mod build_capture;

pub mod snapshots;
#[cfg(test)]
mod snapshots_tests;

pub mod fingerprints;
#[cfg(test)]
mod fingerprints_tests;

pub mod incremental;
#[cfg(test)]
mod incremental_tests;

mod invalidation;
#[cfg(test)]
mod invalidation_tests;
