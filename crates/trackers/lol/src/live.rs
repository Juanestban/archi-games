//! The Live Client Data API's `allgamedata`, and turning the match it describes into the
//! player's stats once it ends.
//!
//! Every answer holds the whole match so far (scores and every event), so the app can start
//! following a match halfway through and still count all of it. Players are named in events by
//! their Riot ID, its game name or their summoner name, depending on the game version.

use std::collections::BTreeMap;

use serde::Deserialize;

/// Matches shorter than this (in-game seconds) are remakes: they don't count.
pub const MIN_MATCH_SECONDS: f64 = 300.0;

/// Modes that aren't a real match: practice, tutorials and Swarm (against the computer).
const IGNORED_MODES: [&str; 2] = ["PRACTICETOOL", "STRAWBERRY"];

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameData {
  pub active_player: Option<ActivePlayer>,
  #[serde(default)]
  pub all_players: Vec<Player>,
  #[serde(default)]
  pub events: Events,
  #[serde(default)]
  pub game_data: GameStats,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivePlayer {
  #[serde(default)]
  pub riot_id: String,
  #[serde(default)]
  pub riot_id_game_name: String,
  #[serde(default)]
  pub summoner_name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
  #[serde(default)]
  pub riot_id: String,
  #[serde(default)]
  pub riot_id_game_name: String,
  #[serde(default)]
  pub summoner_name: String,
  #[serde(default)]
  pub is_bot: bool,
  #[serde(default)]
  pub level: u64,
  #[serde(default)]
  pub scores: Scores,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scores {
  #[serde(default)]
  pub kills: u64,
  #[serde(default)]
  pub deaths: u64,
  #[serde(default)]
  pub assists: u64,
  #[serde(default)]
  pub creep_score: u64,
  #[serde(default)]
  pub ward_score: f64,
}

#[derive(Debug, Default, Deserialize)]
pub struct Events {
  #[serde(rename = "Events", default)]
  pub events: Vec<Event>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Event {
  #[serde(default)]
  pub event_name: String,
  pub killer_name: Option<String>,
  #[serde(default)]
  pub assisters: Vec<String>,
  #[serde(default)]
  pub kill_streak: u32,
  /// `"True"` or `"False"`, as text.
  pub stolen: Option<String>,
  pub dragon_type: Option<String>,
  pub acer: Option<String>,
  pub recipient: Option<String>,
  /// On `GameEnd`: `"Win"` or `"Lose"`.
  pub result: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStats {
  #[serde(default)]
  pub game_mode: String,
  /// In-game seconds since the match started.
  #[serde(default)]
  pub game_time: f64,
}

pub fn parse(body: &[u8]) -> Option<GameData> {
  serde_json::from_slice(body).ok()
}

impl GameData {
  /// The local player's Riot ID (`name#tag`), which identifies the account.
  pub fn account_id(&self) -> Option<&str> {
    let me = self.active_player.as_ref()?;
    [&me.riot_id, &me.summoner_name].into_iter().find(|name| !name.is_empty()).map(String::as_str)
  }

  fn is_me(&self, name: &str) -> bool {
    self.active_player.as_ref().is_some_and(|me| !name.is_empty() && (name == me.riot_id || name == me.riot_id_game_name || name == me.summoner_name))
  }

  fn me(&self) -> Option<&Player> {
    self.all_players.iter().find(|p| [&p.riot_id, &p.riot_id_game_name, &p.summoner_name].into_iter().any(|name| self.is_me(name)))
  }

  /// The mode its stats are also counted under: `classic` (Summoner's Rift), `aram`, `arena`…
  fn mode(&self) -> String {
    match self.game_data.game_mode.as_str() {
      "CHERRY" => "arena".to_owned(),
      mode => mode.to_ascii_lowercase(),
    }
  }

  fn counts(&self) -> bool {
    let mode = self.game_data.game_mode.as_str();
    !IGNORED_MODES.contains(&mode) && !mode.starts_with("TUTORIAL") && !self.all_players.iter().any(|p| p.is_bot)
  }
}

/// A match played to the end, with what it adds to the player's totals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishedMatch {
  /// Unique per match: the mode and the minute it started.
  pub id: String,
  pub account_id: String,
  pub stats: BTreeMap<String, u64>,
}

/// Follows one match at a time through successive answers.
#[derive(Debug, Default)]
pub struct MatchTracker {
  current: Option<Current>,
}

#[derive(Debug)]
struct Current {
  id: String,
  game_time: f64,
  /// Its end was already reported (the server keeps answering on the end screen).
  done: bool,
}

impl MatchTracker {
  /// Feeds one answer (`now` in Unix seconds); returns the match when this answer ends it.
  pub fn update(&mut self, data: &GameData, now: u64) -> Option<FinishedMatch> {
    if !data.counts() {
      self.current = None;
      return None;
    }
    let game_time = data.game_data.game_time;
    // The clock went back: another match.
    if self.current.as_ref().is_some_and(|current| game_time + 5.0 < current.game_time) {
      self.current = None;
    }
    let current = self.current.get_or_insert_with(|| {
      let started = now.saturating_sub(game_time as u64) / 60 * 60;
      Current { id: format!("{}-{started}", data.mode()), game_time, done: false }
    });
    current.game_time = game_time;
    let result = data.events.events.iter().find(|e| e.event_name == "GameEnd")?.result.as_deref();
    if current.done {
      return None;
    }
    current.done = true;
    if game_time < MIN_MATCH_SECONDS {
      return None;
    }
    let account_id = data.account_id()?.to_owned();
    Some(FinishedMatch { id: current.id.clone(), account_id, stats: stats(data, result == Some("Win")) })
  }

  /// The game closed: a match without its end doesn't count.
  pub fn reset(&mut self) {
    self.current = None;
  }
}

fn stats(data: &GameData, won: bool) -> BTreeMap<String, u64> {
  let me = data.me();
  let scores = me.map(|p| &p.scores);
  let score = |f: fn(&Scores) -> u64| scores.map_or(0, f);
  let events = &data.events.events;
  let named = |name: &'static str| events.iter().filter(move |e| e.event_name == name);
  let killed_by_me = |e: &&Event| e.killer_name.as_deref().is_some_and(|name| data.is_me(name));
  // Objectives count for everyone who helped, not just the last hit.
  let took_part = |e: &&Event| killed_by_me(e) || e.assisters.iter().any(|name| data.is_me(name));
  let streaks = |n: u32| named("Multikill").filter(killed_by_me).filter(|e| e.kill_streak >= n).count() as u64;
  let count = |name: &'static str| named(name).filter(took_part).count() as u64;
  let elder = named("DragonKill").filter(took_part).filter(|e| e.dragon_type.as_deref() == Some("Elder")).count() as u64;
  let steals = events
    .iter()
    .filter(|e| matches!(e.event_name.as_str(), "DragonKill" | "BaronKill" | "HeraldKill"))
    .filter(killed_by_me)
    .filter(|e| e.stolen.as_deref() == Some("True"))
    .count() as u64;
  let first_blood = named("FirstBlood").any(|e| e.recipient.as_deref().is_some_and(|name| data.is_me(name)));
  let aces = named("Ace").filter(|e| e.acer.as_deref().is_some_and(|name| data.is_me(name))).count() as u64;
  let mode = data.mode();
  BTreeMap::from([
    ("matches".to_owned(), 1),
    ("wins".to_owned(), u64::from(won)),
    (format!("matches_{mode}"), 1),
    (format!("wins_{mode}"), u64::from(won)),
    ("kills".to_owned(), score(|s| s.kills)),
    ("deaths".to_owned(), score(|s| s.deaths)),
    ("assists".to_owned(), score(|s| s.assists)),
    ("creep_score".to_owned(), score(|s| s.creep_score)),
    ("ward_score".to_owned(), score(|s| s.ward_score.max(0.0) as u64)),
    ("champion_level".to_owned(), me.map_or(0, |p| p.level)),
    ("double_kills".to_owned(), streaks(2)),
    ("triple_kills".to_owned(), streaks(3)),
    ("quadra_kills".to_owned(), streaks(4)),
    ("penta_kills".to_owned(), streaks(5)),
    ("first_blood".to_owned(), u64::from(first_blood)),
    ("turrets".to_owned(), count("TurretKilled")),
    ("inhibitors".to_owned(), count("InhibKilled")),
    ("dragons".to_owned(), count("DragonKill")),
    ("elder_dragons".to_owned(), elder),
    ("barons".to_owned(), count("BaronKill")),
    ("heralds".to_owned(), count("HeraldKill")),
    ("steals".to_owned(), steals),
    ("aces".to_owned(), aces),
  ])
}

#[cfg(test)]
mod tests {
  use serde_json::{Value, json};

  use super::*;

  /// Riot's sample events (docs: liveclientdata_events.json), with the player as "Riot Tuxedo".
  fn sample_events(end: Option<&str>) -> Value {
    let mut events = json!([
      { "EventID": 0, "EventName": "GameStart", "EventTime": 0.0 },
      { "EventID": 1, "EventName": "FirstBlood", "EventTime": 90.0, "Recipient": "Riot Tuxedo" },
      { "EventID": 2, "EventName": "ChampionKill", "EventTime": 90.0, "VictimName": "Riot Gene", "KillerName": "Riot Tuxedo", "Assisters": [] },
      { "EventID": 3, "EventName": "TurretKilled", "EventTime": 600.0, "TurretKilled": "Turret_T2_L_03_A", "KillerName": "Minion_T1L0S10N0222", "Assisters": ["Riot Tuxedo"] },
      { "EventID": 4, "EventName": "InhibKilled", "EventTime": 900.0, "InhibKilled": "Barracks_T2_R1", "KillerName": "Player 1", "Assisters": [] },
      { "EventID": 5, "EventName": "DragonKill", "EventTime": 700.0, "DragonType": "Earth", "Stolen": "True", "KillerName": "Riot Tuxedo", "Assisters": [] },
      { "EventID": 6, "EventName": "DragonKill", "EventTime": 1500.0, "DragonType": "Elder", "Stolen": "False", "KillerName": "Player 1", "Assisters": ["Riot Tuxedo"] },
      { "EventID": 7, "EventName": "HeraldKill", "EventTime": 800.0, "Stolen": "False", "KillerName": "Player 1", "Assisters": [] },
      { "EventID": 8, "EventName": "BaronKill", "EventTime": 1400.0, "Stolen": "False", "KillerName": "Riot Tuxedo", "Assisters": ["Player 1"] },
      { "EventID": 9, "EventName": "Multikill", "EventTime": 1000.0, "KillerName": "Riot Tuxedo", "KillStreak": 2 },
      { "EventID": 10, "EventName": "Multikill", "EventTime": 1001.0, "KillerName": "Riot Tuxedo", "KillStreak": 3 },
      { "EventID": 11, "EventName": "Multikill", "EventTime": 1100.0, "KillerName": "Player 1", "KillStreak": 2 },
      { "EventID": 12, "EventName": "Ace", "EventTime": 1001.0, "Acer": "Riot Tuxedo", "AcingTeam": "ORDER" }
    ]);
    if let Some(result) = end {
      events.as_array_mut().unwrap().push(json!({ "EventID": 13, "EventName": "GameEnd", "EventTime": 1800.0, "Result": result }));
    }
    events
  }

  fn data(mode: &str, game_time: f64, end: Option<&str>) -> GameData {
    serde_json::from_value(json!({
      "activePlayer": { "riotId": "Riot Tuxedo#EUW", "riotIdGameName": "Riot Tuxedo", "summonerName": "Riot Tuxedo#EUW", "level": 18 },
      "allPlayers": [
        { "riotId": "Riot Tuxedo#EUW", "riotIdGameName": "Riot Tuxedo", "summonerName": "Riot Tuxedo#EUW", "isBot": false, "level": 18, "team": "ORDER",
          "scores": { "kills": 9, "deaths": 0, "assists": 14, "creepScore": 212, "wardScore": 31.6 } },
        { "riotId": "Player 1#1", "riotIdGameName": "Player 1", "summonerName": "Player 1#1", "isBot": false, "level": 16, "team": "ORDER",
          "scores": { "kills": 3, "deaths": 5, "assists": 2, "creepScore": 180, "wardScore": 10.0 } }
      ],
      "events": { "Events": sample_events(end) },
      "gameData": { "gameMode": mode, "gameTime": game_time, "mapNumber": 11 }
    }))
    .unwrap()
  }

  #[test]
  fn a_won_match_adds_its_scores_multikills_and_objectives() {
    let mut tracker = MatchTracker::default();
    assert_eq!(tracker.update(&data("CLASSIC", 1200.0, None), 10_000), None);
    let done = tracker.update(&data("CLASSIC", 1800.0, Some("Win")), 10_600).unwrap();

    assert_eq!(done.account_id, "Riot Tuxedo#EUW");
    // Started at 10 000 - 1200 = 8800 s, kept to the minute.
    assert_eq!(done.id, "classic-8760");
    let stat = |name: &str| done.stats[name];
    assert_eq!((stat("matches"), stat("wins"), stat("matches_classic"), stat("wins_classic")), (1, 1, 1, 1));
    assert_eq!((stat("kills"), stat("deaths"), stat("assists")), (9, 0, 14));
    assert_eq!((stat("creep_score"), stat("ward_score"), stat("champion_level")), (212, 31, 18));
    assert_eq!((stat("double_kills"), stat("triple_kills"), stat("quadra_kills")), (2, 1, 0), "a triple is also a double");
    assert_eq!((stat("first_blood"), stat("aces")), (1, 1));
    // As killer or assister; the inhibitor and the herald were someone else's alone.
    assert_eq!((stat("turrets"), stat("inhibitors"), stat("heralds")), (1, 0, 0));
    assert_eq!((stat("dragons"), stat("elder_dragons"), stat("barons"), stat("steals")), (2, 1, 1, 1));
    // The end screen keeps answering: the match is reported once.
    assert_eq!(tracker.update(&data("CLASSIC", 1800.0, Some("Win")), 10_605), None);
  }

  #[test]
  fn arena_losses_remakes_bots_and_practice_count_as_they_should() {
    let mut tracker = MatchTracker::default();
    let lost = tracker.update(&data("CHERRY", 1200.0, Some("Lose")), 5_000).unwrap();
    assert_eq!((lost.stats["wins"], lost.stats["matches_arena"]), (0, 1));

    // A new match (the clock started over), ended by a remake.
    assert_eq!(tracker.update(&data("CLASSIC", 200.0, Some("Lose")), 6_000), None);

    let mut with_bots = data("CLASSIC", 1800.0, Some("Win"));
    with_bots.all_players[1].is_bot = true;
    assert_eq!(MatchTracker::default().update(&with_bots, 7_000), None);
    assert_eq!(MatchTracker::default().update(&data("PRACTICETOOL", 1800.0, Some("Win")), 7_000), None);
  }

  #[test]
  fn a_match_left_before_its_end_counts_nothing() {
    let mut tracker = MatchTracker::default();
    assert_eq!(tracker.update(&data("ARAM", 900.0, None), 1_000), None);
    tracker.reset();
    // Back in a later match, only its own end counts.
    assert!(tracker.update(&data("ARAM", 1000.0, Some("Win")), 9_000).is_some());
  }

  #[test]
  fn tolerates_missing_fields() {
    let data = parse(br#"{"gameData":{"gameMode":"CLASSIC","gameTime":12.5}}"#).unwrap();
    assert_eq!(data.account_id(), None);
    assert!(parse(b"<html>").is_none());
  }
}
