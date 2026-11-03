//! Steam Web API responses and their mapping to a [`StatSnapshot`].

use std::time::Duration;

use archi_core::{AccountInfo, MATCHES, MODE_ALL, StatSnapshot, TrackError};
use serde::Deserialize;

/// Per-match stats Steam keeps for the last match only; kept in their own mode.
pub const MODE_LAST_MATCH: &str = "last_match";

/// What the player typed to find their account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Profile {
  /// A SteamID64.
  Id(String),
  /// A custom URL name, resolved with ResolveVanityURL.
  Vanity(String),
}

/// Accepts `steamcommunity.com/profiles/<SteamID64>`, `steamcommunity.com/id/<name>` (with or
/// without the scheme), a bare SteamID64 or a bare custom URL name.
pub fn parse_profile(input: &str) -> Option<Profile> {
  let input = input.trim().trim_end_matches('/');
  let after = |marker: &str| input.find(marker).map(|at| input[at + marker.len()..].split(['/', '?', '#']).next().unwrap_or(""));
  if let Some(id) = after("steamcommunity.com/profiles/") {
    return is_steam_id(id).then(|| Profile::Id(id.to_owned()));
  }
  let name = after("steamcommunity.com/id/").unwrap_or(input);
  if is_steam_id(name) {
    return Some(Profile::Id(name.to_owned()));
  }
  let valid = (2..=32).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
  valid.then(|| Profile::Vanity(name.to_owned()))
}

/// Individual accounts' SteamID64s are 17 digits starting with 7656119.
fn is_steam_id(text: &str) -> bool {
  text.len() == 17 && text.starts_with("7656119") && text.bytes().all(|b| b.is_ascii_digit())
}

#[derive(Deserialize)]
struct VanityEnvelope {
  response: VanityResponse,
}

#[derive(Deserialize)]
struct VanityResponse {
  success: u32,
  steamid: Option<String>,
}

pub(crate) fn parse_vanity(status: u16, retry_after: Option<Duration>, body: &[u8]) -> Result<String, TrackError> {
  check_status(status, retry_after, body)?;
  let envelope: VanityEnvelope = serde_json::from_slice(body).map_err(|e| unexpected(&e.to_string()))?;
  // 1 is a match; 42 means no such custom URL.
  match (envelope.response.success, envelope.response.steamid) {
    (1, Some(id)) if is_steam_id(&id) => Ok(id),
    _ => Err(TrackError::NotFound),
  }
}

#[derive(Deserialize)]
struct SummaryEnvelope {
  response: Players,
}

#[derive(Deserialize)]
struct Players {
  players: Vec<Player>,
}

#[derive(Deserialize)]
struct Player {
  steamid: String,
  personaname: String,
  /// 3 is public; anything else hides the game's stats.
  communityvisibilitystate: u32,
}

pub(crate) fn parse_summary(status: u16, retry_after: Option<Duration>, body: &[u8]) -> Result<AccountInfo, TrackError> {
  check_status(status, retry_after, body)?;
  let envelope: SummaryEnvelope = serde_json::from_slice(body).map_err(|e| unexpected(&e.to_string()))?;
  let player = envelope.response.players.into_iter().next().ok_or(TrackError::NotFound)?;
  if player.communityvisibilitystate != 3 {
    return Err(TrackError::Private);
  }
  Ok(AccountInfo { id: player.steamid, display_name: player.personaname })
}

#[derive(Deserialize)]
struct StatsEnvelope {
  playerstats: Option<PlayerStats>,
}

#[derive(Deserialize)]
struct PlayerStats {
  #[serde(default)]
  stats: Vec<Stat>,
}

#[derive(Deserialize)]
struct Stat {
  name: String,
  value: serde_json::Number,
}

/// `total_*` counters go to mode [`MODE_ALL`] without the prefix (`total_kills_ak47` →
/// `kills_ak47`), with `total_matches_played` as [`MATCHES`]; `last_match_*` go to
/// [`MODE_LAST_MATCH`]. Anything else is left out.
pub(crate) fn parse_stats(status: u16, retry_after: Option<Duration>, body: &[u8]) -> Result<StatSnapshot, TrackError> {
  check_status(status, retry_after, body)?;
  let envelope: StatsEnvelope = serde_json::from_slice(body).map_err(|e| unexpected(&e.to_string()))?;
  // An empty `playerstats` is what a private profile gets.
  let stats = envelope.playerstats.map(|p| p.stats).unwrap_or_default();
  if stats.is_empty() {
    return Err(TrackError::Private);
  }
  let mut snapshot = StatSnapshot::new();
  for stat in stats {
    // Counters only: a negative or fractional value isn't one.
    let Some(value) = stat.value.as_u64() else { continue };
    if stat.name == "total_matches_played" {
      snapshot.set(MODE_ALL, MATCHES, value);
    } else if let Some(name) = stat.name.strip_prefix("total_") {
      snapshot.set(MODE_ALL, name, value);
    } else if let Some(name) = stat.name.strip_prefix("last_match_") {
      snapshot.set(MODE_LAST_MATCH, name, value);
    }
  }
  if snapshot.get(MODE_ALL, "kills") == 0 && snapshot.get(MODE_ALL, MATCHES) == 0 && snapshot.get(MODE_ALL, "time_played") == 0 {
    return Err(unexpected("no CS2 counters"));
  }
  Ok(snapshot)
}

fn check_status(status: u16, retry_after: Option<Duration>, body: &[u8]) -> Result<(), TrackError> {
  let text = String::from_utf8_lossy(body).to_ascii_lowercase();
  match status {
    200 => Ok(()),
    401 => Err(TrackError::Unauthorized),
    // A bad key gets "verify your key= parameter"; otherwise the profile hides its stats.
    403 if text.contains("key=") => Err(TrackError::Unauthorized),
    403 => Err(TrackError::Private),
    400 | 500 if text.contains("not public") || text.contains("private") => Err(TrackError::Private),
    // "Requested app has no stats": never played.
    400 | 404 => Err(TrackError::NotFound),
    429 => Err(TrackError::RateLimited { retry_after }),
    _ => Err(TrackError::Unexpected(format!("HTTP {status}"))),
  }
}

fn unexpected(what: &str) -> TrackError {
  TrackError::Unexpected(what.to_owned())
}

#[cfg(test)]
mod tests {
  use super::*;

  const STATS: &[u8] = include_bytes!("../tests/fixtures/user_stats_730.json");
  const ID: &str = "76561197960287930";

  #[test]
  fn parses_every_way_to_name_a_profile() {
    let id = || Some(Profile::Id(ID.into()));
    assert_eq!(parse_profile(ID), id());
    assert_eq!(parse_profile(&format!("https://steamcommunity.com/profiles/{ID}/")), id());
    assert_eq!(parse_profile(&format!("steamcommunity.com/profiles/{ID}?l=spanish")), id());
    assert_eq!(parse_profile(" https://steamcommunity.com/id/gaben/ "), Some(Profile::Vanity("gaben".into())));
    assert_eq!(parse_profile("gaben"), Some(Profile::Vanity("gaben".into())));
    assert_eq!(parse_profile("https://steamcommunity.com/profiles/123"), None);
    assert_eq!(parse_profile("a b"), None);
    assert_eq!(parse_profile("../x"), None);
  }

  #[test]
  fn maps_totals_weapons_and_the_last_match() {
    let snapshot = parse_stats(200, None, STATS).unwrap();
    assert_eq!(snapshot.get(MODE_ALL, "kills"), 4210);
    assert_eq!(snapshot.get(MODE_ALL, MATCHES), 512);
    assert_eq!(snapshot.get(MODE_ALL, "matches_won"), 260);
    assert_eq!(snapshot.get(MODE_ALL, "kills_headshot"), 1650);
    assert_eq!(snapshot.get(MODE_ALL, "kills_ak47"), 1320);
    assert_eq!(snapshot.get(MODE_ALL, "kills_taser"), 3);
    assert_eq!(snapshot.get(MODE_LAST_MATCH, "kills"), 21);
    let json = serde_json::to_value(&snapshot).unwrap();
    assert!(!json["modes"]["all"].as_object().unwrap().contains_key("GI.lesson.csgo_instr_explain_buymenu"));
  }

  #[test]
  fn reads_the_account_and_its_visibility() {
    let public = format!(r#"{{"response":{{"players":[{{"steamid":"{ID}","personaname":"Archi","communityvisibilitystate":3}}]}}}}"#);
    assert_eq!(parse_summary(200, None, public.as_bytes()).unwrap(), AccountInfo { id: ID.into(), display_name: "Archi".into() });
    let private = public.replace(":3", ":1");
    assert!(matches!(parse_summary(200, None, private.as_bytes()), Err(TrackError::Private)));
    assert!(matches!(parse_summary(200, None, br#"{"response":{"players":[]}}"#), Err(TrackError::NotFound)));
  }

  #[test]
  fn resolves_custom_urls() {
    let found = format!(r#"{{"response":{{"steamid":"{ID}","success":1}}}}"#);
    assert_eq!(parse_vanity(200, None, found.as_bytes()).unwrap(), ID);
    assert!(matches!(parse_vanity(200, None, br#"{"response":{"success":42,"message":"No match"}}"#), Err(TrackError::NotFound)));
  }

  #[test]
  fn maps_error_statuses() {
    let bad_key =
      b"<html><body><h1>Forbidden</h1>Access is denied. Retrying will not help. Please verify your <pre>key=</pre> parameter.</body></html>";
    assert!(matches!(parse_stats(403, None, bad_key), Err(TrackError::Unauthorized)));
    assert!(matches!(parse_stats(401, None, b""), Err(TrackError::Unauthorized)));
    assert!(matches!(parse_stats(403, None, b""), Err(TrackError::Private)));
    assert!(matches!(parse_stats(200, None, br#"{"playerstats":{}}"#), Err(TrackError::Private)));
    assert!(matches!(
      parse_stats(400, None, br#"{"playerstats":{"error":"Requested app has no stats","success":false}}"#),
      Err(TrackError::NotFound)
    ));
    let retry = Some(Duration::from_secs(60));
    assert!(matches!(parse_stats(429, retry, b""), Err(TrackError::RateLimited { retry_after }) if retry_after == retry));
    assert!(matches!(parse_stats(502, None, b""), Err(TrackError::Unexpected(_))));
  }
}
