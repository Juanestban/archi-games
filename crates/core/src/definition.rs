use std::collections::HashSet;

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
  /// Expansion pack; `None` is the base game, the only part platinum requires.
  #[serde(default)]
  pub pack: Option<String>,
  /// Required for every tier except platinum, which unlocks automatically.
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
  #[error("platinum `{0}` must not have a condition, it unlocks by completing the base game")]
  PlatinumWithCondition(String),
  #[error("platinum `{0}` must belong to the base game, not a pack")]
  PlatinumInPack(String),
  #[error("a game can have only one platinum, found `{0}` and `{1}`")]
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

  pub fn platinum(&self) -> Option<&AchievementDef> {
    self.achievements.iter().find(|a| a.tier == Tier::Platinum)
  }

  /// Every Fluent token this definition references.
  pub fn tokens(&self) -> impl Iterator<Item = &str> {
    std::iter::once(self.game.name.as_str()).chain(self.achievements.iter().flat_map(|a| [a.name.as_str(), a.description.as_str()]))
  }

  fn validate(&self) -> Result<(), DefinitionError> {
    if let Some(token) = self.tokens().find(|t| !is_fluent_id(t)) {
      return Err(DefinitionError::InvalidToken(token.to_owned()));
    }

    let mut ids = HashSet::new();
    let mut platinum: Option<&str> = None;
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
          if achievement.pack.is_some() {
            return Err(DefinitionError::PlatinumInPack(id.clone()));
          }
          if let Some(first) = platinum {
            return Err(DefinitionError::MultiplePlatinum(first.to_owned(), id.clone()));
          }
          platinum = Some(id);
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
    Condition::Total { target: 0, .. } => return invalid("target must be greater than 0"),
    Condition::Total { .. } => return Ok(()),
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
            "#,
    )
    .unwrap();

    assert_eq!(definition.achievements.len(), 4);
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
      ach("x", "platinum", "pack = \"p\""),
      format!("{}{}", ach("p1", "platinum", ""), ach("p2", "platinum", "")),
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
