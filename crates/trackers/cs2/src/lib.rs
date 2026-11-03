//! Counter-Strike 2 tracker backed by the Steam Web API, with the player's own key.
//!
//! Steam keeps CS2's lifetime counters (kills, wins, MVPs, kills by weapon…) as the game's
//! user stats; they change when a match ends, so unlocks happen after each match.

mod api;

use std::time::Duration;

use archi_core::{AccountInfo, GameTracker, StatSnapshot, TrackError};
use reqwest::header::RETRY_AFTER;
use reqwest::{Client, Url};

pub use api::MODE_LAST_MATCH;
use api::{Profile, parse_profile};

pub const GAME_ID: &str = "cs2";

/// The game itself; Steam alone doesn't mean a match.
pub const PROCESS_NAMES: &[&str] = &["cs2.exe"];

/// Credential name holding the player's Steam Web API key.
pub const API_KEY_SETTING: &str = "cs2.api_key";

/// Where players get a key.
pub const API_KEY_URL: &str = "https://steamcommunity.com/dev/apikey";

/// CS2 kept Counter-Strike: Global Offensive's app id.
const APP_ID: &str = "730";

const SUMMARIES_URL: &str = "https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v2/";
const VANITY_URL: &str = "https://api.steampowered.com/ISteamUser/ResolveVanityURL/v1/";
const STATS_URL: &str = "https://api.steampowered.com/ISteamUserStats/GetUserStatsForGame/v2/";

pub struct Cs2Tracker {
  http: Client,
  api_key: String,
}

impl Cs2Tracker {
  pub fn new(api_key: impl Into<String>) -> Result<Self, TrackError> {
    let http =
      Client::builder().user_agent(concat!("archi-games/", env!("CARGO_PKG_VERSION"))).timeout(Duration::from_secs(20)).build().map_err(network)?;
    Ok(Self { http, api_key: api_key.into() })
  }

  /// GET with the key; the parameters are percent-encoded, so no input can change the URL.
  async fn get(&self, url: &str, params: &[(&str, &str)]) -> Result<(u16, Option<Duration>, Vec<u8>), TrackError> {
    let url = Url::parse_with_params(url, params.iter().copied().chain([("key", self.api_key.as_str())]))
      .map_err(|e| TrackError::Unexpected(e.to_string()))?;
    let response = self.http.get(url).send().await.map_err(network)?;
    let status = response.status().as_u16();
    let retry_after = response.headers().get(RETRY_AFTER).and_then(|v| v.to_str().ok()?.parse().ok()).map(Duration::from_secs);
    let body = response.bytes().await.map_err(network)?;
    Ok((status, retry_after, body.to_vec()))
  }

  async fn steam_id(&self, profile: Profile) -> Result<String, TrackError> {
    match profile {
      Profile::Id(id) => Ok(id),
      Profile::Vanity(name) => {
        let (status, retry_after, body) = self.get(VANITY_URL, &[("vanityurl", &name)]).await?;
        api::parse_vanity(status, retry_after, &body)
      }
    }
  }
}

impl GameTracker for Cs2Tracker {
  fn game_id(&self) -> &str {
    GAME_ID
  }

  fn process_names(&self) -> &[&str] {
    PROCESS_NAMES
  }

  /// `name`: a profile URL, a custom URL name or a SteamID64. Also checks that the game's
  /// stats can be read, so a private profile fails here and not on every poll.
  async fn find_account(&self, name: &str) -> Result<AccountInfo, TrackError> {
    let id = self.steam_id(parse_profile(name).ok_or(TrackError::NotFound)?).await?;
    let (status, retry_after, body) = self.get(SUMMARIES_URL, &[("steamids", &id)]).await?;
    let account = api::parse_summary(status, retry_after, &body)?;
    self.fetch(&account.id).await?;
    Ok(account)
  }

  async fn fetch(&self, account_id: &str) -> Result<StatSnapshot, TrackError> {
    let (status, retry_after, body) = self.get(STATS_URL, &[("appid", APP_ID), ("steamid", account_id)]).await?;
    api::parse_stats(status, retry_after, &body)
  }
}

fn network(error: reqwest::Error) -> TrackError {
  TrackError::Network(error.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Against the real Steam Web API, with `STEAM_API_KEY` and `STEAM_PROFILE` (URL, custom
  /// name or SteamID64) set: `cargo test -p archi-cs2 -- --ignored --nocapture`.
  #[tokio::test]
  #[ignore = "needs the network, a Steam Web API key and a public profile"]
  async fn reads_a_real_profile() {
    let key = std::env::var("STEAM_API_KEY").expect("STEAM_API_KEY");
    let profile = std::env::var("STEAM_PROFILE").expect("STEAM_PROFILE");
    let tracker = Cs2Tracker::new(key).unwrap();
    let account = tracker.find_account(&profile).await.unwrap();
    let snapshot = tracker.fetch(&account.id).await.unwrap();
    println!("{} ({})\n{}", account.display_name, account.id, serde_json::to_string_pretty(&snapshot).unwrap());
    assert!(snapshot.get(archi_core::MODE_ALL, "kills") > 0);
  }
}
