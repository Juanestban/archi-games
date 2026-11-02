//! When to poll a tracker: often while the game runs, in a burst after it
//! closes (stats APIs lag behind the match) and rarely otherwise, to catch
//! matches played on other devices.

use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollPolicy {
  /// Interval while the game is running.
  pub running: Duration,
  /// Interval during `after_exit_window` once the game closes.
  pub after_exit: Duration,
  pub after_exit_window: Duration,
  /// Interval otherwise.
  pub idle: Duration,
}

impl Default for PollPolicy {
  fn default() -> Self {
    Self {
      running: Duration::from_secs(60),
      after_exit: Duration::from_secs(30),
      after_exit_window: Duration::from_secs(5 * 60),
      idle: Duration::from_secs(12 * 60),
    }
  }
}

#[derive(Debug, Clone)]
pub struct Poller {
  policy: PollPolicy,
  running: bool,
  exited_at: Option<Instant>,
  last_poll: Option<Instant>,
  not_before: Option<Instant>,
}

impl Poller {
  pub fn new(policy: PollPolicy) -> Self {
    Self { policy, running: false, exited_at: None, last_poll: None, not_before: None }
  }

  /// Call every few seconds with whether the game is running. Returns true
  /// when a poll is due now, and counts it as done. Closing the game makes a
  /// poll due right away.
  pub fn poll_due(&mut self, running: bool, now: Instant) -> bool {
    let closed = self.running && !running;
    if closed {
      self.exited_at = Some(now);
    } else if running {
      self.exited_at = None;
    }
    self.running = running;

    if self.not_before.is_some_and(|until| now < until) {
      return false;
    }
    let due = closed || self.last_poll.is_none_or(|last| now.duration_since(last) >= self.interval(now));
    if due {
      self.last_poll = Some(now);
      self.not_before = None;
    }
    due
  }

  /// Holds every poll until `until`, e.g. after being rate limited.
  pub fn defer(&mut self, until: Instant) {
    self.not_before = Some(until);
  }

  fn interval(&self, now: Instant) -> Duration {
    if self.running {
      self.policy.running
    } else if self.exited_at.is_some_and(|at| now.duration_since(at) < self.policy.after_exit_window) {
      self.policy.after_exit
    } else {
      self.policy.idle
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
  }

  #[test]
  fn idle_polls_rarely() {
    let t0 = Instant::now();
    let mut poller = Poller::new(PollPolicy::default());
    assert!(poller.poll_due(false, t0));
    assert!(!poller.poll_due(false, t0 + secs(5 * 60)));
    assert!(poller.poll_due(false, t0 + secs(12 * 60)));
  }

  #[test]
  fn running_polls_every_minute() {
    let t0 = Instant::now();
    let mut poller = Poller::new(PollPolicy::default());
    assert!(poller.poll_due(true, t0));
    assert!(!poller.poll_due(true, t0 + secs(55)));
    assert!(poller.poll_due(true, t0 + secs(60)));
  }

  #[test]
  fn closing_the_game_starts_a_burst_then_goes_idle() {
    let t0 = Instant::now();
    let mut poller = Poller::new(PollPolicy::default());
    assert!(poller.poll_due(true, t0));
    assert!(poller.poll_due(false, t0 + secs(10)), "poll right when the game closes");
    assert!(!poller.poll_due(false, t0 + secs(30)));
    assert!(poller.poll_due(false, t0 + secs(40)));
    assert!(poller.poll_due(false, t0 + secs(70)));
    assert!(poller.poll_due(false, t0 + secs(280)));
    // Burst window (5 min since closing) over: back to the idle interval.
    assert!(!poller.poll_due(false, t0 + secs(320)));
    assert!(!poller.poll_due(false, t0 + secs(280 + 11 * 60)));
    assert!(poller.poll_due(false, t0 + secs(280 + 12 * 60)));
  }

  #[test]
  fn defer_holds_polls() {
    let t0 = Instant::now();
    let mut poller = Poller::new(PollPolicy::default());
    assert!(poller.poll_due(true, t0));
    poller.defer(t0 + secs(300));
    assert!(!poller.poll_due(true, t0 + secs(120)));
    assert!(poller.poll_due(true, t0 + secs(300)));
  }
}
