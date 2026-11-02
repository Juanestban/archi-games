use std::path::PathBuf;

use archi_core::{GameDefinition, GameProgress, MATCHES, MODE_ALL, Progression, StatSnapshot, Tier};
use archi_storage::{LinkedAccount, PLATFORM_ID, Storage, StorageError};

const ACCOUNT: &str = "epic-123";

fn game(extra: &str) -> GameDefinition {
  game_with_target(2, extra)
}

/// `wins_2` keeps its id whatever `target` is, to simulate editing a condition.
fn game_with_target(target: u64, extra: &str) -> GameDefinition {
  GameDefinition::from_toml_str(&format!(
    r#"
        [game]
        id = "test"
        name = "test-name"

        [[achievement]]
        id = "wins_2"
        name = "a"
        description = "b"
        tier = "gold"
        condition = {{ type = "total", stat = "wins", target = {target} }}
        {extra}
        "#
  ))
  .unwrap()
}

fn wins(n: u64) -> StatSnapshot {
  StatSnapshot::new().with(MODE_ALL, MATCHES, n).with(MODE_ALL, "wins", n)
}

fn ids(unlocks: &[archi_storage::Unlock]) -> Vec<&str> {
  unlocks.iter().map(|u| u.achievement_id.as_str()).collect()
}

fn temp_db(name: &str) -> PathBuf {
  let dir = std::env::temp_dir().join(format!("archi-storage-test-{name}-{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&dir);
  dir.join("nested").join("test.db")
}

#[test]
fn reopening_keeps_data_and_rejects_newer_schemas() {
  let path = temp_db("reopen");
  {
    let storage = Storage::open(&path).unwrap();
    storage.set_setting("lang", "es").unwrap();
  }
  let storage = Storage::open(&path).unwrap();
  assert_eq!(storage.setting("lang").unwrap().as_deref(), Some("es"));
  drop(storage);

  let conn = rusqlite::Connection::open(&path).unwrap();
  conn.pragma_update(None, "user_version", 99).unwrap();
  drop(conn);
  assert!(matches!(Storage::open(&path), Err(StorageError::NewerSchema { found: 99, .. })));
}

#[test]
fn settings_and_accounts() {
  let storage = Storage::open_in_memory().unwrap();
  assert_eq!(storage.setting("volume").unwrap(), None);
  storage.set_setting("volume", "0.5").unwrap();
  storage.set_setting("volume", "0.8").unwrap();
  assert_eq!(storage.setting("volume").unwrap().as_deref(), Some("0.8"));

  let mut account = LinkedAccount { game_id: "fortnite".into(), account_id: ACCOUNT.into(), display_name: "Juan".into(), linked_at: 10 };
  storage.link_account(&account).unwrap();
  account.display_name = "JuanNuevo".into();
  storage.link_account(&account).unwrap();
  assert_eq!(storage.account("fortnite").unwrap(), Some(account));
  storage.unlink_account("fortnite").unwrap();
  assert_eq!(storage.account("fortnite").unwrap(), None);
}

#[test]
fn identical_snapshots_are_not_stored_and_accounts_are_isolated() {
  let storage = Storage::open_in_memory().unwrap();
  assert!(storage.record_snapshot("test", ACCOUNT, &wins(1), 1).unwrap());
  assert!(!storage.record_snapshot("test", ACCOUNT, &wins(1), 2).unwrap());
  assert!(storage.record_snapshot("test", "other", &wins(1), 3).unwrap());
  assert_eq!(storage.snapshots("test", ACCOUNT).unwrap(), vec![(1, wins(1))]);
  assert_eq!(storage.last_snapshot("test", "nobody").unwrap(), None);
}

#[test]
fn unlock_keeps_its_first_date() {
  let storage = Storage::open_in_memory().unwrap();
  assert!(storage.insert_unlock("test", "wins_2", 100).unwrap());
  assert!(!storage.insert_unlock("test", "wins_2", 200).unwrap());
  assert_eq!(storage.unlocks().unwrap()[0].unlocked_at, 100);
}

#[test]
fn ingest_then_rebuild_gives_the_same_progress() {
  let storage = Storage::open_in_memory().unwrap();
  let definition = game("");
  let mut live = GameProgress::default();

  assert!(storage.ingest(&definition, ACCOUNT, &mut live, wins(10), 1).unwrap().is_empty());
  assert!(storage.ingest(&definition, ACCOUNT, &mut live, wins(11), 2).unwrap().is_empty());
  assert!(storage.ingest(&definition, ACCOUNT, &mut live, wins(11), 3).unwrap().is_empty());
  let unlocked = storage.ingest(&definition, ACCOUNT, &mut live, wins(12), 4).unwrap();
  assert_eq!(ids(&unlocked), ["wins_2"]);
  assert_eq!(unlocked[0].tier, Tier::Gold);
  assert_eq!(unlocked[0].unlocked_at, 4);

  let (rebuilt, retroactive) = storage.rebuild_progress(&definition, ACCOUNT).unwrap();
  assert!(retroactive.is_empty());
  assert_eq!(rebuilt, live);
}

#[test]
fn new_achievements_are_evaluated_against_history() {
  let storage = Storage::open_in_memory().unwrap();
  let mut progress = GameProgress::default();
  for (at, n) in [(1, 0), (2, 1), (3, 2)] {
    storage.ingest(&game(""), ACCOUNT, &mut progress, wins(n), at).unwrap();
  }

  let extended = game(
    r#"
        [[achievement]]
        id = "first_win"
        name = "a"
        description = "b"
        tier = "bronze"
        condition = { type = "total", stat = "wins", target = 1 }
        "#,
  );
  let (progress, retroactive) = storage.rebuild_progress(&extended, ACCOUNT).unwrap();
  assert_eq!(ids(&retroactive), ["first_win"]);
  assert_eq!(retroactive[0].unlocked_at, 2, "dated by the snapshot that earned it");
  assert!(progress.is_unlocked("wins_2"));

  let (_, again) = storage.rebuild_progress(&extended, ACCOUNT).unwrap();
  assert!(again.is_empty());
}

#[test]
fn stored_unlocks_survive_a_harder_condition() {
  let storage = Storage::open_in_memory().unwrap();
  let mut progress = GameProgress::default();
  storage.ingest(&game(""), ACCOUNT, &mut progress, wins(0), 1).unwrap();
  storage.ingest(&game(""), ACCOUNT, &mut progress, wins(2), 2).unwrap();

  let (progress, _) = storage.rebuild_progress(&game_with_target(50, ""), ACCOUNT).unwrap();
  assert!(progress.is_unlocked("wins_2"));
}

#[test]
fn a_new_account_starts_from_its_own_baseline() {
  let storage = Storage::open_in_memory().unwrap();
  let definition = game("");
  let mut progress = GameProgress::default();
  storage.ingest(&definition, ACCOUNT, &mut progress, wins(0), 1).unwrap();
  storage.ingest(&definition, ACCOUNT, &mut progress, wins(1), 2).unwrap();

  // The second account already has 500 wins: none of them count.
  let (mut other, _) = storage.rebuild_progress(&definition, "other").unwrap();
  assert!(storage.ingest(&definition, "other", &mut other, wins(500), 3).unwrap().is_empty());
  assert!(storage.ingest(&definition, "other", &mut other, wins(501), 4).unwrap().is_empty());
  assert_eq!(ids(&storage.ingest(&definition, "other", &mut other, wins(502), 5).unwrap()), ["wins_2"]);
}

#[test]
fn platform_meta_achievements_cascade() {
  let storage = Storage::open_in_memory().unwrap();
  let definition = game("");
  let platform = GameDefinition::from_toml_str(&format!(
    r#"
        [game]
        id = "{PLATFORM_ID}"
        name = "platform-name"

        [[achievement]]
        id = "first_unlock"
        name = "a"
        description = "b"
        tier = "bronze"
        condition = {{ type = "total", stat = "unlocked", target = 1 }}

        [[achievement]]
        id = "level_2"
        name = "a"
        description = "b"
        tier = "silver"
        condition = {{ type = "total", stat = "level", target = 2 }}
        "#
  ))
  .unwrap();
  let rules = Progression::default();

  let mut progress = GameProgress::default();
  storage.ingest(&definition, ACCOUNT, &mut progress, wins(0), 1).unwrap();
  storage.ingest(&definition, ACCOUNT, &mut progress, wins(2), 2).unwrap();

  // 90 (gold) + 15 (first_unlock) = 105 XP reaches level 2, which unlocks level_2.
  let unlocked = storage.evaluate_platform(&platform, &[&definition], &rules, 9).unwrap();
  assert_eq!(ids(&unlocked), ["first_unlock", "level_2"]);
  assert!(storage.evaluate_platform(&platform, &[&definition], &rules, 10).unwrap().is_empty());

  let summary = storage.account_summary(&[&definition, &platform], &rules).unwrap();
  assert_eq!(summary.total_xp, 90 + 15 + 30);
  assert_eq!(summary.level.level, 2);
  assert_eq!(summary.game_unlocked, 1);
  assert_eq!(summary.by_tier.get(&Tier::Gold), Some(&1));
  assert_eq!(summary.platinums, 0);
}

#[test]
fn summary_ignores_achievements_removed_from_definitions() {
  let storage = Storage::open_in_memory().unwrap();
  storage.insert_unlock("test", "deleted_achievement", 1).unwrap();
  storage.insert_unlock("unknown_game", "x", 1).unwrap();
  let summary = storage.account_summary(&[&game("")], &Progression::default()).unwrap();
  assert_eq!(summary.total_xp, 0);
}
