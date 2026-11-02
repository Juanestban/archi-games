//! Local match files (Fortnite's `.replay`) already read. Each one is recorded with what it
//! added, so the running totals can be rebuilt and no file counts twice.

use std::collections::BTreeMap;

use rusqlite::params;

use crate::{Result, Storage, Timestamp};

/// A match file and what it added; `stats` is `None` when it had nothing to add (unreadable,
/// left early…), which still marks it as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayFile {
  pub file_name: String,
  pub file_size: u64,
  pub stats: Option<BTreeMap<String, u64>>,
}

impl Storage {
  /// Files are told apart by name and size: the game reuses neither for another match.
  pub fn replay_seen(&self, game_id: &str, file_name: &str, file_size: u64) -> Result<bool> {
    let count: i64 = self.conn.query_row(
      "SELECT COUNT(*) FROM replays WHERE game_id = ?1 AND file_name = ?2 AND file_size = ?3",
      params![game_id, file_name, file_size as i64],
      |row| row.get(0),
    )?;
    Ok(count > 0)
  }

  /// Marks a file read without adding anything.
  pub fn skip_replay(&self, game_id: &str, account_id: &str, file: &ReplayFile, at: Timestamp) -> Result<()> {
    self.insert_replay(game_id, account_id, &ReplayFile { stats: None, ..file.clone() }, at)
  }

  pub(crate) fn insert_replay(&self, game_id: &str, account_id: &str, file: &ReplayFile, at: Timestamp) -> Result<()> {
    let stats = file.stats.as_ref().map(serde_json::to_string).transpose()?;
    self.conn.execute(
      "INSERT OR IGNORE INTO replays (game_id, account_id, file_name, file_size, processed_at, stats) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
      params![game_id, account_id, file.file_name, file.file_size as i64, at, stats],
    )?;
    Ok(())
  }

  /// Every stat summed over the account's read files.
  pub fn replay_totals(&self, game_id: &str, account_id: &str) -> Result<BTreeMap<String, u64>> {
    let mut stmt = self.conn.prepare("SELECT stats FROM replays WHERE game_id = ?1 AND account_id = ?2 AND stats IS NOT NULL")?;
    let rows = stmt.query_map(params![game_id, account_id], |row| row.get::<_, String>(0))?;
    let mut totals = BTreeMap::new();
    for row in rows {
      let stats: BTreeMap<String, u64> = serde_json::from_str(&row?)?;
      for (stat, value) in stats {
        *totals.entry(stat).or_insert(0) += value;
      }
    }
    Ok(totals)
  }

  /// How many of the account's files added stats.
  pub fn replay_count(&self, game_id: &str, account_id: &str) -> Result<u32> {
    let count: i64 = self.conn.query_row(
      "SELECT COUNT(*) FROM replays WHERE game_id = ?1 AND account_id = ?2 AND stats IS NOT NULL",
      params![game_id, account_id],
      |row| row.get(0),
    )?;
    Ok(count as u32)
  }
}
