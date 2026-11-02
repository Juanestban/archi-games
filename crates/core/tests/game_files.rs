//! Every bundled game definition must parse and be fully translated.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use archi_core::GameDefinition;

const LOCALES: [&str; 2] = ["es", "en"];

/// Message ids defined in a Fluent file (top-level `id = ...` lines).
fn fluent_ids(source: &str) -> BTreeSet<String> {
  source
    .lines()
    .filter(|line| line.starts_with(|c: char| c.is_ascii_alphabetic()))
    .filter_map(|line| line.split_once('=').map(|(id, _)| id.trim().to_owned()))
    .collect()
}

#[test]
fn bundled_games_parse_and_are_translated() {
  let games_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../games");
  let mut checked = 0;

  for entry in fs::read_dir(&games_dir).unwrap() {
    let dir = entry.unwrap().path();
    let source = fs::read_to_string(dir.join("achievements.toml")).unwrap();
    let definition = GameDefinition::from_toml_str(&source).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    let tokens: BTreeSet<String> = definition.tokens().map(str::to_owned).collect();

    for locale in LOCALES {
      let path = dir.join("locales").join(format!("{locale}.ftl"));
      let ids = fluent_ids(&fs::read_to_string(&path).unwrap());
      let missing: Vec<_> = tokens.difference(&ids).collect();
      let unused: Vec<_> = ids.difference(&tokens).collect();
      assert!(missing.is_empty(), "{}: missing {missing:?}", path.display());
      assert!(unused.is_empty(), "{}: unused {unused:?}", path.display());
    }
    checked += 1;
  }

  assert!(checked > 0, "no games found in {}", games_dir.display());
}
