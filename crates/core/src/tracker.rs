//! What every game integration implements. A tracker only fetches and
//! normalizes data; the engine decides what gets unlocked.
//!
//! Trackers must follow ADR 0001: never touch the game's process. The game is
//! detected by executable name only and data comes from public APIs.

use std::future::Future;
use std::time::Duration;

use thiserror::Error;

use crate::StatSnapshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountInfo {
  /// Stable id used for every later fetch.
  pub id: String,
  pub display_name: String,
}

#[derive(Debug, Error)]
pub enum TrackError {
  #[error("account not found, or it has never played")]
  NotFound,
  #[error("the account's stats are private")]
  Private,
  #[error("the API key is missing or invalid")]
  Unauthorized,
  #[error("rate limited by the API")]
  RateLimited { retry_after: Option<Duration> },
  #[error("network error: {0}")]
  Network(String),
  /// The API's own servers failed (HTTP 5xx): nothing for the player to fix.
  #[error("the stats service is unavailable: {0}")]
  Unavailable(String),
  #[error("unexpected API response: {0}")]
  Unexpected(String),
}

pub trait GameTracker: Send + Sync {
  /// Matches the `[game] id` of the game's `achievements.toml`.
  fn game_id(&self) -> &str;

  /// Executable names that mean the game is running on this PC.
  fn process_names(&self) -> &[&str];

  /// Resolves the name the player types into an account.
  fn find_account(&self, name: &str) -> impl Future<Output = Result<AccountInfo, TrackError>> + Send;

  /// Current cumulative stats. Must fill [`MODE_ALL`](crate::MODE_ALL) and
  /// [`MATCHES`](crate::MATCHES), and fail rather than return partial data:
  /// a snapshot missing stats would read as a drop and then a jump.
  fn fetch(&self, account_id: &str) -> impl Future<Output = Result<StatSnapshot, TrackError>> + Send;
}
