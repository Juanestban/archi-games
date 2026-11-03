//! The bundled game definitions and progression rules, shared read-only by
//! the UI and the tracker thread.

use archi_core::{DefinitionError, GameDefinition, Progression};

pub struct Catalog {
  /// Real games, in the order the app shows them.
  games: Vec<GameDefinition>,
  pub platform: GameDefinition,
  pub rules: Progression,
}

impl Catalog {
  pub fn bundled() -> Result<Self, DefinitionError> {
    Ok(Self {
      games: vec![
        GameDefinition::from_toml_str(include_str!("../../../games/fortnite/achievements.toml"))?,
        GameDefinition::from_toml_str(include_str!("../../../games/cs2/achievements.toml"))?,
      ],
      platform: GameDefinition::from_toml_str(include_str!("../../../games/platform/achievements.toml"))?,
      rules: Progression::default(),
    })
  }

  /// Real games, without the platform's meta-achievements.
  pub fn games(&self) -> Vec<&GameDefinition> {
    self.games.iter().collect()
  }

  /// Every definition, the platform's last.
  pub fn all(&self) -> Vec<&GameDefinition> {
    self.games.iter().chain([&self.platform]).collect()
  }

  pub fn get(&self, game_id: &str) -> Option<&GameDefinition> {
    self.all().into_iter().find(|definition| definition.game.id == game_id)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::i18n::{I18n, Language};

  /// What Settings shows for a game polled through an API, and for one fed by the game (CS2).
  const API_TEXTS: [&str; 9] =
    ["name", "account", "account-hint", "api-key", "api-key-help", "closed-note", "error-private", "error-not-found", "error-network"];
  const LOCAL_TEXTS: [&str; 16] = [
    "name",
    "integration",
    "integration-help",
    "integration-enable",
    "integration-disable",
    "integration-off",
    "integration-waiting",
    "integration-receiving",
    "integration-account",
    "running",
    "closed",
    "last-match",
    "error-not-found",
    "error-port",
    "error-install",
    "error-private",
  ];

  /// Settings shows these for each game, so every game needs them in both languages.
  #[test]
  fn every_game_has_its_settings_texts() {
    let catalog = Catalog::bundled().unwrap();
    for language in [Language::Es, Language::En] {
      let i18n = I18n::new(language);
      for game in catalog.games() {
        let suffixes: &[&str] = if game.game.id == archi_cs2::GAME_ID { &LOCAL_TEXTS } else { &API_TEXTS };
        for suffix in suffixes {
          let token = format!("{}-{suffix}", game.game.id);
          assert_ne!(i18n.tr(&token), token, "{token} missing in {language:?}");
        }
        for achievement in &game.achievements {
          assert_ne!(i18n.tr(&achievement.name), achievement.name, "{} missing in {language:?}", achievement.name);
          assert_ne!(i18n.tr(&achievement.description), achievement.description);
        }
      }
    }
  }
}
