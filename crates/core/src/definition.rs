use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use thiserror::Error;

use crate::stats::MODE_ALL;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
  Bronze,
  Silver,
  Gold,
  Platinum,
}

/// One game's achievement list, as written in its `achievements.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameDefinition {
  pub game: GameMeta,
  #[serde(default, rename = "achievement")]
  pub achievements: Vec<AchievementDef>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameMeta {
  pub id: String,
  /// Fluent token.
  pub name: String,
  /// Modes the app adds up itself from local matches (replays, Game State Integration):
  /// their stats start at zero, so one recorded for the first time counts in full. In every
  /// other mode a stat first seen after the baseline counts from when it's first seen.
  #[serde(default)]
  pub local_modes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AchievementDef {
  pub id: String,
  /// Fluent token.
  pub name: String,
  /// Fluent token.
  pub description: String,
  pub tier: Tier,
  /// Secret until unlocked.
  #[serde(default)]
  pub hidden: bool,
  /// Expansion pack; `None` is the base game. Its name is the Fluent token `<game>-pack-<pack>`.
  #[serde(default)]
  pub pack: Option<String>,
  /// Required for every tier except platinum, which unlocks by itself when every other
  /// achievement of its part (the base game or its pack) is unlocked.
  #[serde(default)]
  pub condition: Option<Condition>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Condition {
  /// Accumulated since the account was linked.
  Total {
    stat: String,
    #[serde(default = "mode_all")]
    mode: String,
    target: u64,
  },
  /// Within a single match. Only evaluated when exactly one match was
  /// played in `mode` between two snapshots.
  Match {
    #[serde(default = "mode_all")]
    mode: String,
    all: Vec<Predicate>,
  },
  /// The current value, not what was added since linking: for numbers that start over, like
  /// a season's battle pass level.
  Reach {
    stat: String,
    #[serde(default = "mode_all")]
    mode: String,
    target: u64,
  },
  /// `count` consecutive matches in `mode` each satisfying `all`.
  Streak {
    #[serde(default = "mode_all")]
    mode: String,
    all: Vec<Predicate>,
    count: u32,
  },
}

fn mode_all() -> String {
  MODE_ALL.to_owned()
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Predicate {
  pub stat: String,
  #[serde(default)]
  pub eq: Option<u64>,
  #[serde(default)]
  pub gte: Option<u64>,
  #[serde(default)]
  pub lte: Option<u64>,
}

impl Predicate {
  pub fn matches(&self, value: u64) -> bool {
    self.eq.is_none_or(|eq| value == eq) && self.gte.is_none_or(|gte| value >= gte) && self.lte.is_none_or(|lte| value <= lte)
  }
}

#[derive(Debug, Error)]
pub enum DefinitionError {
  #[error("invalid TOML: {0}")]
  Parse(#[from] toml::de::Error),
  #[error("duplicate achievement id `{0}`")]
  DuplicateId(String),
  #[error("`{0}` is not a valid Fluent token")]
  InvalidToken(String),
  #[error("achievement `{0}` needs a condition")]
  MissingCondition(String),
  #[error("platinum `{0}` must not have a condition, it unlocks by completing its base game or pack")]
  PlatinumWithCondition(String),
  #[error("the base game and each pack can have only one platinum, found `{0}` and `{1}`")]
  MultiplePlatinum(String, String),
  #[error("achievement `{0}` has an invalid condition: {1}")]
  InvalidCondition(String, &'static str),
}

impl GameDefinition {
  pub fn from_toml_str(source: &str) -> Result<Self, DefinitionError> {
    let definition: GameDefinition = toml::from_str(source)?;
    definition.validate()?;
    Ok(definition)
  }

  pub fn get(&self, id: &str) -> Option<&AchievementDef> {
    self.achievements.iter().find(|a| a.id == id)
  }

  /// The base game's platinum.
  pub fn platinum(&self) -> Option<&AchievementDef> {
    self.platinums().find(|a| a.pack.is_none())
  }

  /// The base game's platinum and those of its packs.
  pub fn platinums(&self) -> impl Iterator<Item = &AchievementDef> {
    self.achievements.iter().filter(|a| a.tier == Tier::Platinum)
  }

  /// Every pack, in the order they first appear.
  pub fn packs(&self) -> Vec<&str> {
    let mut packs: Vec<&str> = Vec::new();
    for pack in self.achievements.iter().filter_map(|a| a.pack.as_deref()) {
      if !packs.contains(&pack) {
        packs.push(pack);
      }
    }
    packs
  }

  /// The Fluent token with a pack's name.
  pub fn pack_token(&self, pack: &str) -> String {
    format!("{}-pack-{pack}", self.game.id)
  }

  /// Every Fluent token this definition references, pack names included.
  pub fn tokens(&self) -> Vec<String> {
    let achievements = self.achievements.iter().flat_map(|a| [a.name.clone(), a.description.clone()]);
    let packs = self.packs().into_iter().map(|pack| self.pack_token(pack));
    std::iter::once(self.game.name.clone()).chain(achievements).chain(packs).collect()
  }

  fn validate(&self) -> Result<(), DefinitionError> {
    if let Some(token) = self.tokens().into_iter().find(|t| !is_fluent_id(t)) {
      return Err(DefinitionError::InvalidToken(token));
    }

    let mut ids = HashSet::new();
    let mut platinums: HashMap<Option<&str>, &str> = HashMap::new();
    for achievement in &self.achievements {
      let id = &achievement.id;
      if !ids.insert(id.as_str()) {
        return Err(DefinitionError::DuplicateId(id.clone()));
      }
      match (&achievement.condition, achievement.tier) {
        (Some(_), Tier::Platinum) => {
          return Err(DefinitionError::PlatinumWithCondition(id.clone()));
        }
        (None, Tier::Platinum) => {
          if let Some(first) = platinums.insert(achievement.pack.as_deref(), id) {
            return Err(DefinitionError::MultiplePlatinum(first.to_owned(), id.clone()));
          }
        }
        (None, _) => return Err(DefinitionError::MissingCondition(id.clone())),
        (Some(condition), _) => validate_condition(id, condition)?,
      }
    }
    Ok(())
  }
}

fn validate_condition(id: &str, condition: &Condition) -> Result<(), DefinitionError> {
  let invalid = |reason| Err(DefinitionError::InvalidCondition(id.to_owned(), reason));
  let predicates = match condition {
    Condition::Total { target: 0, .. } | Condition::Reach { target: 0, .. } => return invalid("target must be greater than 0"),
    Condition::Total { .. } | Condition::Reach { .. } => return Ok(()),
    Condition::Streak { count: 0, .. } => return invalid("count must be greater than 0"),
    Condition::Match { all, .. } | Condition::Streak { all, .. } => all,
  };
  if predicates.is_empty() {
    return invalid("`all` needs at least one predicate");
  }
  if predicates.iter().any(|p| p.eq.is_none() && p.gte.is_none() && p.lte.is_none()) {
    return invalid("every predicate needs `eq`, `gte` or `lte`");
  }
  Ok(())
}

/// Fluent message identifier: `[a-zA-Z][a-zA-Z0-9_-]*`.
fn is_fluent_id(token: &str) -> bool {
  let mut chars = token.chars();
  chars.next().is_some_and(|c| c.is_ascii_alphabetic()) && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
  use super::*;

  const HEADER: &str = "[game]\nid = \"g\"\nname = \"g-name\"\n";

  fn parse(achievements: &str) -> Result<GameDefinition, DefinitionError> {
    GameDefinition::from_toml_str(&format!("{HEADER}{achievements}"))
  }

  #[test]
  fn parses_all_condition_types() {
    let definition = parse(
      r#"
            [[achievement]]
            id = "wins_100"
            name = "a"
            description = "b"
            tier = "gold"
            condition = { type = "total", stat = "wins", target = 100 }

            [[achievement]]
            id = "dominant"
            name = "a"
            description = "b"
            tier = "silver"
            hidden = true
            condition = { type = "match", mode = "solo", all = [{ stat = "wins", eq = 1 }, { stat = "kills", gte = 10 }] }

            [[achievement]]
            id = "streak"
            name = "a"
            description = "b"
            tier = "gold"
            pack = "season-2"
            condition = { type = "streak", all = [{ stat = "wins", eq = 1 }], count = 3 }

            [[achievement]]
            id = "plat"
            name = "a"
            description = "b"
            tier = "platinum"

            [[achievement]]
            id = "season_plat"
            name = "a"
            description = "b"
            tier = "platinum"
            pack = "season-2"
            "#,
    )
    .unwrap();

    assert_eq!(definition.achievements.len(), 5);
    assert_eq!(definition.packs(), ["season-2"]);
    assert_eq!(definition.pack_token("season-2"), "g-pack-season-2");
    assert_eq!(definition.platinums().count(), 2);
    assert_eq!(definition.get("wins_100").unwrap().condition, Some(Condition::Total { stat: "wins".into(), mode: MODE_ALL.into(), target: 100 }));
    assert!(definition.get("dominant").unwrap().hidden);
    assert_eq!(definition.get("streak").unwrap().pack.as_deref(), Some("season-2"));
    assert_eq!(definition.platinum().unwrap().id, "plat");
  }

  #[test]
  fn rejects_invalid_definitions() {
    let ach =
      |id: &str, tier: &str, rest: &str| format!("[[achievement]]\nid = \"{id}\"\nname = \"a\"\ndescription = \"b\"\ntier = \"{tier}\"\n{rest}\n");
    let total = "condition = { type = \"total\", stat = \"wins\", target = 1 }";

    let cases = [
      format!("{}{}", ach("x", "bronze", total), ach("x", "bronze", total)),
      ach("x", "bronze", ""),
      ach("x", "platinum", total),
      format!("{}{}", ach("p1", "platinum", ""), ach("p2", "platinum", "")),
      format!("{}{}", ach("p1", "platinum", "pack = \"p\""), ach("p2", "platinum", "pack = \"p\"")),
      ach("x", "bronze", "condition = { type = \"total\", stat = \"wins\", target = 0 }"),
      ach("x", "bronze", "condition = { type = \"match\", all = [] }"),
      ach("x", "bronze", "condition = { type = \"match\", all = [{ stat = \"wins\" }] }"),
      ach("x", "bronze", "condition = { type = \"match\", all = [{ stat = \"wins\", gt = 1 }] }"),
      ach("x", "bronze", "condition = { type = \"streak\", all = [{ stat = \"wins\", eq = 1 }], count = 0 }"),
      ach("x", "diamond", total),
      ach("x", "bronze", total).replace("name = \"a\"", "name = \"has space\""),
    ];
    for case in cases {
      assert!(parse(&case).is_err(), "should reject:\n{case}");
    }
  }

  #[test]
  fn predicate_bounds() {
    let p = Predicate { stat: "kills".into(), eq: None, gte: Some(3), lte: Some(5) };
    assert!(!p.matches(2));
    assert!(p.matches(3));
    assert!(p.matches(5));
    assert!(!p.matches(6));
  }
}
