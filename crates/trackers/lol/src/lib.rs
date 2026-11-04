//! League of Legends through Riot's Live Client Data API: while a match runs, the game itself
//! serves its state as JSON on `https://127.0.0.1:2999`, documented by Riot for third-party
//! tools. The app only sends it read-only GET requests while the game's process is up; nothing
//! reads or touches the process itself (ADR 0001). No setup and no API key.

pub mod live;

use std::time::Duration;

pub const GAME_ID: &str = "lol";

/// The match itself; the client (`LeagueClient.exe`) alone doesn't mean a match.
pub const PROCESS_NAMES: &[&str] = &["League of Legends.exe"];

/// Everything the game knows about the match in progress.
pub const ALL_GAME_DATA_URL: &str = "https://127.0.0.1:2999/liveclientdata/allgamedata";

/// The game's HTTPS certificate is signed by this Riot root (published at
/// static.developer.riotgames.com/docs/lol/riotgames.pem, valid until 2043).
const RIOT_ROOT_PEM: &[u8] = include_bytes!("riotgames.pem");

#[derive(Debug)]
pub enum LiveError {
  /// No match (the server only answers during one) or the game is still loading.
  Unavailable,
  /// An answer that isn't the expected JSON.
  Unexpected,
}

/// Reads the match in progress from the game's local server.
pub struct LiveClient {
  http: reqwest::Client,
}

impl LiveClient {
  pub fn new() -> Result<Self, reqwest::Error> {
    let root = reqwest::Certificate::from_pem(RIOT_ROOT_PEM)?;
    let http = reqwest::Client::builder()
      .add_root_certificate(root)
      // The certificate is Riot's, but not issued for "127.0.0.1"; the root still has to match.
      .danger_accept_invalid_hostnames(true)
      .timeout(Duration::from_secs(5))
      .no_proxy()
      .build()?;
    Ok(Self { http })
  }

  pub async fn fetch(&self) -> Result<live::GameData, LiveError> {
    let response = self.http.get(ALL_GAME_DATA_URL).send().await.map_err(|_| LiveError::Unavailable)?;
    if !response.status().is_success() {
      return Err(LiveError::Unavailable);
    }
    let body = response.bytes().await.map_err(|_| LiveError::Unavailable)?;
    live::parse(&body).ok_or(LiveError::Unexpected)
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn the_riot_root_certificate_loads() {
    assert!(super::LiveClient::new().is_ok());
  }

  /// Reads the match in progress (a Practice Tool match will do), certificate check included:
  /// `cargo test -p archi-lol reads_a_live_match -- --ignored --nocapture`.
  #[test]
  #[ignore = "needs a League of Legends match in progress"]
  fn reads_a_live_match() {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let data = runtime.block_on(super::LiveClient::new().unwrap().fetch()).expect("no match answered");
    println!(
      "account {:?}, mode {:?}, {} s, {} events",
      data.account_id(),
      data.game_data.game_mode,
      data.game_data.game_time,
      data.events.events.len()
    );
    assert!(data.account_id().is_some());
  }
}
