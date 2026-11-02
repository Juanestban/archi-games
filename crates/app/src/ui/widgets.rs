//! Shared widgets in the ArtCraft style: mono upper-case labels, flat buttons, segmented
//! controls, hairline cards and rows that light up on hover.

use std::sync::Arc;

use archi_core::Tier;
use eframe::egui::{self, Color32, CornerRadius, Frame, Galley, Id, Margin, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, pos2, vec2};

use super::icons::{self, Icon};
use super::theme;

pub fn ease_out(t: f32) -> f32 {
  1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// Upper-case mono text, tracked out like a HUD.
pub fn caps(ui: &Ui, text: &str, size: f32, color: Color32) -> Arc<Galley> {
  let format = egui::TextFormat { font_id: theme::mono(size), color, extra_letter_spacing: size * 0.15, ..Default::default() };
  ui.painter().layout_job(egui::text::LayoutJob::single_section(text.to_uppercase(), format))
}

/// Archivo at its display weight and width, for painter-drawn headings and numbers.
pub fn display_text(ui: &Ui, text: &str, size: f32, color: Color32) -> Arc<Galley> {
  let format = egui::TextFormat {
    font_id: theme::display(size),
    color,
    coords: egui::epaint::text::VariationCoords::new([("wght", 620.0), ("wdth", 118.0)]),
    ..Default::default()
  };
  ui.painter().layout_job(egui::text::LayoutJob::single_section(text.to_owned(), format))
}

pub fn hud(ui: &mut Ui, text: &str, color: Color32) -> Response {
  let galley = caps(ui, text, theme::HUD_SIZE, color);
  let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
  ui.painter().galley(rect.min, galley, color);
  response
}

/// A section heading: HUD label with a hairline running to the right edge.
pub fn section(ui: &mut Ui, text: &str) {
  ui.add_space(6.0);
  let galley = caps(ui, text, theme::HUD_SIZE, theme::MUTED);
  let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), galley.size().y + 6.0), Sense::hover());
  let y = rect.center().y;
  ui.painter().galley(pos2(rect.left(), y - galley.size().y / 2.0), galley.clone(), theme::MUTED);
  let start = rect.left() + galley.size().x + 12.0;
  if start < rect.right() {
    ui.painter().hline(start..=rect.right(), y, theme::hairline());
  }
  ui.add_space(4.0);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
  /// White block, near-black text: the one main action.
  Primary,
  Accent,
  /// Outlined.
  Secondary,
}

/// A flat button with a mono label; hover fades in.
pub fn button(ui: &mut Ui, text: &str, kind: Kind) -> Response {
  let enabled = ui.is_enabled();
  let galley = caps(ui, text, theme::HUD_SIZE, theme::INK);
  let (rect, response) = ui.allocate_exact_size(vec2(galley.size().x + 36.0, theme::CONTROL_H), Sense::click());
  let hover = ui.ctx().animate_bool_with_time(response.id, response.hovered() && enabled, 0.12);
  let (fill, stroke, ink) = match kind {
    Kind::Primary => (theme::fade(Color32::WHITE, 1.0 - 0.12 * hover), Stroke::NONE, Color32::from_rgb(11, 11, 12)),
    Kind::Accent => (lerp(theme::ACCENT, theme::ACCENT_400, hover), Stroke::NONE, Color32::WHITE),
    Kind::Secondary => {
      (theme::fade(Color32::WHITE, hover), Stroke::new(1.0, theme::LINE_STRONG), lerp(theme::INK, Color32::from_rgb(11, 11, 12), hover))
    }
  };
  let dim = |c: Color32| if enabled { c } else { theme::fade(c, 0.4) };
  ui.painter().rect(rect, theme::RADIUS, dim(fill), Stroke::new(stroke.width, dim(stroke.color)), egui::StrokeKind::Inside);
  ui.painter().galley_with_override_text_color(rect.center() - galley.size() / 2.0, galley, dim(ink));
  if enabled { response.on_hover_cursor(egui::CursorIcon::PointingHand) } else { response }
}

/// Joined toggle buttons; returns the index clicked.
pub fn segmented(ui: &mut Ui, options: &[String], selected: usize) -> Option<usize> {
  let mut clicked = None;
  let galleys: Vec<_> = options.iter().map(|o| caps(ui, o, theme::HUD_SIZE, theme::INK)).collect();
  let width: f32 = galleys.iter().map(|g| g.size().x + 28.0).sum();
  let (rect, _) = ui.allocate_exact_size(vec2(width, 30.0), Sense::hover());
  ui.painter().rect(rect, theme::RADIUS, theme::SUNKEN, theme::hairline(), egui::StrokeKind::Inside);
  let mut x = rect.left();
  for (i, galley) in galleys.into_iter().enumerate() {
    let cell = Rect::from_min_size(pos2(x, rect.top()), vec2(galley.size().x + 28.0, rect.height()));
    x = cell.right();
    let response = ui.interact(cell, ui.scope_id().with(("segment", i, &options[i])), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    let hover = ui.ctx().animate_bool_with_time(response.id, response.hovered(), 0.12);
    let on = i == selected;
    if on {
      ui.painter().rect_filled(cell.shrink(3.0), CornerRadius::same(2), Color32::WHITE);
    } else if hover > 0.0 {
      ui.painter().rect_filled(cell.shrink(3.0), CornerRadius::same(2), theme::fade(theme::WASH_HOVER, hover));
    }
    let ink = if on { Color32::from_rgb(11, 11, 12) } else { lerp(theme::MUTED, Color32::WHITE, hover) };
    ui.painter().galley_with_override_text_color(cell.center() - galley.size() / 2.0, galley, ink);
    if response.clicked() {
      clicked = Some(i);
    }
  }
  clicked
}

/// An on/off switch whose knob slides.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
  let (rect, mut response) = ui.allocate_exact_size(vec2(38.0, 20.0), Sense::click());
  if response.clicked() {
    *on = !*on;
    response.mark_changed();
  }
  let t = ui.ctx().animate_bool_with_time(response.id, *on, 0.15);
  let track = lerp(theme::SUNKEN, theme::ACCENT, t);
  ui.painter().rect(rect, CornerRadius::same(10), track, Stroke::new(1.0, lerp(theme::LINE, theme::ACCENT, t)), egui::StrokeKind::Inside);
  let x = rect.left() + 10.0 + t * (rect.width() - 20.0);
  ui.painter().circle_filled(pos2(x, rect.center().y), 7.0, lerp(theme::MUTED, Color32::WHITE, t));
  response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A thin bar in a sunken well. `fraction` is drawn as given (animate it before).
pub fn progress(ui: &mut Ui, width: f32, height: f32, fraction: f32, color: Color32) -> Rect {
  let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
  ui.painter().rect_filled(rect, CornerRadius::same(1), theme::SUNKEN);
  let filled = Rect::from_min_size(rect.min, vec2(rect.width() * fraction.clamp(0.0, 1.0), rect.height()));
  ui.painter().rect_filled(filled, CornerRadius::same(1), color);
  rect
}

/// A circular progress ring.
pub fn ring(painter: &egui::Painter, center: Pos2, radius: f32, width: f32, fraction: f32, color: Color32) {
  painter.circle_stroke(center, radius, Stroke::new(width, theme::SUNKEN));
  let fraction = fraction.clamp(0.0, 1.0);
  if fraction > 0.0 {
    let start = -std::f32::consts::FRAC_PI_2;
    let steps = (64.0 * fraction).ceil() as usize + 1;
    painter.add(Shape::line(icons::arc(center, radius, start, start + fraction * std::f32::consts::TAU, steps), Stroke::new(width, color)));
  }
}

/// A trophy tile: solid cup in the tier's color with a soft glow, or a lock when locked.
pub fn tier_badge(ui: &mut Ui, tier: Tier, unlocked: bool, size: f32) -> Rect {
  let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
  paint_tier_badge(ui.painter(), rect, tier, unlocked);
  rect
}

pub fn paint_tier_badge(p: &egui::Painter, rect: Rect, tier: Tier, unlocked: bool) {
  let color = theme::tier_color(tier);
  p.rect(
    rect,
    theme::RADIUS,
    theme::SUNKEN,
    Stroke::new(1.0, if unlocked { theme::fade(color, 0.6) } else { theme::LINE }),
    egui::StrokeKind::Inside,
  );
  if unlocked {
    for (r, a) in [(0.48, 0.10), (0.36, 0.16)] {
      p.circle_filled(rect.center(), rect.width() * r, theme::fade(color, a));
    }
    icons::trophy(p, rect.shrink(rect.width() * 0.2), color, true);
    if tier == Tier::Platinum {
      p.rect_stroke(rect.expand(2.0), CornerRadius::same(4), Stroke::new(1.0, theme::fade(color, 0.5)), egui::StrokeKind::Outside);
    }
  } else {
    icons::paint(p, icons::square(rect.center(), rect.width() * 0.42), Icon::Lock, theme::FAINT);
  }
}

/// A square hairline card.
pub fn card() -> Frame {
  Frame::NONE.fill(theme::CONTROLS).stroke(theme::hairline()).inner_margin(Margin::same(16))
}

/// A full-width row whose background lights up on hover. Returns the row's response.
pub fn hover_row(ui: &mut Ui, id: Id, add_contents: impl FnOnce(&mut Ui)) -> Response {
  let background = ui.painter().add(Shape::Noop);
  let inner = Frame::NONE.inner_margin(Margin::symmetric(14, 12)).show(ui, |ui| {
    ui.set_width(ui.available_width());
    add_contents(ui);
  });
  let rect = inner.response.rect;
  let response = ui.interact(rect, id, Sense::hover());
  let hover = ui.ctx().animate_bool_with_time(id, response.hovered(), 0.15);
  ui.painter().set(background, Shape::rect_filled(rect, CornerRadius::ZERO, lerp(theme::CONTROLS, Color32::from_rgb(40, 41, 46), hover)));
  ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, theme::hairline());
  response
}

pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
  a.lerp_to_gamma(b, t.clamp(0.0, 1.0))
}
