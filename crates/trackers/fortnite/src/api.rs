//! fortnite-api.com BR stats (v2) responses and their mapping to a
//! [`StatSnapshot`].

use std::time::Duration;

use archi_core::{AccountInfo, MATCHES, MODE_ALL, StatSnapshot, TrackError};
use serde::Deserialize;

#[derive(Deserialize)]
struct Envelope {
  data: Option<StatsData>,
  error: Option<String>,
}

#[derive(Deserialize)]
struct StatsData {
  account: Account,
  stats: Inputs,
}

#[derive(Deserialize)]
struct Account {
  id: String,
  name: String,
}

/// Stats split by input device. Only `all` is used: it's the whole account,
/// console matches included.
#[derive(Deserialize)]
struct Inputs {
  all: Option<Modes>,
}

/// A mode is `null` until the account plays it. The API never fills `trio`.
#[derive(Deserialize)]
struct Modes {
  overall: Option<ModeStats>,
  solo: Option<ModeStats>,
  duo: Option<ModeStats>,
  trio: Option<ModeStats>,
  squad: Option<ModeStats>,
  ltm: Option<ModeStats>,
}

/// Counters only; ratios like `kd` or `winRate` are left out. Each mode
/// reports a different set of `top*` placements.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModeStats {
  score: Option<u64>,
  wins: Option<u64>,
  top3: Option<u64>,
  top5: Option<u64>,
  top6: Option<u64>,
  top10: Option<u64>,
  top12: Option<u64>,
  top25: Option<u64>,
  kills: Option<u64>,
  deaths: Option<u64>,
  matches: Option<u64>,
  minutes_played: Option<u64>,
  players_outlived: Option<u64>,
}

impl ModeStats {
  fn counters(&self) -> [(&'static str, Option<u64>); 13] {
    [
      ("score", self.score),
      ("wins", self.wins),
      ("top3", self.top3),
      ("top5", self.top5),
      ("top6", self.top6),
      ("top10", self.top10),
      ("top12", self.top12),
      ("top25", self.top25),
      ("kills", self.kills),
      ("deaths", self.deaths),
      (MATCHES, self.matches),
      ("minutes_played", self.minutes_played),
      ("players_outlived", self.players_outlived),
    ]
  }
}

/// Turns an HTTP response into the account and its snapshot. `overall`
/// becomes mode [`MODE_ALL`]; other modes keep their API names.
pub(crate) fn parse_stats(status: u16, retry_after: Option<Duration>, body: &[u8]) -> Result<(AccountInfo, StatSnapshot), TrackError> {
  let envelope = serde_json::from_slice::<Envelope>(body);
  let message = envelope.as_ref().ok().and_then(|e| e.error.clone()).unwrap_or_default();
  match status {
    200 => {}
    401 => return Err(TrackError::Unauthorized),
    403 if message.contains("public") => return Err(TrackError::Private),
    403 => return Err(TrackError::Unauthorized),
    404 => return Err(TrackError::NotFound),
    429 => return Err(TrackError::RateLimited { retry_after }),
    _ => return Err(TrackError::Unexpected(format!("HTTP {status}: {message}"))),
  }

  let data = envelope.map_err(|e| unexpected(&e.to_string()))?.data.ok_or_else(|| unexpected("no data"))?;
  let modes = data.stats.all.ok_or_else(|| unexpected("no stats"))?;
  let overall = modes.overall.as_ref().ok_or_else(|| unexpected("no overall stats"))?;
  if overall.matches.is_none() {
    return Err(unexpected("no match count"));
  }

  let by_mode = [
    (MODE_ALL, Some(overall)),
    ("solo", modes.solo.as_ref()),
    ("duo", modes.duo.as_ref()),
    ("trio", modes.trio.as_ref()),
    ("squad", modes.squad.as_ref()),
    ("ltm", modes.ltm.as_ref()),
  ];
  let mut snapshot = StatSnapshot::new();
  for (mode, stats) in by_mode {
    for (stat, value) in stats.into_iter().flat_map(ModeStats::counters) {
      if let Some(value) = value {
        snapshot.set(mode, stat, value);
      }
    }
  }

  Ok((AccountInfo { id: data.account.id, display_name: data.account.name }, snapshot))
}

fn unexpected(what: &str) -> TrackError {
  TrackError::Unexpected(what.to_owned())
}

#[cfg(test)]
mod tests {
  use super::*;

  const RESPONSE: &[u8] = include_bytes!("../tests/fixtures/stats_br_v2.json");

  #[test]
  fn maps_overall_to_all_and_keeps_modes() {
    let (account, snapshot) = parse_stats(200, None, RESPONSE).unwrap();
    assert_eq!(account, AccountInfo { id: "4735ce9132924caf8a5b17789b40f79c".into(), display_name: "ArchiTester".into() });
    assert_eq!(snapshot.get(MODE_ALL, "wins"), 412);
    assert_eq!(snapshot.get(MODE_ALL, MATCHES), 3120);
    assert_eq!(snapshot.get(MODE_ALL, "top10"), 980);
    assert_eq!(snapshot.get(MODE_ALL, "minutes_played"), 40210);
    assert_eq!(snapshot.get("solo", "kills"), 2104);
    assert_eq!(snapshot.get("squad", "top6"), 260);
    assert_eq!(snapshot.get("ltm", "wins"), 30);
  }

  #[test]
  fn null_modes_and_ratios_are_skipped() {
    let (_, snapshot) = parse_stats(200, None, RESPONSE).unwrap();
    let json = serde_json::to_value(&snapshot).unwrap();
    let modes = json["modes"].as_object().unwrap();
    assert!(!modes.contains_key("trio"));
    assert!(!modes["all"].as_object().unwrap().contains_key("kd"));
  }

  #[test]
  fn maps_error_statuses() {
    let private = br#"{"status":403,"error":"the requested account's stats are not public"}"#;
    assert!(matches!(parse_stats(403, None, private), Err(TrackError::Private)));
    assert!(matches!(parse_stats(403, None, br#"{"status":403,"error":"forbidden"}"#), Err(TrackError::Unauthorized)));
    assert!(matches!(parse_stats(401, None, b""), Err(TrackError::Unauthorized)));
    assert!(matches!(parse_stats(404, None, br#"{"status":404,"error":"the requested account does not exist"}"#), Err(TrackError::NotFound)));
    let retry = Some(Duration::from_secs(30));
    assert!(matches!(parse_stats(429, retry, b""), Err(TrackError::RateLimited { retry_after }) if retry_after == retry));
    assert!(matches!(parse_stats(500, None, b"oops"), Err(TrackError::Unexpected(_))));
  }

  #[test]
  fn partial_data_is_an_error_not_a_snapshot() {
    let no_overall = br#"{"status":200,"data":{"account":{"id":"a","name":"b"},"stats":{"all":{"overall":null}}}}"#;
    assert!(matches!(parse_stats(200, None, no_overall), Err(TrackError::Unexpected(_))));
    let no_matches = br#"{"status":200,"data":{"account":{"id":"a","name":"b"},"stats":{"all":{"overall":{"wins":1}}}}}"#;
    assert!(matches!(parse_stats(200, None, no_matches), Err(TrackError::Unexpected(_))));
  }
}
