//! Record and failure storage in SQLite.
//!
//! Records live in `users`. Failure counts live in `failures`, keyed by user
//! id whether or not the user exists, so lockout behaves the same for unknown
//! ids and does not reveal which ids are real. Enrolments waiting for
//! confirmation live in `pending`, at most one per user id.

use pixel_factor_core::Record;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

/// A storage failure. The message is for logs, never for callers.
#[derive(Debug)]
pub struct StoreError(String);

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        Self(format!("database: {e}"))
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        Self(format!("stored record: {e}"))
    }
}

/// Failed verifications for one user id.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Failures {
    /// Consecutive failures inside the failure window.
    pub count: u32,
    /// Unix time in seconds until which the id is locked. 0 when not locked.
    pub locked_until: u64,
}

/// One SQLite connection. Callers serialise access.
pub struct Store {
    conn: Connection,
}

fn to_i64(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

impl Store {
    /// Open or create the database at `path`.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        Self::init(Connection::open(path)?)
    }

    /// A database that lives only in memory. For tests.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, StoreError> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS users (
                 user_id TEXT PRIMARY KEY,
                 record  TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS failures (
                 user_id      TEXT PRIMARY KEY,
                 count        INTEGER NOT NULL,
                 locked_until INTEGER NOT NULL,
                 last_failure INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS failures_by_age ON failures (last_failure);
             CREATE TABLE IF NOT EXISTS pending (
                 user_id TEXT PRIMARY KEY,
                 record  TEXT NOT NULL,
                 created INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS pending_by_age ON pending (created);",
        )?;
        Ok(Self { conn })
    }

    /// The stored record, if any.
    pub fn get(&self, user_id: &str) -> Result<Option<Record>, StoreError> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT record FROM users WHERE user_id = ?1",
                params![user_id],
                |row| row.get(0),
            )
            .optional()?;
        json.map(|j| serde_json::from_str(&j))
            .transpose()
            .map_err(StoreError::from)
    }

    /// Store a new record, replacing any earlier one and clearing failures.
    pub fn put(&self, user_id: &str, record: &Record) -> Result<(), StoreError> {
        let json = serde_json::to_string(record)?;
        self.conn.execute(
            "INSERT INTO users (user_id, record) VALUES (?1, ?2)
             ON CONFLICT (user_id) DO UPDATE SET record = ?2",
            params![user_id, json],
        )?;
        self.clear_failures(user_id)
    }

    /// Remove a user, their failures and any pending enrolment. Not an error
    /// if there is none.
    pub fn delete(&self, user_id: &str) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM users WHERE user_id = ?1", params![user_id])?;
        self.conn
            .execute("DELETE FROM pending WHERE user_id = ?1", params![user_id])?;
        self.clear_failures(user_id)
    }

    /// Store an enrolment waiting for confirmation, replacing any earlier one.
    /// The live record and failures are untouched.
    pub fn put_pending(&self, user_id: &str, record: &Record, now: u64) -> Result<(), StoreError> {
        let json = serde_json::to_string(record)?;
        self.conn.execute(
            "INSERT INTO pending (user_id, record, created) VALUES (?1, ?2, ?3)
             ON CONFLICT (user_id) DO UPDATE SET record = ?2, created = ?3",
            params![user_id, json, to_i64(now)],
        )?;
        Ok(())
    }

    /// Remove and return the pending enrolment, if one was stored within the
    /// last `ttl_secs`. Atomic, so of several concurrent callers only one gets
    /// it. An expired one is removed and not returned.
    pub fn take_pending(
        &self,
        user_id: &str,
        now: u64,
        ttl_secs: u64,
    ) -> Result<Option<Record>, StoreError> {
        let row: Option<(String, i64)> = self
            .conn
            .query_row(
                "DELETE FROM pending WHERE user_id = ?1 RETURNING record, created",
                params![user_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((json, created)) = row else {
            return Ok(None);
        };
        if u64::try_from(created).unwrap_or(0) < now.saturating_sub(ttl_secs) {
            return Ok(None);
        }
        Ok(Some(serde_json::from_str(&json)?))
    }

    /// Drop pending enrolments older than `ttl_secs`.
    pub fn prune_pending(&self, now: u64, ttl_secs: u64) -> Result<usize, StoreError> {
        Ok(self.conn.execute(
            "DELETE FROM pending WHERE created < ?1",
            params![to_i64(now.saturating_sub(ttl_secs))],
        )?)
    }

    /// Failures for `user_id` that are still inside the window.
    pub fn failures(
        &self,
        user_id: &str,
        now: u64,
        window_secs: u64,
    ) -> Result<Failures, StoreError> {
        let row: Option<(i64, i64)> = self
            .conn
            .query_row(
                "SELECT count, locked_until FROM failures WHERE user_id = ?1 AND last_failure >= ?2",
                params![user_id, to_i64(now.saturating_sub(window_secs))],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(
            row.map_or_else(Failures::default, |(count, locked_until)| Failures {
                count: u32::try_from(count).unwrap_or(u32::MAX),
                locked_until: u64::try_from(locked_until).unwrap_or(0),
            }),
        )
    }

    /// Count one more failure and return the new count. A count whose last
    /// failure is outside the window starts again at 1. Atomic, so concurrent
    /// failures are all counted.
    pub fn add_failure(
        &self,
        user_id: &str,
        now: u64,
        window_secs: u64,
    ) -> Result<u32, StoreError> {
        let count: i64 = self.conn.query_row(
            "INSERT INTO failures (user_id, count, locked_until, last_failure) VALUES (?1, 1, 0, ?2)
             ON CONFLICT (user_id) DO UPDATE SET
                 count = CASE WHEN last_failure >= ?3 THEN count + 1 ELSE 1 END,
                 locked_until = CASE WHEN last_failure >= ?3 THEN locked_until ELSE 0 END,
                 last_failure = ?2
             RETURNING count",
            params![user_id, to_i64(now), to_i64(now.saturating_sub(window_secs))],
            |row| row.get(0),
        )?;
        Ok(u32::try_from(count).unwrap_or(u32::MAX))
    }

    /// Set when the id's lock ends.
    pub fn set_locked_until(&self, user_id: &str, locked_until: u64) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE failures SET locked_until = ?2 WHERE user_id = ?1",
            params![user_id, to_i64(locked_until)],
        )?;
        Ok(())
    }

    /// Forget an id's failures.
    pub fn clear_failures(&self, user_id: &str) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM failures WHERE user_id = ?1", params![user_id])?;
        Ok(())
    }

    /// Drop failure rows whose window has passed and whose lock has ended, so
    /// failures against made-up ids cannot grow the database without bound.
    pub fn prune_failures(&self, now: u64, window_secs: u64) -> Result<usize, StoreError> {
        Ok(self.conn.execute(
            "DELETE FROM failures WHERE last_failure < ?1 AND locked_until <= ?2",
            params![to_i64(now.saturating_sub(window_secs)), to_i64(now)],
        )?)
    }
}
