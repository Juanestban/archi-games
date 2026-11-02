use rusqlite::Connection;

use crate::{Result, StorageError};

/// Applied in order; `PRAGMA user_version` records how many ran. Never edit a
/// shipped migration, append a new one.
const MIGRATIONS: &[&str] = &[
  r"
    CREATE TABLE snapshots (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        game_id     TEXT    NOT NULL,
        account_id  TEXT    NOT NULL,
        recorded_at INTEGER NOT NULL,
        stats       TEXT    NOT NULL
    );
    CREATE INDEX snapshots_by_account ON snapshots (game_id, account_id, id);

    CREATE TABLE unlocks (
        game_id        TEXT    NOT NULL,
        achievement_id TEXT    NOT NULL,
        unlocked_at    INTEGER NOT NULL,
        PRIMARY KEY (game_id, achievement_id)
    );

    CREATE TABLE accounts (
        game_id      TEXT    PRIMARY KEY,
        account_id   TEXT    NOT NULL,
        display_name TEXT    NOT NULL,
        linked_at    INTEGER NOT NULL
    );

    CREATE TABLE settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
",
  r"
    -- Local match files (e.g. Fortnite .replay) already read, so none counts twice.
    -- `stats`: JSON of what the match added, NULL if it had nothing to add.
    CREATE TABLE replays (
        game_id      TEXT    NOT NULL,
        account_id   TEXT    NOT NULL,
        file_name    TEXT    NOT NULL,
        file_size    INTEGER NOT NULL,
        processed_at INTEGER NOT NULL,
        stats        TEXT,
        PRIMARY KEY (game_id, file_name, file_size)
    );
",
];

pub fn migrate(conn: &mut Connection) -> Result<()> {
  let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
  let version = version as usize;
  if version > MIGRATIONS.len() {
    return Err(StorageError::NewerSchema { found: version, supported: MIGRATIONS.len() });
  }
  for (index, sql) in MIGRATIONS.iter().enumerate().skip(version) {
    let tx = conn.transaction()?;
    tx.execute_batch(sql)?;
    tx.pragma_update(None, "user_version", (index + 1) as i64)?;
    tx.commit()?;
  }
  Ok(())
}
