//! Engine scenarios with simulated snapshot sequences, the way a tracker
//! would feed them after each poll.

use archi_core::{GameDefinition, GameProgress, MATCHES, MODE_ALL, Progress, Progression, StatSnapshot};

const GAME: &str = r#"
[game]
id = "test"
name = "test-name"

[[achievement]]
id = "wins_3"
name = "a"
description = "b"
tier = "bronze"
condition = { type = "total", stat = "wins", target = 3 }

[[achievement]]
id = "solo_dominant"
name = "a"
description = "b"
tier = "silver"
condition = { type = "match", mode = "solo", all = [{ stat = "wins", eq = 1 }, { stat = "kills", gte = 10 }] }

[[achievement]]
id = "streak_2"
name = "a"
description = "b"
tier = "gold"
condition = { type = "streak", all = [{ stat = "wins", eq = 1 }], count = 2 }

[[achievement]]
id = "dlc_kills"
name = "a"
description = "b"
tier = "gold"
pack = "season-2"
condition = { type = "total", stat = "kills", target = 1000 }

[[achievement]]
id = "plat"
name = "a"
description = "b"
tier = "platinum"
"#;

/// Cumulative account stats, advanced one match at a time. Like a real tracker, it reports
/// every stat it knows, 0 included.
#[derive(Clone)]
struct Account {
  snapshot: StatSnapshot,
}

impl Default for Account {
  fn default() -> Self {
    let mut snapshot = StatSnapshot::new();
    for mode in [MODE_ALL, "solo", "duo", "squad"] {
      for stat in [MATCHES, "wins", "kills"] {
        snapshot.set(mode, stat, 0);
      }
    }
    Self { snapshot }
  }
}

impl Account {
  fn play(&mut self, mode: &str, won: bool, kills: u64) -> &mut Self {
    for m in [mode, MODE_ALL] {
      let add = |s: &mut StatSnapshot, stat: &str, n: u64| {
        let value = s.get(m, stat) + n;
        s.set(m, stat, value);
      };
      add(&mut self.snapshot, MATCHES, 1);
      add(&mut self.snapshot, "wins", u64::from(won));
      add(&mut self.snapshot, "kills", kills);
    }
    self
  }

  fn snap(&self) -> StatSnapshot {
    self.snapshot.clone()
  }
}

fn ids(unlocked: Vec<&archi_core::AchievementDef>) -> Vec<&str> {
  unlocked.into_iter().map(|a| a.id.as_str()).collect()
}

fn game() -> GameDefinition {
  GameDefinition::from_toml_str(GAME).unwrap()
}

#[test]
fn first_snapshot_is_the_baseline() {
  let game = game();
  let mut account = Account::default();
  for _ in 0..50 {
    account.play("solo", true, 12);
  }

  let mut progress = GameProgress::default();
  assert!(progress.apply(&game, account.snap()).is_empty(), "history before install never counts");

  account.play("duo", true, 0);
  assert!(progress.apply(&game, account.snap()).is_empty());
  assert_eq!(progress.progress(game.get("wins_3").unwrap()), Some(Progress { current: 1, target: 3 }));
}

#[test]
fn match_condition_needs_exactly_one_match() {
  let game = game();
  let mut account = Account::default();
  let mut progress = GameProgress::default();
  progress.apply(&game, account.snap());

  // Two solo wins with 10 kills total between polls: can't attribute.
  account.play("solo", true, 5).play("solo", true, 5);
  assert!(!ids(progress.apply(&game, account.snap())).contains(&"solo_dominant"));

  // Duo match doesn't count for a solo condition.
  account.play("duo", true, 15);
  assert!(!ids(progress.apply(&game, account.snap())).contains(&"solo_dominant"));

  account.play("solo", true, 11);
  assert!(ids(progress.apply(&game, account.snap())).contains(&"solo_dominant"));
}

#[test]
fn streak_resets_on_loss_and_on_ambiguous_polls() {
  let game = game();
  let mut account = Account::default();
  let mut progress = GameProgress::default();
  progress.apply(&game, account.snap());

  account.play("solo", true, 0);
  progress.apply(&game, account.snap());
  account.play("solo", false, 0);
  progress.apply(&game, account.snap());
  account.play("solo", true, 0);
  progress.apply(&game, account.snap());
  assert!(!progress.is_unlocked("streak_2"), "a loss resets the streak");

  // Two wins in one poll: ambiguous, so the streak resets too.
  account.play("solo", true, 0).play("duo", true, 0);
  progress.apply(&game, account.snap());
  assert!(!progress.is_unlocked("streak_2"));

  // A poll with no new matches keeps the streak.
  account.play("solo", true, 0);
  progress.apply(&game, account.snap());
  progress.apply(&game, account.snap());
  account.play("solo", true, 0);
  assert!(ids(progress.apply(&game, account.snap())).contains(&"streak_2"));
}

#[test]
fn platinum_unlocks_last_and_ignores_packs() {
  let game = game();
  let mut account = Account::default();
  let mut progress = GameProgress::default();
  progress.apply(&game, account.snap());

  account.play("solo", true, 0);
  assert_eq!(ids(progress.apply(&game, account.snap())), Vec::<&str>::new());
  account.play("solo", true, 0);
  assert_eq!(ids(progress.apply(&game, account.snap())), ["streak_2"]);
  account.play("solo", true, 12);
  assert_eq!(ids(progress.apply(&game, account.snap())), ["wins_3", "solo_dominant", "plat"]);
  assert!(!progress.is_unlocked("dlc_kills"));

  // Already unlocked achievements are never reported again.
  account.play("solo", true, 12);
  assert!(progress.apply(&game, account.snap()).is_empty());
}

#[test]
fn each_pack_platinum_needs_only_its_own_pack() {
  let game = GameDefinition::from_toml_str(&format!(
    "{GAME}{}",
    r#"
[[achievement]]
id = "dlc_wins"
name = "a"
description = "b"
tier = "silver"
pack = "season-2"
condition = { type = "total", stat = "wins", target = 2 }

[[achievement]]
id = "season_plat"
name = "a"
description = "b"
tier = "platinum"
pack = "season-2"
"#
  ))
  .unwrap();
  let mut account = Account::default();
  let mut progress = GameProgress::default();
  progress.apply(&game, account.snap());

  account.play("duo", true, 400).play("duo", true, 600);
  assert_eq!(ids(progress.apply(&game, account.snap())), ["dlc_kills", "dlc_wins", "season_plat"], "the base game isn't needed");
  assert!(!progress.is_unlocked("plat"));
}

#[test]
fn replaying_history_rebuilds_the_same_state() {
  let game = game();
  let mut account = Account::default();
  let mut history = vec![account.snap()];
  for (won, kills) in [(true, 3), (false, 1), (true, 12), (true, 0), (true, 4)] {
    account.play("solo", won, kills);
    history.push(account.snap());
  }

  let mut live = GameProgress::default();
  for snapshot in &history {
    live.apply(&game, snapshot.clone());
  }
  let mut replayed = GameProgress::default();
  for snapshot in history {
    replayed.apply(&game, snapshot);
  }
  assert_eq!(live, replayed);
}

#[test]
fn platform_meta_achievements_start_from_zero() {
  let game = GameDefinition::from_toml_str(
    r#"
        [game]
        id = "platform"
        name = "platform-name"
        local_modes = ["all"]

        [[achievement]]
        id = "first_platinum"
        name = "a"
        description = "b"
        tier = "gold"
        condition = { type = "total", stat = "platinums", target = 1 }
        "#,
  )
  .unwrap();

  let mut progress = GameProgress::with_baseline(StatSnapshot::new());
  let unlocked = progress.apply(&game, StatSnapshot::new().with(MODE_ALL, "platinums", 1));
  assert_eq!(ids(unlocked), ["first_platinum"]);
}

/// A newer tracker reads stats the baseline never had: they're lifetime totals, not progress.
/// In a local mode, though, a stat seen for the first time is new progress.
#[test]
fn stats_first_seen_after_the_baseline_count_from_then_except_in_local_modes() {
  let game = GameDefinition::from_toml_str(
    r#"
        [game]
        id = "g"
        name = "g-name"
        local_modes = ["replay"]

        [[achievement]]
        id = "gamepad_win"
        name = "a"
        description = "b"
        tier = "bronze"
        condition = { type = "total", mode = "input_gamepad", stat = "wins", target = 1 }

        [[achievement]]
        id = "first_taser"
        name = "a"
        description = "b"
        tier = "bronze"
        condition = { type = "total", mode = "replay", stat = "kills_taser", target = 1 }
        "#,
  )
  .unwrap();
  let mut progress = GameProgress::default();
  progress.apply(&game, StatSnapshot::new().with(MODE_ALL, "wins", 50));
  // 30 lifetime gamepad wins show up: none of them is new.
  let lifetime = StatSnapshot::new().with(MODE_ALL, "wins", 50).with("input_gamepad", "wins", 30);
  assert!(progress.apply(&game, lifetime.clone()).is_empty());
  assert_eq!(progress.progress(game.get("gamepad_win").unwrap()).unwrap().current, 0);
  assert_eq!(ids(progress.apply(&game, lifetime.clone().with(MODE_ALL, "wins", 51).with("input_gamepad", "wins", 31))), ["gamepad_win"]);
  // The first taser kill from a replay counts.
  let with_replay = lifetime.with(MODE_ALL, "wins", 51).with("input_gamepad", "wins", 31).with("replay", "kills_taser", 1);
  assert_eq!(ids(progress.apply(&game, with_replay)), ["first_taser"]);
}

#[test]
fn reach_reads_the_current_value_even_after_it_starts_over() {
  let game = GameDefinition::from_toml_str(
    r#"
        [game]
        id = "g"
        name = "g-name"

        [[achievement]]
        id = "level_100"
        name = "a"
        description = "b"
        tier = "silver"
        condition = { type = "reach", mode = "battlepass", stat = "level", target = 100 }
        "#,
  )
  .unwrap();
  let level = |n| StatSnapshot::new().with("battlepass", "level", n);
  let achievement = game.get("level_100").unwrap();

  let mut progress = GameProgress::default();
  assert!(progress.apply(&game, level(90)).is_empty(), "the baseline");
  // A new season starts the level over: progress follows the current value, not a delta.
  assert!(progress.apply(&game, level(3)).is_empty());
  assert_eq!(progress.progress(achievement).unwrap().current, 3);
  assert_eq!(ids(progress.apply(&game, level(104))), ["level_100"]);
  assert_eq!(progress.progress(achievement).unwrap().current, 100);
}

#[test]
fn unlocked_tiers_add_up_to_account_level() {
  let game = game();
  let mut account = Account::default();
  let mut progress = GameProgress::default();
  progress.apply(&game, account.snap());
  for kills in [0, 0, 12] {
    account.play("solo", true, kills);
    progress.apply(&game, account.snap());
  }

  let rules = Progression::default();
  let xp: u64 = progress.unlocked.iter().map(|id| rules.xp_for(game.get(id).unwrap().tier)).sum();
  assert_eq!(xp, 15 + 30 + 90 + 300);
  assert_eq!(rules.level_for(xp).level, 3);
}
