//! Glue between the engine and the database: replaying history, ingesting
//! polls atomically, account XP/level and the platform's meta-achievements.

use std::collections::BTreeMap;

use archi_core::{GameDefinition, GameProgress, LevelInfo, MODE_ALL, Progression, StatSnapshot, Tier};

use crate::{Result, Storage, Timestamp};

/// Game id of the platform's own definition (`games/platform`).
pub const PLATFORM_ID: &str = "platform";

/// Stats the platform definition's conditions can use, all in mode `all`.
pub const PLATFORM_STATS: [&str; 3] = ["platinums", "level", "unlocked"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unlock {
  pub game_id: String,
  pub achievement_id: String,
  pub tier: Tier,
  pub unlocked_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSummary {
  pub total_xp: u64,
  pub level: LevelInfo,
  /// Every unlock, platform ones included.
  pub by_tier: BTreeMap<Tier, u32>,
  pub platinums: u32,
  /// Game unlocks only, so platform achievements can't feed themselves.
  pub game_unlocked: u32,
}

impl AccountSummary {
  /// What the platform definition's conditions read ([`PLATFORM_STATS`]).
  pub fn platform_stats(&self) -> StatSnapshot {
    StatSnapshot::new().with(MODE_ALL, "platinums", self.platinums.into()).with(MODE_ALL, "level", self.level.level.into()).with(
      MODE_ALL,
      "unlocked",
      self.game_unlocked.into(),
    )
  }
}

fn unlock(definition: &GameDefinition, id: &str, tier: Tier, at: Timestamp) -> Unlock {
  Unlock { game_id: definition.game.id.clone(), achievement_id: id.to_owned(), tier, unlocked_at: at }
}

impl Storage {
  /// Rebuilds a game's progress for the linked account by replaying its
  /// stored snapshots. Unlocks that history earns but the database lacks
  /// (achievements added since) are stored with the date of the snapshot
  /// that earned them and returned. Stored unlocks are never revoked.
  pub fn rebuild_progress(&self, definition: &GameDefinition, account_id: &str) -> Result<(GameProgress, Vec<Unlock>)> {
    let game_id = &definition.game.id;
    let tx = self.conn.unchecked_transaction()?;
    let mut progress = GameProgress::default();
    let mut retroactive = Vec::new();

    for (at, snapshot) in self.snapshots(game_id, account_id)? {
      for achievement in progress.apply(definition, snapshot) {
        if self.insert_unlock(game_id, &achievement.id, at)? {
          retroactive.push(unlock(definition, &achievement.id, achievement.tier, at));
        }
      }
    }
    let stored = self.unlocked_ids(game_id)?;
    progress.unlocked.extend(stored.into_iter().filter(|id| definition.get(id).is_some()));

    tx.commit()?;
    Ok((progress, retroactive))
  }

  /// Feeds a fresh poll: stores the snapshot and its unlocks in one
  /// transaction. Identical consecutive polls are ignored.
  pub fn ingest(
    &self,
    definition: &GameDefinition,
    account_id: &str,
    progress: &mut GameProgress,
    snapshot: StatSnapshot,
    at: Timestamp,
  ) -> Result<Vec<Unlock>> {
    if progress.last.as_ref() == Some(&snapshot) {
      return Ok(Vec::new());
    }
    let game_id = &definition.game.id;
    let tx = self.conn.unchecked_transaction()?;
    self.record_snapshot(game_id, account_id, &snapshot, at)?;

    let mut unlocked = Vec::new();
    for achievement in progress.apply(definition, snapshot) {
      if self.insert_unlock(game_id, &achievement.id, at)? {
        unlocked.push(unlock(definition, &achievement.id, achievement.tier, at));
      }
    }
    tx.commit()?;
    Ok(unlocked)
  }

  /// XP, level and trophy counts from every stored unlock that still exists
  /// in `definitions` (the platform's included).
  pub fn account_summary(&self, definitions: &[&GameDefinition], rules: &Progression) -> Result<AccountSummary> {
    let mut total_xp = 0;
    let mut by_tier = BTreeMap::new();
    let mut game_unlocked = 0;

    for record in self.unlocks()? {
      let Some(definition) = definitions.iter().find(|d| d.game.id == record.game_id) else { continue };
      let Some(achievement) = definition.get(&record.achievement_id) else { continue };
      total_xp += rules.xp_for(achievement.tier);
      *by_tier.entry(achievement.tier).or_insert(0) += 1;
      if definition.game.id != PLATFORM_ID {
        game_unlocked += 1;
      }
    }

    Ok(AccountSummary {
      total_xp,
      level: rules.level_for(total_xp),
      platinums: by_tier.get(&Tier::Platinum).copied().unwrap_or(0),
      by_tier,
      game_unlocked,
    })
  }

  /// Unlocks the platform's meta-achievements. Loops until stable, because
  /// a meta-achievement's XP can raise the level and unlock another one.
  pub fn evaluate_platform(&self, platform: &GameDefinition, games: &[&GameDefinition], rules: &Progression, at: Timestamp) -> Result<Vec<Unlock>> {
    let mut definitions = games.to_vec();
    definitions.push(platform);
    let mut unlocked = Vec::new();

    loop {
      let stats = self.account_summary(&definitions, rules)?.platform_stats();
      let mut progress = GameProgress::with_baseline(StatSnapshot::new());
      progress.unlocked = self.unlocked_ids(&platform.game.id)?;
      let new = progress.apply(platform, stats);
      if new.is_empty() {
        return Ok(unlocked);
      }
      for achievement in new {
        self.insert_unlock(&platform.game.id, &achievement.id, at)?;
        unlocked.push(unlock(platform, &achievement.id, achievement.tier, at));
      }
    }
  }
}
