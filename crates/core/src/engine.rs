use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::definition::{AchievementDef, Condition, GameDefinition, Predicate, Tier};
use crate::stats::{MATCHES, StatSnapshot};

/// Evaluation state of one game for one account.
///
/// Feeding every stored snapshot, in order, into a fresh `GameProgress`
/// rebuilds the same state, which is how newly added achievements get
/// evaluated against the history since install.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GameProgress {
  /// Progress counts from here. Set by the first snapshot unless given.
  pub baseline: Option<StatSnapshot>,
  pub last: Option<StatSnapshot>,
  /// Current streak length per `streak` achievement id.
  pub streaks: BTreeMap<String, u32>,
  pub unlocked: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
  pub current: u64,
  pub target: u64,
}

impl GameProgress {
  /// Starts from an explicit baseline, e.g. an empty snapshot for the
  /// platform's own meta-achievements, whose stats start at zero.
  pub fn with_baseline(baseline: StatSnapshot) -> Self {
    Self { baseline: Some(baseline.clone()), last: Some(baseline), ..Self::default() }
  }

  pub fn is_unlocked(&self, id: &str) -> bool {
    self.unlocked.contains(id)
  }

  /// Applies a new snapshot and returns the achievements it unlocked, in
  /// definition order with the platinum always last.
  pub fn apply<'d>(&mut self, definition: &'d GameDefinition, snapshot: StatSnapshot) -> Vec<&'d AchievementDef> {
    let (Some(baseline), Some(previous)) = (&self.baseline, &self.last) else {
      self.baseline = Some(snapshot.clone());
      self.last = Some(snapshot);
      return Vec::new();
    };

    let delta = snapshot.delta(previous);
    let mut unlocked = Vec::new();

    for achievement in &definition.achievements {
      let Some(condition) = &achievement.condition else { continue };
      let met = match condition {
        Condition::Total { stat, mode, target } => snapshot.get(mode, stat).saturating_sub(baseline.get(mode, stat)) >= *target,
        Condition::Reach { stat, mode, target } => snapshot.get(mode, stat) >= *target,
        Condition::Match { mode, all } => single_match(&delta, mode, all) == Some(true),
        Condition::Streak { mode, all, count } => {
          let streak = self.streaks.entry(achievement.id.clone()).or_default();
          match (delta.get(mode, MATCHES), single_match(&delta, mode, all)) {
            (0, _) => {}
            (_, Some(true)) => *streak += 1,
            // A failed match, or several matches we can't tell
            // apart: never award a doubtful streak.
            _ => *streak = 0,
          }
          *streak >= *count
        }
      };
      if met && self.unlocked.insert(achievement.id.clone()) {
        unlocked.push(achievement);
      }
    }

    if let Some(platinum) = definition.platinum()
      && !self.is_unlocked(&platinum.id)
      && base_game_complete(definition, &self.unlocked)
    {
      self.unlocked.insert(platinum.id.clone());
      unlocked.push(platinum);
    }

    self.last = Some(snapshot);
    unlocked
  }

  /// Progress bar for `total` and `reach` achievements, capped at the target.
  pub fn progress(&self, achievement: &AchievementDef) -> Option<Progress> {
    let (current, target) = match (&achievement.condition, &self.baseline, &self.last) {
      (Some(Condition::Total { stat, mode, target }), Some(baseline), Some(last)) => {
        (last.get(mode, stat).saturating_sub(baseline.get(mode, stat)), *target)
      }
      (Some(Condition::Reach { stat, mode, target }), _, Some(last)) => (last.get(mode, stat), *target),
      (Some(Condition::Total { target, .. } | Condition::Reach { target, .. }), _, _) => (0, *target),
      _ => return None,
    };
    Some(Progress { current: current.min(target), target })
  }
}

/// `Some(result)` when exactly one match was played in `mode`, `None` when
/// the delta can't be attributed to a single match.
fn single_match(delta: &StatSnapshot, mode: &str, predicates: &[Predicate]) -> Option<bool> {
  (delta.get(mode, MATCHES) == 1).then(|| predicates.iter().all(|p| p.matches(delta.get(mode, &p.stat))))
}

fn base_game_complete(definition: &GameDefinition, unlocked: &BTreeSet<String>) -> bool {
  let mut base = definition.achievements.iter().filter(|a| a.pack.is_none() && a.tier != Tier::Platinum).peekable();
  base.peek().is_some() && base.all(|a| unlocked.contains(&a.id))
}
