//! Unlock notifications: the app's own small window in the bottom-right
//! corner (borderless, transparent, always on top, click-through), so it
//! shows over a windowed or borderless game without touching it (ADR 0001).
//! Cards queue and show one at a time.

use std::collections::VecDeque;

use archi_core::Tier;
use archi_storage::Unlock;
use eframe::egui::{self, Color32, Rect, Stroke, Ui, Vec2, ViewportBuilder, ViewportId, pos2, vec2};

use super::{ArchiApp, theme, widgets};
use crate::catalog::Catalog;
use crate::sound::Cue;

/// Seconds each card stays, sliding in and fading out included.
const SHOW_FOR: f64 = 5.0;
const SLIDE_IN: f64 = 0.35;
const FADE_OUT: f64 = 0.4;
const SIZE: Vec2 = vec2(400.0, 96.0);
/// Distance to the screen's right and bottom edges (clears the taskbar).
const MARGIN: Vec2 = vec2(24.0, 72.0);

#[derive(Debug, Clone, PartialEq)]
pub enum Toast {
  Unlock { name: String, tier: Tier, xp: u64 },
  LevelUp(u32),
}

impl Toast {
  fn cue(&self) -> Cue {
    match self {
      Self::Unlock { tier, .. } => Cue::Tier(*tier),
      Self::LevelUp(_) => Cue::LevelUp,
    }
  }
}

/// Unlocks as cards: platinums always last, then the level-up card.
pub fn toasts(catalog: &Catalog, unlocks: &[Unlock], new_level: Option<u32>) -> Vec<Toast> {
  let mut cards: Vec<_> = unlocks
    .iter()
    .filter_map(|unlock| {
      let achievement = catalog.get(&unlock.game_id)?.get(&unlock.achievement_id)?;
      Some(Toast::Unlock { name: achievement.name.clone(), tier: achievement.tier, xp: catalog.rules.xp_for(achievement.tier) })
    })
    .collect();
  cards.sort_by_key(|card| matches!(card, Toast::Unlock { tier: Tier::Platinum, .. }));
  cards.extend(new_level.map(Toast::LevelUp));
  cards
}

#[derive(Default)]
pub struct Notifier {
  queue: VecDeque<Toast>,
  /// When the front card appeared (egui time, seconds).
  since: Option<f64>,
}

pub struct Showing<'a> {
  pub toast: &'a Toast,
  pub elapsed: f64,
  /// First frame of this card: time to play its sound.
  pub started: bool,
}

impl Notifier {
  pub fn push(&mut self, toasts: impl IntoIterator<Item = Toast>) {
    self.queue.extend(toasts);
  }

  pub fn is_active(&self) -> bool {
    !self.queue.is_empty()
  }

  /// The card to draw at `now`, dropping the one whose time is up.
  pub fn current(&mut self, now: f64) -> Option<Showing<'_>> {
    if self.since.is_some_and(|since| now - since >= SHOW_FOR) {
      self.queue.pop_front();
      self.since = None;
    }
    let started = self.since.is_none();
    let toast = self.queue.front()?;
    let since = *self.since.get_or_insert(now);
    Some(Showing { toast, elapsed: now - since, started })
  }
}

impl ArchiApp {
  pub(super) fn notifications(&mut self, ctx: &egui::Context) {
    let now = ctx.input(|i| i.time);
    let Some(showing) = self.notifier.current(now) else { return };
    let (toast, elapsed) = (showing.toast.clone(), showing.elapsed);
    if showing.started {
      self.sounds.play(toast.cue(), self.volume);
    }
    ctx.request_repaint();

    let monitor = ctx.input(|i| i.viewport().monitor_size).unwrap_or(vec2(1920.0, 1080.0));
    let builder = ViewportBuilder::default()
      .with_title("Archi Games")
      .with_decorations(false)
      .with_transparent(true)
      .with_always_on_top()
      .with_mouse_passthrough(true)
      .with_taskbar(false)
      .with_active(false)
      .with_resizable(false)
      .with_inner_size(SIZE)
      .with_position(pos2(monitor.x - SIZE.x - MARGIN.x, monitor.y - SIZE.y - MARGIN.y));
    ctx.show_viewport_immediate(ViewportId::from_hash_of("archi-notification"), builder, |ui, _| self.draw_toast(ui, &toast, elapsed));
  }

  fn draw_toast(&self, ui: &mut Ui, toast: &Toast, elapsed: f64) {
    let t = elapsed as f32;
    ui.set_opacity(((SHOW_FOR - elapsed) / FADE_OUT).clamp(0.0, 1.0) as f32);
    let slide = widgets::ease_out(t / SLIDE_IN as f32);
    let rect = Rect::from_min_size(pos2((1.0 - slide) * SIZE.x, 0.0), SIZE).shrink(1.0);
    let p = ui.painter().with_clip_rect(rect.expand(1.0));

    let (accent, title, name, xp) = match toast {
      Toast::Unlock { name, tier, xp } => {
        let title = if *tier == Tier::Platinum { "ui-toast-platinum" } else { "ui-toast-unlocked" };
        (theme::tier_color(*tier), self.tr(title), self.tr(name), Some(*xp))
      }
      Toast::LevelUp(level) => (theme::ACCENT_400, self.tr("ui-toast-level-up"), self.tr_args("ui-level", &[("level", (*level).into())]), None),
    };
    let platinum = matches!(toast, Toast::Unlock { tier: Tier::Platinum, .. });

    // Square card, hairline in the tier's color, a stripe on the left.
    p.rect(rect, 0.0, theme::PANEL, Stroke::new(1.0, theme::fade(accent, 0.55)), egui::StrokeKind::Inside);
    p.rect_filled(Rect::from_min_size(rect.min, vec2(3.0, rect.height())), 0.0, accent);
    let badge = Rect::from_center_size(pos2(rect.left() + 46.0, rect.center().y), vec2(54.0, 54.0));
    match toast {
      Toast::Unlock { tier, .. } => widgets::paint_tier_badge(&p, badge, *tier, true),
      Toast::LevelUp(_) => level_badge(&p, badge),
    }
    let text_x = badge.right() + 16.0;
    p.galley(pos2(text_x, rect.top() + 22.0), widgets::caps(ui, &title, 10.5, accent), accent);
    p.galley(pos2(text_x, rect.top() + 40.0), widgets::display_text(ui, &name, 19.0, theme::INK), theme::INK);
    if let Some(xp) = xp {
      let reward = self.tr_args("ui-xp-reward", &[("xp", xp.into())]);
      p.text(pos2(rect.right() - 16.0, rect.center().y), egui::Align2::RIGHT_CENTER, reward, theme::mono(14.0), accent);
    }

    // A band of light sweeps across once; on a platinum it keeps sweeping.
    let sweep = if platinum { (t - 0.25).rem_euclid(1.6) / 0.9 } else { (t - 0.25) / 0.75 };
    if (0.0..1.0).contains(&sweep) {
      let x = rect.left() - 90.0 + (rect.width() + 180.0) * widgets::ease_out(sweep);
      let band = vec![pos2(x, rect.top()), pos2(x + 46.0, rect.top()), pos2(x - 4.0, rect.bottom()), pos2(x - 50.0, rect.bottom())];
      let glow = if platinum { 0.16 } else { 0.08 };
      p.add(egui::Shape::convex_polygon(band, theme::fade(Color32::WHITE, glow), Stroke::NONE));
    }
    // Time left, as a line along the bottom.
    let left = (1.0 - elapsed / SHOW_FOR).clamp(0.0, 1.0) as f32;
    p.hline(rect.left()..=rect.left() + rect.width() * left, rect.bottom() - 1.0, Stroke::new(2.0, theme::fade(accent, 0.7)));
  }

  /// Shows a sample of every kind of card (Settings > Sound).
  pub(super) fn test_notifications(&mut self) {
    let rules = &self.catalog.rules;
    let sample = |tier| Toast::Unlock { name: "ui-toast-sample".into(), tier, xp: rules.xp_for(tier) };
    let cards = [sample(Tier::Gold), sample(Tier::Platinum), Toast::LevelUp(self.summary.level.level + 1)];
    self.notifier.push(cards);
  }
}

fn level_badge(p: &egui::Painter, rect: Rect) {
  p.rect(rect, theme::RADIUS, theme::SUNKEN, Stroke::new(1.0, theme::fade(theme::ACCENT_400, 0.6)), egui::StrokeKind::Inside);
  p.circle_filled(rect.center(), rect.width() * 0.42, theme::fade(theme::ACCENT, 0.18));
  let c = rect.center();
  let arrow = vec![
    pos2(c.x, c.y - 12.0),
    pos2(c.x + 12.0, c.y + 4.0),
    pos2(c.x + 5.0, c.y + 4.0),
    pos2(c.x + 5.0, c.y + 12.0),
    pos2(c.x - 5.0, c.y + 12.0),
    pos2(c.x - 5.0, c.y + 4.0),
    pos2(c.x - 12.0, c.y + 4.0),
  ];
  p.add(egui::Shape::closed_line(arrow, Stroke::new(1.5, theme::ACCENT_400)));
}

#[cfg(test)]
mod tests {
  use super::*;

  fn unlock(game_id: &str, achievement_id: &str) -> Unlock {
    Unlock { game_id: game_id.into(), achievement_id: achievement_id.into(), tier: Tier::Bronze, unlocked_at: 0 }
  }

  fn names(toasts: &[Toast]) -> Vec<String> {
    toasts
      .iter()
      .map(|toast| match toast {
        Toast::Unlock { name, .. } => name.clone(),
        Toast::LevelUp(level) => format!("level {level}"),
      })
      .collect()
  }

  #[test]
  fn platinum_goes_last_then_the_level_up() {
    let catalog = Catalog::bundled().unwrap();
    let unlocks = [unlock("fortnite", "platinum"), unlock("fortnite", "first_win"), unlock("platform", "first_platinum"), unlock("fortnite", "gone")];
    let toasts = toasts(&catalog, &unlocks, Some(4));
    assert_eq!(names(&toasts), ["fortnite-first_win-name", "platform-first_platinum-name", "fortnite-platinum-name", "level 4"]);
  }

  #[test]
  fn cards_show_one_at_a_time() {
    let mut notifier = Notifier::default();
    notifier.push([Toast::LevelUp(2), Toast::LevelUp(3)]);

    let first = notifier.current(10.0).unwrap();
    assert!(first.started && first.toast == &Toast::LevelUp(2));
    let later = notifier.current(12.0).unwrap();
    assert!(!later.started && later.elapsed == 2.0);

    let second = notifier.current(15.0).unwrap();
    assert!(second.started && second.toast == &Toast::LevelUp(3));
    assert!(notifier.current(20.0).is_none());
    assert!(!notifier.is_active());
  }
}
