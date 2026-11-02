use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Aggregate mode every tracker must fill with the totals across all modes.
pub const MODE_ALL: &str = "all";

/// Stat counting played matches; `match` and `streak` conditions rely on it.
pub const MATCHES: &str = "matches";

/// Cumulative stats of an account at one point in time, keyed by mode then stat.
///
/// Missing values read as 0, so trackers only need to set what they know.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatSnapshot {
  modes: BTreeMap<String, BTreeMap<String, u64>>,
}

impl StatSnapshot {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn set(&mut self, mode: impl Into<String>, stat: impl Into<String>, value: u64) {
    self.modes.entry(mode.into()).or_default().insert(stat.into(), value);
  }

  /// Builder form of [`set`](Self::set).
  pub fn with(mut self, mode: impl Into<String>, stat: impl Into<String>, value: u64) -> Self {
    self.set(mode, stat, value);
    self
  }

  pub fn get(&self, mode: &str, stat: &str) -> u64 {
    self.modes.get(mode).and_then(|stats| stats.get(stat)).copied().unwrap_or(0)
  }

  /// What happened between `previous` and `self`. Stats that went down
  /// (e.g. an upstream reset) count as 0 instead of underflowing.
  pub fn delta(&self, previous: &StatSnapshot) -> StatSnapshot {
    let mut delta = StatSnapshot::new();
    for (mode, stats) in &self.modes {
      for (stat, &value) in stats {
        delta.set(mode, stat, value.saturating_sub(previous.get(mode, stat)));
      }
    }
    delta
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn missing_stats_read_as_zero() {
    let snapshot = StatSnapshot::new().with("solo", "wins", 3);
    assert_eq!(snapshot.get("solo", "wins"), 3);
    assert_eq!(snapshot.get("solo", "kills"), 0);
    assert_eq!(snapshot.get("duo", "wins"), 0);
  }

  #[test]
  fn delta_saturates_on_decrease() {
    let previous = StatSnapshot::new().with("all", "wins", 10).with("all", "kills", 50);
    let current = StatSnapshot::new().with("all", "wins", 12).with("all", "kills", 40);
    let delta = current.delta(&previous);
    assert_eq!(delta.get("all", "wins"), 2);
    assert_eq!(delta.get("all", "kills"), 0);
  }
}
