use archi_core::{GameDefinition, Progression};
use archi_storage::Storage;

fn main() {
  // Placeholder until the UI step; for now proves the data and database load.
  let fortnite = GameDefinition::from_toml_str(include_str!("../../../games/fortnite/achievements.toml")).expect("bundled Fortnite definition");
  let platform = GameDefinition::from_toml_str(include_str!("../../../games/platform/achievements.toml")).expect("bundled platform definition");

  let path = Storage::default_path().expect("data directory");
  let storage = Storage::open(&path).expect("open database");
  let summary = storage.account_summary(&[&fortnite, &platform], &Progression::default()).expect("summary");

  println!("{}: {} achievements", fortnite.game.id, fortnite.achievements.len());
  println!("database: {}", path.display());
  println!("level {} ({} XP)", summary.level.level, summary.total_xp);
}
