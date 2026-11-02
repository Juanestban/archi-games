//! Dark "game menu" look: deep navy panels, a violet accent and metallic
//! tier colors.

use archi_core::Tier;
use eframe::egui::{self, Color32, CornerRadius, Frame, Margin, RichText, Sense, Stroke, Ui, Vec2};

pub const BACKGROUND: Color32 = Color32::from_rgb(13, 16, 30);
pub const PANEL: Color32 = Color32::from_rgb(20, 24, 44);
pub const CARD: Color32 = Color32::from_rgb(28, 33, 60);
pub const ACCENT: Color32 = Color32::from_rgb(124, 92, 255);
pub const TEXT: Color32 = Color32::from_rgb(228, 230, 245);
pub const MUTED: Color32 = Color32::from_rgb(140, 146, 180);
pub const DANGER: Color32 = Color32::from_rgb(255, 110, 120);
pub const SUCCESS: Color32 = Color32::from_rgb(110, 220, 160);

pub fn apply(ctx: &egui::Context) {
  let mut visuals = egui::Visuals::dark();
  visuals.panel_fill = BACKGROUND;
  visuals.window_fill = PANEL;
  visuals.extreme_bg_color = Color32::from_rgb(10, 12, 24);
  visuals.override_text_color = Some(TEXT);
  visuals.selection.bg_fill = ACCENT;
  visuals.hyperlink_color = ACCENT;
  visuals.widgets.inactive.weak_bg_fill = CARD;
  visuals.widgets.inactive.bg_fill = CARD;
  visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(44, 50, 88);
  visuals.widgets.active.weak_bg_fill = ACCENT;
  ctx.set_visuals(visuals);
}

pub fn tier_color(tier: Tier) -> Color32 {
  match tier {
    Tier::Bronze => Color32::from_rgb(205, 127, 50),
    Tier::Silver => Color32::from_rgb(192, 198, 210),
    Tier::Gold => Color32::from_rgb(255, 200, 60),
    Tier::Platinum => Color32::from_rgb(150, 210, 255),
  }
}

pub fn tier_token(tier: Tier) -> &'static str {
  match tier {
    Tier::Bronze => "ui-tier-bronze",
    Tier::Silver => "ui-tier-silver",
    Tier::Gold => "ui-tier-gold",
    Tier::Platinum => "ui-tier-platinum",
  }
}

pub fn nav_frame() -> Frame {
  Frame::NONE.fill(PANEL).inner_margin(Margin::same(16))
}

pub fn content_frame() -> Frame {
  Frame::NONE.fill(BACKGROUND).inner_margin(Margin::same(24))
}

pub fn card() -> Frame {
  Frame::NONE.fill(CARD).corner_radius(CornerRadius::same(10)).inner_margin(Margin::same(14))
}

/// A trophy-like disc in the tier's color; dimmed while locked.
pub fn tier_badge(ui: &mut Ui, tier: Tier, unlocked: bool, size: f32) {
  let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
  let color = if unlocked { tier_color(tier) } else { tier_color(tier).gamma_multiply(0.25) };
  let painter = ui.painter();
  let center = rect.center();
  painter.circle_filled(center, size * 0.5, color);
  painter.circle_stroke(center, size * 0.36, Stroke::new(size * 0.06, Color32::from_black_alpha(if unlocked { 90 } else { 40 })));
  if tier == Tier::Platinum && unlocked {
    painter.circle_stroke(center, size * 0.5, Stroke::new(2.0, Color32::WHITE));
  }
}

pub fn heading(ui: &mut Ui, text: impl Into<String>) {
  ui.label(RichText::new(text).size(26.0).strong());
  ui.add_space(12.0);
}

pub fn muted(text: impl Into<String>) -> RichText {
  RichText::new(text).color(MUTED)
}
