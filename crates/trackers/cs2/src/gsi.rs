//! CS2's Game State Integration payloads, and turning a stream of them into finished matches.
//!
//! The game sends the whole state on every change. Only the local player counts: when they're
//! dead and spectating, `player` is whoever they watch, so those updates are skipped.

use std::collections::BTreeMap;

use serde::Deserialize;

/// Modes whose matches count; deathmatch, arms race and custom games would inflate kills.
const COUNTED_MODES: [&str; 4] = ["competitive", "premier", "casual", "scrimcomp2v2"];

#[derive(Debug, Default, Deserialize)]
pub struct Payload {
  pub provider: Option<Provider>,
  pub map: Option<Map>,
  pub player: Option<Player>,
  pub auth: Option<Auth>,
}

#[derive(Debug, Deserialize)]
pub struct Provider {
  /// The local player, whatever `player` shows.
  pub steamid: String,
  #[serde(default)]
  pub timestamp: u64,
}

#[derive(Debug, Deserialize)]
pub struct Map {
  #[serde(default)]
  pub mode: String,
  #[serde(default)]
  pub name: String,
  /// warmup, live, intermission or gameover.
  #[serde(default)]
  pub phase: String,
  #[serde(default)]
  pub round: u32,
  pub team_ct: Option<Team>,
  pub team_t: Option<Team>,
}

#[derive(Debug, Deserialize)]
pub struct Team {
  #[serde(default)]
  pub score: u32,
}

#[derive(Debug, Deserialize)]
pub struct Player {
  #[serde(default)]
  pub steamid: String,
  #[serde(default)]
  pub name: String,
  /// CT or T.
  pub team: Option<String>,
  pub state: Option<PlayerState>,
  pub match_stats: Option<MatchStats>,
  #[serde(default)]
  pub weapons: BTreeMap<String, Weapon>,
}

#[derive(Debug, Deserialize)]
pub struct PlayerState {
  #[serde(default)]
  pub round_kills: u32,
  #[serde(default)]
  pub round_killhs: u32,
}

/// Team kills count as -1, so these can go down.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct MatchStats {
  #[serde(default)]
  pub kills: i64,
  #[serde(default)]
  pub assists: i64,
  #[serde(default)]
  pub deaths: i64,
  #[serde(default)]
  pub mvps: i64,
  #[serde(default)]
  pub score: i64,
}

#[derive(Debug, Deserialize)]
pub struct Weapon {
  #[serde(default)]
  pub name: String,
  /// active, holstered or reloading.
  #[serde(default)]
  pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct Auth {
  #[serde(default)]
  pub token: String,
}

pub fn parse(body: &[u8]) -> Option<Payload> {
  serde_json::from_slice(body).ok()
}

impl Payload {
  pub fn token(&self) -> Option<&str> {
    self.auth.as_ref().map(|auth| auth.token.as_str())
  }

  /// The local player, when this update is about them (not someone they spectate).
  pub fn local_player(&self) -> Option<&Player> {
    let provider = self.provider.as_ref()?;
    self.player.as_ref().filter(|player| player.steamid == provider.steamid)
  }
}

/// A match played to the end, with what it adds to the player's totals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishedMatch {
  /// Unique per match: the map and when its first live update came.
  pub id: String,
  pub stats: BTreeMap<String, u64>,
}

/// Follows one match at a time through its updates.
#[derive(Debug, Default)]
pub struct MatchTracker {
  current: Option<Current>,
}

#[derive(Debug)]
struct Current {
  id: String,
  map: String,
  stats: MatchStats,
  team: Option<String>,
  kills_by_weapon: BTreeMap<String, u64>,
  headshots: u64,
  aces: u64,
  round: u32,
  round_kills: u32,
  round_headshots: u32,
}

impl MatchTracker {
  /// Feeds one update; returns the match when this update ends it.
  pub fn update(&mut self, payload: &Payload) -> Option<FinishedMatch> {
    let map = payload.map.as_ref()?;
    if !COUNTED_MODES.contains(&map.mode.as_str()) || map.phase == "warmup" {
      self.current = None;
      return None;
    }
    if self.current.as_ref().is_some_and(|current| current.map != map.name) {
      self.current = None;
    }
    if self.current.is_none() {
      // A match is only followed from its start; joining a finished one adds nothing.
      if map.phase != "live" {
        return None;
      }
      let timestamp = payload.provider.as_ref().map_or(0, |provider| provider.timestamp);
      self.current = Some(Current::new(format!("{}-{timestamp}", map.name), &map.name));
    }
    let current = self.current.as_mut()?;
    if let Some(player) = payload.local_player() {
      current.observe(player, map.round);
    }
    if map.phase != "gameover" {
      return None;
    }
    let current = self.current.take()?;
    Some(current.finish(map))
  }
}

impl Current {
  fn new(id: String, map: &str) -> Self {
    Self {
      id,
      map: map.to_owned(),
      stats: MatchStats::default(),
      team: None,
      kills_by_weapon: BTreeMap::new(),
      headshots: 0,
      aces: 0,
      round: 0,
      round_kills: 0,
      round_headshots: 0,
    }
  }

  fn observe(&mut self, player: &Player, round: u32) {
    if let Some(stats) = player.match_stats {
      // New kills go to the weapon in hand when they're reported.
      let new_kills = stats.kills - self.stats.kills;
      if new_kills > 0
        && let Some(weapon) = player.weapons.values().find(|weapon| weapon.state == "active")
      {
        *self.kills_by_weapon.entry(weapon_key(&weapon.name)).or_insert(0) += new_kills as u64;
      }
      self.stats = stats;
    }
    if player.team.is_some() {
      self.team.clone_from(&player.team);
    }
    if let Some(state) = &player.state {
      // Per-round counters: they reset with each round.
      if round != self.round || state.round_kills < self.round_kills || state.round_killhs < self.round_headshots {
        self.round = round;
        self.round_kills = 0;
        self.round_headshots = 0;
      }
      self.headshots += u64::from(state.round_killhs - self.round_headshots);
      if state.round_kills >= 5 && self.round_kills < 5 {
        self.aces += 1;
      }
      self.round_kills = state.round_kills;
      self.round_headshots = state.round_killhs;
    }
  }

  fn finish(self, map: &Map) -> FinishedMatch {
    let score = |team: &Option<Team>| team.as_ref().map_or(0, |team| team.score);
    let (ct, t) = (score(&map.team_ct), score(&map.team_t));
    let won = match self.team.as_deref() {
      Some("CT") => ct > t,
      Some("T") => t > ct,
      _ => false,
    };
    let count = |value: i64| value.max(0) as u64;
    let mut stats = BTreeMap::from([
      ("matches".to_owned(), 1),
      ("matches_won".to_owned(), u64::from(won)),
      ("kills".to_owned(), count(self.stats.kills)),
      ("deaths".to_owned(), count(self.stats.deaths)),
      ("assists".to_owned(), count(self.stats.assists)),
      ("mvps".to_owned(), count(self.stats.mvps)),
      ("score".to_owned(), count(self.stats.score)),
      ("kills_headshot".to_owned(), self.headshots),
      ("aces".to_owned(), self.aces),
    ]);
    for (weapon, kills) in self.kills_by_weapon {
      stats.insert(format!("kills_{weapon}"), kills);
    }
    FinishedMatch { id: self.id, stats }
  }
}

/// `weapon_ak47` → `ak47`; every knife is `knife`; both M4s are `m4`.
fn weapon_key(name: &str) -> String {
  let name = name.strip_prefix("weapon_").unwrap_or(name);
  match name {
    _ if name.contains("knife") || name == "bayonet" => "knife".to_owned(),
    "m4a1" | "m4a1_silencer" => "m4".to_owned(),
    _ => name.to_owned(),
  }
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  const ME: &str = "76561198421401415";

  /// One update: the local player (or someone they watch) holding `weapon`.
  fn update(phase: &str, round: u32, steamid: &str, weapon: &str, kills: i64, round_kills: u32, round_hs: u32) -> Payload {
    serde_json::from_value(json!({
      "provider": { "steamid": ME, "timestamp": 1_000 },
      "map": { "mode": "competitive", "name": "de_dust2", "phase": phase, "round": round,
               "team_ct": { "score": 13 }, "team_t": { "score": 9 } },
      "player": { "steamid": steamid, "name": "juanes", "team": "CT",
                  "state": { "round_kills": round_kills, "round_killhs": round_hs },
                  "match_stats": { "kills": kills, "assists": 2, "deaths": 7, "mvps": 3, "score": 40 },
                  "weapons": { "weapon_0": { "name": "weapon_knife_karambit", "state": "holstered" },
                               "weapon_1": { "name": weapon, "state": "active" } } },
      "auth": { "token": "secret" }
    }))
    .unwrap()
  }

  #[test]
  fn a_full_match_adds_its_kills_weapons_headshots_aces_and_win() {
    let mut tracker = MatchTracker::default();
    assert_eq!(tracker.update(&update("live", 0, ME, "weapon_ak47", 0, 0, 0)), None);
    assert_eq!(tracker.update(&update("live", 0, ME, "weapon_ak47", 2, 2, 1)), None);
    // Dead and watching a teammate: their numbers aren't ours.
    assert_eq!(tracker.update(&update("live", 0, "76561190000000000", "weapon_awp", 30, 5, 5)), None);
    // Round 1: an ace, 3 of them with the AWP and one headshot.
    assert_eq!(tracker.update(&update("live", 1, ME, "weapon_m4a1_silencer", 4, 2, 0)), None);
    assert_eq!(tracker.update(&update("live", 1, ME, "weapon_awp", 7, 5, 1)), None);
    let done = tracker.update(&update("gameover", 1, ME, "weapon_awp", 7, 5, 1)).unwrap();

    assert_eq!(done.id, "de_dust2-1000");
    let stat = |name: &str| done.stats.get(name).copied().unwrap_or(0);
    assert_eq!((stat("matches"), stat("matches_won")), (1, 1));
    assert_eq!((stat("kills"), stat("deaths"), stat("assists"), stat("mvps")), (7, 7, 2, 3));
    assert_eq!((stat("kills_ak47"), stat("kills_m4"), stat("kills_awp")), (2, 2, 3));
    assert_eq!(stat("kills_headshot"), 2);
    assert_eq!(stat("aces"), 1);
    // Further game-over updates don't count the match again.
    assert_eq!(tracker.update(&update("gameover", 1, ME, "weapon_awp", 7, 5, 1)), None);
  }

  #[test]
  fn warmup_other_modes_and_late_joins_count_nothing() {
    let mut tracker = MatchTracker::default();
    assert_eq!(tracker.update(&update("warmup", 0, ME, "weapon_ak47", 3, 3, 0)), None);
    // Joined during the end screen: not followed from the start.
    assert_eq!(tracker.update(&update("gameover", 0, ME, "weapon_ak47", 3, 3, 0)), None);

    let mut deathmatch = update("live", 0, ME, "weapon_ak47", 0, 0, 0);
    deathmatch.map.as_mut().unwrap().mode = "deathmatch".into();
    assert_eq!(tracker.update(&deathmatch), None);
    deathmatch.map.as_mut().unwrap().phase = "gameover".into();
    assert_eq!(tracker.update(&deathmatch), None);
  }

  #[test]
  fn a_loss_is_not_a_win_and_knives_are_one_weapon() {
    let mut tracker = MatchTracker::default();
    let mut on_t = |phase: &str, kills: i64| {
      let mut payload = update(phase, 0, ME, "weapon_bayonet", kills, kills as u32, 0);
      payload.player.as_mut().unwrap().team = Some("T".into());
      tracker.update(&payload)
    };
    on_t("live", 0);
    on_t("live", 1);
    let done = on_t("gameover", 1).unwrap();
    assert_eq!(done.stats["matches_won"], 0);
    assert_eq!(done.stats["kills_knife"], 1);
  }

  #[test]
  fn reads_the_token_and_ignores_garbage() {
    assert_eq!(update("live", 0, ME, "weapon_ak47", 0, 0, 0).token(), Some("secret"));
    assert!(parse(b"not json").is_none());
    assert!(parse(br#"{"provider":{"steamid":"1"}}"#).is_some());
  }
}
