//! The bundled game definitions and progression rules, shared read-only by
//! the UI and the tracker thread.

use archi_core::{DefinitionError, GameDefinition, Progression};

pub struct Catalog {
  pub fortnite: GameDefinition,
  pub platform: GameDefinition,
  pub rules: Progression,
}

impl Catalog {
  pub fn bundled() -> Result<Self, DefinitionError> {
    Ok(Self {
      fortnite: GameDefinition::from_toml_str(include_str!("../../../games/fortnite/achievements.toml"))?,
      platform: GameDefinition::from_toml_str(include_str!("../../../games/platform/achievements.toml"))?,
      rules: Progression::default(),
    })
  }

  /// Real games, without the platform's meta-achievements.
  pub fn games(&self) -> [&GameDefinition; 1] {
    [&self.fortnite]
  }

  /// Every definition, the platform's last.
  pub fn all(&self) -> [&GameDefinition; 2] {
    [&self.fortnite, &self.platform]
  }

  pub fn get(&self, game_id: &str) -> Option<&GameDefinition> {
    self.all().into_iter().find(|definition| definition.game.id == game_id)
  }
}
