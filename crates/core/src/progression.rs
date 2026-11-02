use serde::Deserialize;

use crate::definition::Tier;

/// XP rewards and level curve. Loaded from config so it can be tuned without
/// recompiling; [`Default`] holds the agreed values.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Progression {
  pub bronze_xp: u64,
  pub silver_xp: u64,
  pub gold_xp: u64,
  pub platinum_xp: u64,
  /// XP to go from level `n` to `n + 1` is `level_base * n ^ level_exponent`.
  pub level_base: f64,
  pub level_exponent: f64,
}

impl Default for Progression {
  fn default() -> Self {
    Self { bronze_xp: 15, silver_xp: 30, gold_xp: 90, platinum_xp: 300, level_base: 100.0, level_exponent: 1.5 }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelInfo {
  /// Starts at 1, no cap.
  pub level: u32,
  pub xp_into_level: u64,
  pub xp_for_next: u64,
}

impl Progression {
  pub fn xp_for(&self, tier: Tier) -> u64 {
    match tier {
      Tier::Bronze => self.bronze_xp,
      Tier::Silver => self.silver_xp,
      Tier::Gold => self.gold_xp,
      Tier::Platinum => self.platinum_xp,
    }
  }

  pub fn xp_to_next(&self, level: u32) -> u64 {
    // Never 0, otherwise `level_for` would loop forever on a bad config.
    ((self.level_base * f64::from(level).powf(self.level_exponent)).round() as u64).max(1)
  }

  pub fn level_for(&self, total_xp: u64) -> LevelInfo {
    let mut level = 1;
    let mut remaining = total_xp;
    loop {
      let needed = self.xp_to_next(level);
      if remaining < needed {
        return LevelInfo { level, xp_into_level: remaining, xp_for_next: needed };
      }
      remaining -= needed;
      level += 1;
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn default_curve() {
    let p = Progression::default();
    assert_eq!(p.xp_to_next(1), 100);
    assert_eq!(p.xp_to_next(2), 283);
    assert_eq!(p.xp_to_next(4), 800);
  }

  #[test]
  fn level_from_total_xp() {
    let p = Progression::default();
    assert_eq!(p.level_for(0), LevelInfo { level: 1, xp_into_level: 0, xp_for_next: 100 });
    assert_eq!(p.level_for(99).level, 1);
    assert_eq!(p.level_for(100), LevelInfo { level: 2, xp_into_level: 0, xp_for_next: 283 });
    assert_eq!(p.level_for(400), LevelInfo { level: 3, xp_into_level: 17, xp_for_next: 520 });
  }

  #[test]
  fn partial_config_keeps_defaults() {
    let p: Progression = toml::from_str("gold_xp = 100").unwrap();
    assert_eq!(p.gold_xp, 100);
    assert_eq!(p.platinum_xp, 300);
  }
}
