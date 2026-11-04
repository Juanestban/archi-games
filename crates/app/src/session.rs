//! A linked account's live session: rebuilds progress from history at
//! startup, then turns each poll and each local match file into stored unlocks.
//!
//! Local matches (Fortnite's replay files, CS2's Game State Integration) add to a mode of
//! their own: `replay` for Fortnite, whose totals are merged into every API poll too so the
//! mode never reads as a drop between two polls; `all` for CS2, which has no API.

use std::collections::BTreeMap;

use archi_core::{GameDefinition, GameProgress, GameTracker, MODE_ALL, Progression, StatSnapshot, TrackError};
use archi_storage::{ReplayFile, Storage, StorageError, Timestamp, Unlock, unix_now};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SessionError {
  #[error(transparent)]
  Storage(#[from] StorageError),
  #[error(transparent)]
  Track(#[from] TrackError),
}

pub struct Session<'a> {
  storage: &'a Storage,
  game: &'a GameDefinition,
  /// Every real game, for the platform's meta-achievements.
  games: Vec<&'a GameDefinition>,
  platform: &'a GameDefinition,
  rules: &'a Progression,
  account_id: String,
  progress: GameProgress,
  /// Every local match's stats, summed.
  replay_totals: BTreeMap<String, u64>,
  /// The mode local matches add to: the game's first `local_modes` (`all` without one).
  local_mode: &'a str,
}

impl<'a> Session<'a> {
  /// Also returns what history earned but wasn't stored yet (achievements
  /// added since the last run).
  pub fn start(
    storage: &'a Storage,
    game: &'a GameDefinition,
    games: Vec<&'a GameDefinition>,
    platform: &'a GameDefinition,
    rules: &'a Progression,
    account_id: &str,
  ) -> Result<(Self, Vec<Unlock>), StorageError> {
    let (progress, mut unlocks) = storage.rebuild_progress(game, account_id)?;
    unlocks.extend(storage.evaluate_platform(platform, &games, rules, unix_now())?);
    let replay_totals = storage.replay_totals(&game.game.id, account_id)?;
    let local_mode = game.game.local_modes.first().map_or(MODE_ALL, String::as_str);
    let account_id = account_id.to_owned();
    Ok((Self { storage, game, games, platform, rules, account_id, progress, replay_totals, local_mode }, unlocks))
  }

  pub fn account_id(&self) -> &str {
    &self.account_id
  }

  pub fn game_id(&self) -> &str {
    &self.game.game.id
  }

  fn with_replays(&self, totals: &BTreeMap<String, u64>, mut snapshot: StatSnapshot) -> StatSnapshot {
    for (stat, &value) in totals {
      snapshot.set(self.local_mode, stat, value);
    }
    snapshot
  }

  /// For a game without an API (CS2): an empty first reading, so the first match counts in
  /// full instead of becoming the baseline.
  pub fn start_from_zero(&mut self, at: Timestamp) -> Result<(), StorageError> {
    if self.progress.last.is_none() {
      self.record(StatSnapshot::new(), at)?;
    }
    Ok(())
  }

  /// Adds a local match file: a new snapshot (the last API stats plus the new replay totals)
  /// stored together with the file, so it counts exactly once. A file without stats is only
  /// marked as read.
  pub fn record_replay(&mut self, file: ReplayFile, at: Timestamp) -> Result<Vec<Unlock>, StorageError> {
    let Some(stats) = &file.stats else {
      self.storage.skip_replay(&self.game.game.id, &self.account_id, &file, at)?;
      return Ok(Vec::new());
    };
    let mut totals = self.replay_totals.clone();
    for (stat, value) in stats {
      *totals.entry(stat.clone()).or_insert(0) += value;
    }
    let snapshot = self.with_replays(&totals, self.progress.last.clone().unwrap_or_default());
    let mut unlocks = self.storage.ingest_replay(self.game, &self.account_id, &mut self.progress, snapshot, &file, at)?;
    self.replay_totals = totals;
    if !unlocks.is_empty() {
      unlocks.extend(self.storage.evaluate_platform(self.platform, &self.games, self.rules, at)?);
    }
    Ok(unlocks)
  }

  pub fn progress(&self) -> &GameProgress {
    &self.progress
  }

  /// Unlocks in notification order: the game's (platinum last), then the
  /// platform's meta-achievements.
  pub fn record(&mut self, snapshot: StatSnapshot, at: Timestamp) -> Result<Vec<Unlock>, StorageError> {
    let mut unlocks = self.storage.ingest(self.game, &self.account_id, &mut self.progress, snapshot, at)?;
    if !unlocks.is_empty() {
      unlocks.extend(self.storage.evaluate_platform(self.platform, &self.games, self.rules, at)?);
    }
    Ok(unlocks)
  }

  pub async fn poll(&mut self, tracker: &impl GameTracker) -> Result<Vec<Unlock>, SessionError> {
    let snapshot = self.with_replays(&self.replay_totals, tracker.fetch(&self.account_id).await?);
    Ok(self.record(snapshot, unix_now())?)
  }
}

#[cfg(test)]
mod tests {
  use std::collections::VecDeque;
  use std::sync::Mutex;

  use archi_core::{AccountInfo, MATCHES, MODE_ALL};

  use super::*;

  /// Simulated API: replays recorded polls in order.
  struct ReplayTracker {
    polls: Mutex<VecDeque<Result<StatSnapshot, TrackError>>>,
  }

  impl ReplayTracker {
    fn new(polls: impl IntoIterator<Item = Result<StatSnapshot, TrackError>>) -> Self {
      Self { polls: Mutex::new(polls.into_iter().collect()) }
    }
  }

  impl GameTracker for ReplayTracker {
    fn game_id(&self) -> &str {
      "fortnite"
    }

    fn process_names(&self) -> &[&str] {
      &[]
    }

    async fn find_account(&self, name: &str) -> Result<AccountInfo, TrackError> {
      Ok(AccountInfo { id: "acc".into(), display_name: name.into() })
    }

    async fn fetch(&self, _account_id: &str) -> Result<StatSnapshot, TrackError> {
      self.polls.lock().unwrap().pop_front().expect("no more recorded polls")
    }
  }

  fn definitions() -> (GameDefinition, GameDefinition) {
    let fortnite = GameDefinition::from_toml_str(include_str!("../../../games/fortnite/achievements.toml")).unwrap();
    let platform = GameDefinition::from_toml_str(include_str!("../../../games/platform/achievements.toml")).unwrap();
    (fortnite, platform)
  }

  /// Lifetime totals, with every match so far played in solo.
  fn totals(matches: u64, wins: u64, kills: u64) -> StatSnapshot {
    let mut snapshot = StatSnapshot::new();
    for mode in [MODE_ALL, "solo"] {
      snapshot.set(mode, MATCHES, matches);
      snapshot.set(mode, "wins", wins);
      snapshot.set(mode, "kills", kills);
    }
    snapshot
  }

  fn ids(unlocks: &[Unlock]) -> Vec<&str> {
    unlocks.iter().map(|u| u.achievement_id.as_str()).collect()
  }

  fn replay(name: &str, damage: u64, damage_taken: u64, won: bool) -> ReplayFile {
    let stats = [("matches", 1), ("wins", u64::from(won)), ("damage", damage), ("damage_taken", damage_taken), ("accuracy", 20), ("eliminations", 1)];
    ReplayFile { file_name: name.into(), file_size: 10, stats: Some(stats.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()) }
  }

  /// Rebuilds a real database (a copy) and lists what its history unlocks:
  /// `ARCHI_DB_COPY=<copy> cargo test -p archi-games rebuilds_a_real_history -- --ignored --nocapture`.
  #[test]
  #[ignore = "needs a copy of a real database"]
  fn rebuilds_a_real_history() {
    let storage = Storage::open(std::path::Path::new(&std::env::var("ARCHI_DB_COPY").unwrap())).unwrap();
    let catalog = crate::catalog::Catalog::bundled().unwrap();
    for game in catalog.games() {
      let Some(account) = storage.account(&game.game.id).unwrap() else { continue };
      let (session, unlocks) = Session::start(&storage, game, catalog.games(), &catalog.platform, &catalog.rules, &account.account_id).unwrap();
      println!("{}: retroactive {:?}, unlocked {:?}", game.game.id, ids(&unlocks), session.progress().unlocked);
    }
  }

  /// CS2 has no API: a zero baseline, then each match adds to `all`.
  #[test]
  fn local_matches_count_in_full_for_a_game_without_an_api() {
    let cs2 = GameDefinition::from_toml_str(include_str!("../../../games/cs2/achievements.toml")).unwrap();
    let (_, platform) = definitions();
    let rules = Progression::default();
    let storage = Storage::open_in_memory().unwrap();
    let (mut session, _) = Session::start(&storage, &cs2, vec![&cs2], &platform, &rules, "steam").unwrap();
    session.start_from_zero(1).unwrap();
    session.start_from_zero(1).unwrap();

    let a_match = |id: &str, kills: u64, won: u64| ReplayFile {
      file_name: id.into(),
      file_size: 0,
      stats: Some([("matches", 1), ("matches_won", won), ("kills", kills), ("aces", 1)].into_iter().map(|(k, v)| (k.to_owned(), v)).collect()),
    };
    let unlocked = session.record_replay(a_match("de_dust2-1", 26, 1), 2).unwrap();
    assert_eq!(ids(&unlocked), ["first_win", "ace", "match_kills_25"]);
    assert!(session.record_replay(a_match("de_mirage-2", 3, 0), 3).unwrap().is_empty());
    assert_eq!(session.progress().last.as_ref().unwrap().get(MODE_ALL, "kills"), 29);
    assert_eq!(storage.snapshots("cs2", "steam").unwrap().len(), 3, "the zero baseline is stored once");
  }

  /// League of Legends: like CS2, each finished match adds to `all` from a zero baseline.
  #[test]
  fn a_league_match_unlocks_base_and_pack_achievements() {
    let lol = GameDefinition::from_toml_str(include_str!("../../../games/lol/achievements.toml")).unwrap();
    let (_, platform) = definitions();
    let rules = Progression::default();
    let storage = Storage::open_in_memory().unwrap();
    let (mut session, _) = Session::start(&storage, &lol, vec![&lol], &platform, &rules, "Riot Tuxedo#EUW").unwrap();
    session.start_from_zero(1).unwrap();
    let stats = [("matches", 1), ("wins", 1), ("wins_classic", 1), ("kills", 15), ("deaths", 0), ("penta_kills", 1), ("first_blood", 1)];
    let file =
      ReplayFile { file_name: "classic-8760".into(), file_size: 0, stats: Some(stats.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()) };
    let unlocked = session.record_replay(file, 2).unwrap();
    assert_eq!(ids(&unlocked), ["first_win", "first_blood", "match_kills_15", "perfect_game", "penta", "match_kills_10", "classic_win"]);
  }

  #[tokio::test]
  async fn replays_unlock_match_achievements_and_survive_api_polls() {
    let (fortnite, platform) = definitions();
    let rules = Progression::default();
    let storage = Storage::open_in_memory().unwrap();
    let (mut session, _) = Session::start(&storage, &fortnite, vec![&fortnite], &platform, &rules, "acc").unwrap();
    let tracker = ReplayTracker::new([Ok(totals(10, 0, 5)), Ok(totals(10, 0, 5))]);
    session.poll(&tracker).await.unwrap();

    assert_eq!(ids(&session.record_replay(replay("1.replay", 1200, 30, false), 2).unwrap()), ["match_damage_500", "demolisher"]);
    // An API poll in between must not reset the replay totals...
    assert!(session.poll(&tracker).await.unwrap().is_empty());
    // ...so the next replay is again exactly one match: a win without taking damage.
    assert_eq!(ids(&session.record_replay(replay("2.replay", 100, 0, true), 3).unwrap()), ["untouchable"]);
    // A file without stats is only marked as read.
    let empty = ReplayFile { file_name: "3.replay".into(), file_size: 10, stats: None };
    assert!(session.record_replay(empty, 4).unwrap().is_empty());
    assert!(storage.replay_seen("fortnite", "3.replay", 10).unwrap());

    // Restarting rebuilds the same totals.
    let (session, _) = Session::start(&storage, &fortnite, vec![&fortnite], &platform, &rules, "acc").unwrap();
    assert_eq!(session.replay_totals["damage"], 1300);
  }

  #[tokio::test]
  async fn solo_win_with_12_kills_unlocks_after_the_match() {
    let (fortnite, platform) = definitions();
    let rules = Progression::default();
    let storage = Storage::open_in_memory().unwrap();
    let (mut session, retro) = Session::start(&storage, &fortnite, vec![&fortnite], &platform, &rules, "acc").unwrap();
    assert!(retro.is_empty());

    let tracker = ReplayTracker::new([Ok(totals(300, 20, 500)), Ok(totals(300, 20, 500)), Ok(totals(301, 21, 512))]);
    assert!(session.poll(&tracker).await.unwrap().is_empty(), "first poll is the baseline");
    assert!(session.poll(&tracker).await.unwrap().is_empty(), "no match played");
    assert_eq!(ids(&session.poll(&tracker).await.unwrap()), ["first_win", "solo_dominant", "match_kills_5", "match_kills_10"]);
    assert_eq!(storage.snapshots("fortnite", "acc").unwrap().len(), 2, "the identical poll isn't stored");
  }

  #[tokio::test]
  async fn a_failed_poll_changes_nothing() {
    let (fortnite, platform) = definitions();
    let rules = Progression::default();
    let storage = Storage::open_in_memory().unwrap();
    let (mut session, _) = Session::start(&storage, &fortnite, vec![&fortnite], &platform, &rules, "acc").unwrap();

    let tracker = ReplayTracker::new([Ok(totals(10, 0, 5)), Err(TrackError::Private), Ok(totals(11, 1, 5))]);
    session.poll(&tracker).await.unwrap();
    assert!(matches!(session.poll(&tracker).await, Err(SessionError::Track(TrackError::Private))));
    assert_eq!(storage.snapshots("fortnite", "acc").unwrap().len(), 1);
    // The next poll still sees exactly one match: a no-kill win.
    assert_eq!(ids(&session.poll(&tracker).await.unwrap()), ["first_win", "pacifist"]);
  }

  #[tokio::test]
  async fn restarting_resumes_without_repeating_unlocks() {
    let (fortnite, platform) = definitions();
    let rules = Progression::default();
    let storage = Storage::open_in_memory().unwrap();
    let tracker = ReplayTracker::new([Ok(totals(10, 0, 5)), Ok(totals(11, 1, 6)), Ok(totals(12, 2, 7))]);

    let (mut session, _) = Session::start(&storage, &fortnite, vec![&fortnite], &platform, &rules, "acc").unwrap();
    session.poll(&tracker).await.unwrap();
    session.poll(&tracker).await.unwrap();
    let before = session.progress().clone();

    let (mut session, retro) = Session::start(&storage, &fortnite, vec![&fortnite], &platform, &rules, "acc").unwrap();
    assert!(retro.is_empty());
    assert_eq!(session.progress(), &before);
    // Second win in a row: the streak survived the restart, nothing repeats.
    assert!(session.poll(&tracker).await.unwrap().is_empty());
    assert_eq!(session.progress().streaks.get("win_streak_3"), Some(&2));
  }
}
