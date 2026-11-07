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
#[serde(rename_all = "camelCase")]
struct StatsData {
  account: Account,
  battle_pass: Option<BattlePass>,
  stats: Inputs,
}

/// This season's pass: it starts over every season.
#[derive(Deserialize)]
struct BattlePass {
  level: Option<u64>,
}

#[derive(Deserialize)]
struct Account {
  id: String,
  name: String,
}

/// Stats split by input device. `all` is the whole account, console matches included; each
/// device's overall stats go to an `input_*` mode of their own.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Inputs {
  all: Option<Modes>,
  keyboard_mouse: Option<Modes>,
  gamepad: Option<Modes>,
  touch: Option<Modes>,
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
#[derive(Default, Deserialize)]
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

/// Mode holding the battle pass level (`level`); see [`parse_stats`].
pub const MODE_BATTLE_PASS: &str = "battlepass";

/// Turns an HTTP response into the account and its snapshot. `overall`
/// becomes mode [`MODE_ALL`]; other modes keep their API names; each input
/// device's overall stats become `input_keyboardmouse`, `input_gamepad` or
/// `input_touch`; the battle pass level is `level` in [`MODE_BATTLE_PASS`].
///
/// Every counter of every mode is recorded, as 0 when the API has nothing (a mode
/// never played): the engine then knows a stat missing from an old snapshot was
/// just not read yet, not zero (see `GameMeta::local_modes`).
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
    // e.g. 500 "failed to fetch the requested stats": fortnite-api.com couldn't reach Epic.
    500..=599 => return Err(TrackError::Unavailable(format!("HTTP {status}: {message}"))),
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
  fn overall_of(input: &Option<Modes>) -> Option<&ModeStats> {
    input.as_ref().and_then(|modes| modes.overall.as_ref())
  }
  let by_input = [
    ("input_keyboardmouse", overall_of(&data.stats.keyboard_mouse)),
    ("input_gamepad", overall_of(&data.stats.gamepad)),
    ("input_touch", overall_of(&data.stats.touch)),
  ];
  let none = ModeStats::default();
  let mut snapshot = StatSnapshot::new();
  for (mode, stats) in by_mode.into_iter().chain(by_input) {
    for (stat, value) in stats.unwrap_or(&none).counters() {
      snapshot.set(mode, stat, value.unwrap_or(0));
    }
  }
  if let Some(level) = data.battle_pass.and_then(|pass| pass.level) {
    snapshot.set(MODE_BATTLE_PASS, "level", level);
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
    assert_eq!(snapshot.get(MODE_BATTLE_PASS, "level"), 87);
    assert_eq!(snapshot.get("input_gamepad", "wins"), 5);
    assert_eq!(snapshot.get("input_gamepad", MATCHES), 40);
    assert_eq!(snapshot.get("input_keyboardmouse", "wins"), 0, "null input: nothing");
  }

  #[test]
  fn null_modes_read_as_zero_and_ratios_are_skipped() {
    let (_, snapshot) = parse_stats(200, None, RESPONSE).unwrap();
    assert!(snapshot.contains("trio", "wins") && snapshot.get("trio", "wins") == 0);
    assert!(snapshot.contains("input_touch", MATCHES));
    assert!(snapshot.contains("solo", "top3"), "every counter, even one the mode doesn't report");
    assert!(!snapshot.contains(MODE_ALL, "kd"));
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
    assert!(matches!(parse_stats(418, None, b"oops"), Err(TrackError::Unexpected(_))));
  }

  #[test]
  fn server_errors_are_an_outage() {
    let outage = br#"{"status":500,"error":"failed to fetch the requested stats (0)"}"#;
    assert!(matches!(parse_stats(500, None, outage), Err(TrackError::Unavailable(m)) if m.contains("failed to fetch")));
    assert!(matches!(parse_stats(503, None, b""), Err(TrackError::Unavailable(_))));
  }

  #[test]
  fn partial_data_is_an_error_not_a_snapshot() {
    let no_overall = br#"{"status":200,"data":{"account":{"id":"a","name":"b"},"stats":{"all":{"overall":null}}}}"#;
    assert!(matches!(parse_stats(200, None, no_overall), Err(TrackError::Unexpected(_))));
    let no_matches = br#"{"status":200,"data":{"account":{"id":"a","name":"b"},"stats":{"all":{"overall":{"wins":1}}}}}"#;
    assert!(matches!(parse_stats(200, None, no_matches), Err(TrackError::Unexpected(_))));
  }
}
