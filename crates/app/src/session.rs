//! A linked account's live session: rebuilds progress from history at
//! startup, then turns each poll into stored unlocks.

use archi_core::{GameDefinition, GameProgress, GameTracker, Progression, StatSnapshot, TrackError};
use archi_storage::{Storage, StorageError, Timestamp, Unlock, unix_now};
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
  platform: &'a GameDefinition,
  rules: &'a Progression,
  account_id: String,
  progress: GameProgress,
}

impl<'a> Session<'a> {
  /// Also returns what history earned but wasn't stored yet (achievements
  /// added since the last run).
  pub fn start(
    storage: &'a Storage,
    game: &'a GameDefinition,
    platform: &'a GameDefinition,
    rules: &'a Progression,
    account_id: &str,
  ) -> Result<(Self, Vec<Unlock>), StorageError> {
    let (progress, mut unlocks) = storage.rebuild_progress(game, account_id)?;
    unlocks.extend(storage.evaluate_platform(platform, &[game], rules, unix_now())?);
    Ok((Self { storage, game, platform, rules, account_id: account_id.to_owned(), progress }, unlocks))
  }

  pub fn progress(&self) -> &GameProgress {
    &self.progress
  }

  /// Unlocks in notification order: the game's (platinum last), then the
  /// platform's meta-achievements.
  pub fn record(&mut self, snapshot: StatSnapshot, at: Timestamp) -> Result<Vec<Unlock>, StorageError> {
    let mut unlocks = self.storage.ingest(self.game, &self.account_id, &mut self.progress, snapshot, at)?;
    if !unlocks.is_empty() {
      unlocks.extend(self.storage.evaluate_platform(self.platform, &[self.game], self.rules, at)?);
    }
    Ok(unlocks)
  }

  pub async fn poll(&mut self, tracker: &impl GameTracker) -> Result<Vec<Unlock>, SessionError> {
    let snapshot = tracker.fetch(&self.account_id).await?;
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

  #[tokio::test]
  async fn solo_win_with_12_kills_unlocks_after_the_match() {
    let (fortnite, platform) = definitions();
    let rules = Progression::default();
    let storage = Storage::open_in_memory().unwrap();
    let (mut session, retro) = Session::start(&storage, &fortnite, &platform, &rules, "acc").unwrap();
    assert!(retro.is_empty());

    let tracker = ReplayTracker::new([Ok(totals(300, 20, 500)), Ok(totals(300, 20, 500)), Ok(totals(301, 21, 512))]);
    assert!(session.poll(&tracker).await.unwrap().is_empty(), "first poll is the baseline");
    assert!(session.poll(&tracker).await.unwrap().is_empty(), "no match played");
    assert_eq!(ids(&session.poll(&tracker).await.unwrap()), ["first_win", "solo_dominant"]);
    assert_eq!(storage.snapshots("fortnite", "acc").unwrap().len(), 2, "the identical poll isn't stored");
  }

  #[tokio::test]
  async fn a_failed_poll_changes_nothing() {
    let (fortnite, platform) = definitions();
    let rules = Progression::default();
    let storage = Storage::open_in_memory().unwrap();
    let (mut session, _) = Session::start(&storage, &fortnite, &platform, &rules, "acc").unwrap();

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

    let (mut session, _) = Session::start(&storage, &fortnite, &platform, &rules, "acc").unwrap();
    session.poll(&tracker).await.unwrap();
    session.poll(&tracker).await.unwrap();
    let before = session.progress().clone();

    let (mut session, retro) = Session::start(&storage, &fortnite, &platform, &rules, "acc").unwrap();
    assert!(retro.is_empty());
    assert_eq!(session.progress(), &before);
    // Second win in a row: the streak survived the restart, nothing repeats.
    assert!(session.poll(&tracker).await.unwrap().is_empty());
    assert_eq!(session.progress().streaks.get("win_streak_3"), Some(&2));
  }
}
