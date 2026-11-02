//! Window chrome for the undecorated main window: dragging and maximizing from the title bar,
//! Windows-style caption buttons and resizing from the edges. Adapted from ArtCraft's
//! `ui/window.rs` (github.com/storytold/artcraft, MIT/Apache-2.0).

use eframe::egui::{self, Color32, Rect, Response, Sense, Stroke, Ui, vec2};

use super::theme;

pub const TITLE_BAR_HEIGHT: f32 = 40.0;
/// How far in from the window's edges a drag resizes.
const RESIZE_EDGE: f32 = 5.0;
/// Windows' own caption buttons are 46 px wide.
const BUTTON_WIDTH: f32 = 46.0;
const CLOSE_RED: Color32 = Color32::from_rgb(0xc4, 0x2b, 0x1c);

/// Dragging the title bar's empty space moves the window; double-clicking maximizes or
/// restores. Call before drawing the bar's controls so they stay on top.
pub fn title_bar_drag(ui: &mut Ui) {
  let r = ui.max_rect();
  let rect = Rect::from_min_max(r.min + vec2(RESIZE_EDGE, RESIZE_EDGE), r.max - vec2(RESIZE_EDGE, 0.0));
  let response = ui.interact(rect, ui.scope_id().with("title-bar-drag"), Sense::click_and_drag());
  if response.drag_started_by(egui::PointerButton::Primary) {
    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
  }
  if response.double_clicked() {
    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Caption {
  Minimize,
  Maximize,
  Restore,
  Close,
}

/// Minimize, maximize (or restore) and close, for a right-to-left layout.
pub fn caption_buttons(ui: &mut Ui) {
  let ctx = ui.ctx().clone();
  let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
  if caption_button(ui, Caption::Close).clicked() {
    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
  }
  let size = if maximized { Caption::Restore } else { Caption::Maximize };
  if caption_button(ui, size).clicked() {
    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
  }
  if caption_button(ui, Caption::Minimize).clicked() {
    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
  }
}

fn caption_button(ui: &mut Ui, button: Caption) -> Response {
  let (rect, _) = ui.allocate_exact_size(vec2(BUTTON_WIDTH, TITLE_BAR_HEIGHT), Sense::hover());
  // Clickable below the resize band along the top (and, for close, the right edge).
  let right = if button == Caption::Close { RESIZE_EDGE } else { 0.0 };
  let hit = Rect::from_min_max(rect.min + vec2(0.0, RESIZE_EDGE), rect.max - vec2(right, 0.0));
  let response = ui.interact(hit, ui.scope_id().with(("caption", button as u8)), Sense::click());
  let hover = ui.ctx().animate_bool_with_time(response.id, response.hovered(), 0.1);
  let (fill, ink) = match button {
    Caption::Close => (theme::fade(CLOSE_RED, hover), super::widgets::lerp(theme::MUTED, Color32::WHITE, hover)),
    _ => (theme::fade(theme::WASH_HOVER, hover), super::widgets::lerp(theme::MUTED, Color32::WHITE, hover)),
  };
  let p = ui.painter();
  p.rect_filled(rect, 0.0, fill);
  let c = rect.center();
  let stroke = Stroke::new(1.0, ink);
  let s = 5.0;
  match button {
    Caption::Minimize => {
      p.hline(c.x - s..=c.x + s, c.y, stroke);
    }
    Caption::Maximize => {
      p.rect_stroke(Rect::from_center_size(c, vec2(2.0 * s, 2.0 * s)), 0.0, stroke, egui::StrokeKind::Inside);
    }
    Caption::Restore => {
      let front = Rect::from_min_size(c + vec2(-s, -s + 2.0), vec2(2.0 * s - 2.0, 2.0 * s - 2.0));
      p.rect_stroke(front, 0.0, stroke, egui::StrokeKind::Inside);
      p.hline(c.x - s + 2.0..=c.x + s, c.y - s, stroke);
      p.vline(c.x + s, c.y - s..=c.y + s - 2.0, stroke);
    }
    Caption::Close => {
      p.line_segment([c + vec2(-s, -s), c + vec2(s, s)], stroke);
      p.line_segment([c + vec2(-s, s), c + vec2(s, -s)], stroke);
    }
  }
  response
}

/// A hairline round the window, and resizing from its edges and corners (when not maximized).
pub fn window_edges(ctx: &egui::Context) {
  use egui::CursorIcon as C;
  use egui::viewport::ResizeDirection as D;
  if ctx.input(|i| i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)) {
    return;
  }
  let r = ctx.content_rect();
  ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("window-border"))).rect_stroke(
    r,
    0.0,
    theme::hairline(),
    egui::StrokeKind::Inside,
  );
  let Some(p) = ctx.input(|i| i.pointer.hover_pos()).filter(|p| r.contains(*p)) else { return };
  let near = |d: f32| d < RESIZE_EDGE;
  let corner = |d: f32| d < 3.0 * RESIZE_EDGE;
  let (left, right, top, bottom) = (p.x - r.left(), r.right() - p.x, p.y - r.top(), r.bottom() - p.y);
  let hit = if (near(top) && corner(left)) || (near(left) && corner(top)) {
    Some((D::NorthWest, C::ResizeNorthWest))
  } else if (near(top) && corner(right)) || (near(right) && corner(top)) {
    Some((D::NorthEast, C::ResizeNorthEast))
  } else if (near(bottom) && corner(left)) || (near(left) && corner(bottom)) {
    Some((D::SouthWest, C::ResizeSouthWest))
  } else if (near(bottom) && corner(right)) || (near(right) && corner(bottom)) {
    Some((D::SouthEast, C::ResizeSouthEast))
  } else if near(top) {
    Some((D::North, C::ResizeNorth))
  } else if near(bottom) {
    Some((D::South, C::ResizeSouth))
  } else if near(left) {
    Some((D::West, C::ResizeWest))
  } else if near(right) {
    Some((D::East, C::ResizeEast))
  } else {
    None
  };
  if let Some((direction, cursor)) = hit {
    ctx.set_cursor_icon(cursor);
    if ctx.input(|i| i.pointer.primary_pressed()) {
      ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
    }
  }
}
