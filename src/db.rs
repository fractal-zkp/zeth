use revm::primitives::FixedBytes;
use rusqlite::Connection;
use std::sync::{Arc, Mutex, MutexGuard};
use trace_decoder::BlockTrace;

use super::error::DatabaseError;

#[derive(Debug)]
pub(crate) struct Database {
    connection: Arc<Mutex<Connection>>,
}

impl Database {
    pub(crate) fn new(connection: Arc<Mutex<Connection>>) -> Result<Self, DatabaseError> {
        let database = Self { connection };
        database.create_tables()?;
        Ok(database)
    }

    pub(crate) fn commit_block_trace(
        &self,
        block_hash: FixedBytes<32>,
        block_number: u64,
        block_trace: BlockTrace,
    ) -> Result<(), DatabaseError> {
        let connection = self.connection();
        connection.execute(
            "INSERT INTO block_trace (block_hash, block_number, block_trace) VALUES (?1, ?2, ?3)",
            (
                block_hash.to_string(),
                block_number,
                serde_json::to_string(&block_trace).expect("block trace is serializable"),
            ),
        ).map_err(DatabaseError::FailedToInsertTrace)?;
        Ok(())
    }

    /// Get block trace by block number.
    pub(crate) fn get_block_trace_by_hash(
        &self,
        block_hash: FixedBytes<32>,
    ) -> Result<Option<BlockTrace>, DatabaseError> {
        let connection = self.connection();
        let mut statement = connection
            .prepare("SELECT block_trace FROM block_trace WHERE block_hash = ?1")
            .expect("statement is well formed");
        let mut rows = statement
            .query([block_hash.to_string()])
            .expect("parameter bindings are well formed");
        if let Some(row) = rows.next().map_err(DatabaseError::FailedToGetTrace)? {
            let block_trace: String = row.get(0).expect("column is well formed");
            Ok(Some(
                serde_json::from_str(&block_trace).expect("block trace is deserializable"),
            ))
        } else {
            Ok(None)
        }
    }

    /// Get block trace by block number.
    pub(crate) fn get_block_trace_by_number(
        &self,
        block_number: u64,
    ) -> Result<Option<BlockTrace>, DatabaseError> {
        let connection = self.connection();
        let mut statement = connection
            .prepare("SELECT block_trace FROM block_trace WHERE block_number = ?1")
            .expect("statement is well formed");
        let mut rows = statement
            .query([block_number])
            .expect("parameter bindings are well formed");
        if let Some(row) = rows.next().map_err(DatabaseError::FailedToGetTrace)? {
            let block_trace: String = row.get(0).expect("column is well formed");
            Ok(Some(
                serde_json::from_str(&block_trace).expect("block trace is deserializable"),
            ))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn delete_block_trace_by_hash(
        &self,
        block_hash: FixedBytes<32>,
    ) -> Result<(), DatabaseError> {
        let connection = self.connection();
        connection
            .execute(
                "DELETE FROM block_trace WHERE block_hash = ?1",
                [block_hash.to_string()],
            )
            .map_err(DatabaseError::FailedToDeleteTrace)?;
        Ok(())
    }

    fn connection(&self) -> MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .expect("failed to acquire database lock")
    }

    fn create_tables(&self) -> Result<(), DatabaseError> {
        self.connection()
            .execute(
                "CREATE TABLE IF NOT EXISTS block_trace (
                block_hash TEXT PRIMARY KEY,
                block_number INTEGER NOT NULL,
                block_trace TEXT NOT NULL
            )",
                [],
            )
            .map_err(DatabaseError::FailedToCreateTables)?;
        Ok(())
    }
}
