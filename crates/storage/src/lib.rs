//! Local persistence (SQLite): stat snapshots, unlocks, linked accounts and
//! settings. [`ledger`] ties it to the `core` engine.

mod ledger;
mod schema;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use archi_core::StatSnapshot;
use directories::BaseDirs;
use rusqlite::{Connection, OptionalExtension, params};
use thiserror::Error;

pub use ledger::{AccountSummary, PLATFORM_ID, PLATFORM_STATS, Unlock};

/// Seconds since the Unix epoch.
pub type Timestamp = i64;

pub fn unix_now() -> Timestamp {
  SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as Timestamp)
}

#[derive(Debug, Error)]
pub enum StorageError {
  #[error("database error: {0}")]
  Sqlite(#[from] rusqlite::Error),
  #[error("corrupt snapshot: {0}")]
  Json(#[from] serde_json::Error),
  #[error("could not create the data directory: {0}")]
  Io(#[from] std::io::Error),
  #[error("no user data directory on this system")]
  NoDataDir,
  #[error("database schema v{found} is newer than this app supports (v{supported})")]
  NewerSchema { found: usize, supported: usize },
}

pub type Result<T> = std::result::Result<T, StorageError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedAccount {
  pub game_id: String,
  pub account_id: String,
  pub display_name: String,
  pub linked_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockRecord {
  pub game_id: String,
  pub achievement_id: String,
  pub unlocked_at: Timestamp,
}

pub struct Storage {
  conn: Connection,
}

impl Storage {
  /// `%APPDATA%\archi-games\archi-games.db` on Windows, the platform's user
  /// data directory elsewhere.
  pub fn default_path() -> Result<PathBuf> {
    let dirs = BaseDirs::new().ok_or(StorageError::NoDataDir)?;
    Ok(dirs.data_dir().join("archi-games").join("archi-games.db"))
  }

  pub fn open(path: &Path) -> Result<Self> {
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    Self::init(conn)
  }

  pub fn open_in_memory() -> Result<Self> {
    Self::init(Connection::open_in_memory()?)
  }

  fn init(mut conn: Connection) -> Result<Self> {
    conn.pragma_update(None, "foreign_keys", true)?;
    schema::migrate(&mut conn)?;
    Ok(Self { conn })
  }

  pub fn last_snapshot(&self, game_id: &str, account_id: &str) -> Result<Option<StatSnapshot>> {
    let stats: Option<String> = self
      .conn
      .query_row("SELECT stats FROM snapshots WHERE game_id = ?1 AND account_id = ?2 ORDER BY id DESC LIMIT 1", params![game_id, account_id], |row| {
        row.get(0)
      })
      .optional()?;
    Ok(stats.map(|s| serde_json::from_str(&s)).transpose()?)
  }

  /// Stores a poll result. Returns `false` without writing when it equals the
  /// previous snapshot of that account: identical polls change nothing in the
  /// engine, so skipping them keeps history small without altering replays.
  pub fn record_snapshot(&self, game_id: &str, account_id: &str, snapshot: &StatSnapshot, at: Timestamp) -> Result<bool> {
    if self.last_snapshot(game_id, account_id)?.as_ref() == Some(snapshot) {
      return Ok(false);
    }
    self.conn.execute(
      "INSERT INTO snapshots (game_id, account_id, recorded_at, stats) VALUES (?1, ?2, ?3, ?4)",
      params![game_id, account_id, at, serde_json::to_string(snapshot)?],
    )?;
    Ok(true)
  }

  /// Every stored snapshot of an account, oldest first.
  pub fn snapshots(&self, game_id: &str, account_id: &str) -> Result<Vec<(Timestamp, StatSnapshot)>> {
    let mut stmt = self.conn.prepare("SELECT recorded_at, stats FROM snapshots WHERE game_id = ?1 AND account_id = ?2 ORDER BY id")?;
    let rows = stmt.query_map(params![game_id, account_id], |row| Ok((row.get::<_, Timestamp>(0)?, row.get::<_, String>(1)?)))?;
    rows
      .map(|row| {
        let (at, stats) = row?;
        Ok((at, serde_json::from_str(&stats)?))
      })
      .collect()
  }

  /// Returns `false` if it was already unlocked; the original date is kept.
  pub fn insert_unlock(&self, game_id: &str, achievement_id: &str, at: Timestamp) -> Result<bool> {
    let inserted = self
      .conn
      .execute("INSERT OR IGNORE INTO unlocks (game_id, achievement_id, unlocked_at) VALUES (?1, ?2, ?3)", params![game_id, achievement_id, at])?;
    Ok(inserted == 1)
  }

  pub fn unlocked_ids(&self, game_id: &str) -> Result<BTreeSet<String>> {
    let mut stmt = self.conn.prepare("SELECT achievement_id FROM unlocks WHERE game_id = ?1")?;
    let rows = stmt.query_map([game_id], |row| row.get(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
  }

  /// All unlocks across games, most recent first (the history screen).
  pub fn unlocks(&self) -> Result<Vec<UnlockRecord>> {
    let mut stmt = self.conn.prepare("SELECT game_id, achievement_id, unlocked_at FROM unlocks ORDER BY unlocked_at DESC, rowid DESC")?;
    let rows = stmt.query_map([], |row| Ok(UnlockRecord { game_id: row.get(0)?, achievement_id: row.get(1)?, unlocked_at: row.get(2)? }))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
  }

  pub fn account(&self, game_id: &str) -> Result<Option<LinkedAccount>> {
    Ok(
      self
        .conn
        .query_row("SELECT game_id, account_id, display_name, linked_at FROM accounts WHERE game_id = ?1", [game_id], |row| {
          Ok(LinkedAccount { game_id: row.get(0)?, account_id: row.get(1)?, display_name: row.get(2)?, linked_at: row.get(3)? })
        })
        .optional()?,
    )
  }

  /// Links (or replaces) the account tracked for a game. Snapshots stay
  /// per account, so a different account starts from a fresh baseline while
  /// earned unlocks are kept.
  pub fn link_account(&self, account: &LinkedAccount) -> Result<()> {
    self.conn.execute(
      "INSERT INTO accounts (game_id, account_id, display_name, linked_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (game_id) DO UPDATE SET
                account_id = excluded.account_id,
                display_name = excluded.display_name,
                linked_at = excluded.linked_at",
      params![account.game_id, account.account_id, account.display_name, account.linked_at],
    )?;
    Ok(())
  }

  pub fn unlink_account(&self, game_id: &str) -> Result<()> {
    self.conn.execute("DELETE FROM accounts WHERE game_id = ?1", [game_id])?;
    Ok(())
  }

  pub fn setting(&self, key: &str) -> Result<Option<String>> {
    Ok(self.conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| row.get(0)).optional()?)
  }

  pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
    self.conn.execute(
      "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
      params![key, value],
    )?;
    Ok(())
  }

  pub fn delete_setting(&self, key: &str) -> Result<()> {
    self.conn.execute("DELETE FROM settings WHERE key = ?1", [key])?;
    Ok(())
  }
}
