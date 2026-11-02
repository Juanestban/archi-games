//! Fortnite tracker backed by fortnite-api.com, with the player's own API key.
//!
//! Stats only change when a match ends, so unlocks happen after each match.

mod api;
pub mod replay;

use std::time::Duration;

use archi_core::{AccountInfo, GameTracker, StatSnapshot, TrackError};
use reqwest::header::{AUTHORIZATION, RETRY_AFTER};
use reqwest::{Client, Url};

pub const GAME_ID: &str = "fortnite";

/// The game client itself; the launcher alone doesn't mean a match.
pub const PROCESS_NAMES: &[&str] = &["FortniteClient-Win64-Shipping.exe"];

/// Settings key holding the player's fortnite-api.com key.
pub const API_KEY_SETTING: &str = "fortnite.api_key";

/// Where players get an API key.
pub const API_KEY_URL: &str = "https://dash.fortnite-api.com";

const STATS_URL: &str = "https://fortnite-api.com/v2/stats/br/v2";

pub struct FortniteTracker {
  http: Client,
  api_key: String,
}

impl FortniteTracker {
  pub fn new(api_key: impl Into<String>) -> Result<Self, TrackError> {
    let http =
      Client::builder().user_agent(concat!("archi-games/", env!("CARGO_PKG_VERSION"))).timeout(Duration::from_secs(20)).build().map_err(network)?;
    Ok(Self { http, api_key: api_key.into() })
  }

  async fn stats(&self, url: Url) -> Result<(AccountInfo, StatSnapshot), TrackError> {
    let response = self.http.get(url).header(AUTHORIZATION, &self.api_key).send().await.map_err(network)?;
    let status = response.status().as_u16();
    let retry_after = response.headers().get(RETRY_AFTER).and_then(|v| v.to_str().ok()?.parse().ok()).map(Duration::from_secs);
    let body = response.bytes().await.map_err(network)?;
    api::parse_stats(status, retry_after, &body)
  }
}

impl GameTracker for FortniteTracker {
  fn game_id(&self) -> &str {
    GAME_ID
  }

  fn process_names(&self) -> &[&str] {
    PROCESS_NAMES
  }

  async fn find_account(&self, name: &str) -> Result<AccountInfo, TrackError> {
    let url = Url::parse_with_params(STATS_URL, [("name", name)]).map_err(|e| TrackError::Unexpected(e.to_string()))?;
    Ok(self.stats(url).await?.0)
  }

  async fn fetch(&self, account_id: &str) -> Result<StatSnapshot, TrackError> {
    let mut url = Url::parse(STATS_URL).map_err(|e| TrackError::Unexpected(e.to_string()))?;
    // A path segment is percent-encoded, so an id can't change the URL.
    url.path_segments_mut().map_err(|()| TrackError::Unexpected("bad stats URL".into()))?.push(account_id);
    Ok(self.stats(url).await?.1)
  }
}

fn network(error: reqwest::Error) -> TrackError {
  TrackError::Network(error.to_string())
}
