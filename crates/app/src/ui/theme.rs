//! Design tokens. The base is ArtCraft's brutalist system (github.com/storytold/artcraft,
//! MIT/Apache-2.0): near-black surfaces, white/15 hairlines, Archivo display type, Inter body
//! text, Geist Mono labels and 3 px corners on anything pressable; large surfaces stay square.
//! On top of it: the trophy tier colors.

use archi_core::Tier;
use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, RichText, Stroke, TextStyle, Visuals};

const fn hex(rgb: u32) -> Color32 {
  let [_, r, g, b] = rgb.to_be_bytes();
  Color32::from_rgb(r, g, b)
}

const fn alpha(rgb: u32, a: u8) -> Color32 {
  let [_, r, g, b] = rgb.to_be_bytes();
  Color32::from_rgba_unmultiplied_const(r, g, b, a)
}

/// The content panel.
pub const BG: Color32 = hex(0x121316);
/// Notifications, popups.
pub const PANEL: Color32 = hex(0x101014);
/// Cards, rows, buttons.
pub const CONTROLS: Color32 = hex(0x1e1f22);
/// The window chrome: title bar and sidebar.
pub const CHROME: Color32 = hex(0x0a0a0c);
/// Inputs and progress wells.
pub const SUNKEN: Color32 = hex(0x060607);

pub const INK: Color32 = hex(0xf2f1ee);
pub const MUTED: Color32 = alpha(0xf2f1ee, 153);
pub const FAINT: Color32 = alpha(0xf2f1ee, 102);
pub const SIDEBAR_INK: Color32 = alpha(0xffffff, 184);

/// `white/15`: every hairline.
pub const LINE: Color32 = alpha(0xffffff, 38);
pub const LINE_STRONG: Color32 = alpha(0xf2f1ee, 102);
/// `white/5`, `white/8`, `white/10`: quiet, selected and hovered fills.
pub const WASH: Color32 = alpha(0xffffff, 13);
pub const WASH_SELECTED: Color32 = alpha(0xffffff, 20);
pub const WASH_HOVER: Color32 = alpha(0xffffff, 26);

pub const ACCENT: Color32 = hex(0x2d81ff);
pub const ACCENT_400: Color32 = hex(0x59a7ff);
pub const ACCENT_INK: Color32 = hex(0x74aaff);

pub const BAD: Color32 = hex(0xf87171);
pub const GOOD: Color32 = hex(0x4ade80);
/// Something is off but retried on its own.
pub const WARN: Color32 = hex(0xfbbf24);

/// 3 px: buttons, inputs, segmented controls.
pub const RADIUS: CornerRadius = CornerRadius::same(3);
pub const CONTROL_H: f32 = 34.0;

/// Mono upper-case labels: 11 px, tracked 0.15 em apart (see `widgets::caps`).
pub const HUD_SIZE: f32 = 11.0;

pub fn hairline() -> Stroke {
  Stroke::new(1.0, LINE)
}

/// `c` at `a` (0–1) of its opacity.
pub fn fade(c: Color32, a: f32) -> Color32 {
  c.gamma_multiply(a.clamp(0.0, 1.0))
}

pub fn shadow() -> egui::epaint::Shadow {
  egui::epaint::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(140) }
}

pub fn tier_color(tier: Tier) -> Color32 {
  match tier {
    Tier::Bronze => hex(0xd08a4e),
    Tier::Silver => hex(0xc9d1dc),
    Tier::Gold => hex(0xf5c542),
    Tier::Platinum => hex(0x9fd8ff),
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

/// Archivo, wide and heavy: headings and big numbers.
pub fn display(size: f32) -> FontId {
  FontId::new(size, FontFamily::Name("display".into()))
}

pub fn mono(size: f32) -> FontId {
  FontId::new(size, FontFamily::Monospace)
}

/// Heading text in Archivo 620 at 118 % width.
pub fn heading(text: impl Into<String>, size: f32) -> RichText {
  RichText::new(text).font(display(size)).variation("wght", 620.0).variation("wdth", 118.0).color(INK)
}

/// Inter at a weight (400 regular, 500 medium, 600 semibold).
pub fn body(text: impl Into<String>, size: f32, weight: f32) -> RichText {
  RichText::new(text).size(size).variation("wght", weight).variation("opsz", 14.0)
}

pub fn muted(text: impl Into<String>) -> RichText {
  RichText::new(text).color(MUTED)
}

/// Dark only, like ArtCraft.
pub fn apply(ctx: &egui::Context) {
  install_fonts(ctx);
  ctx.set_visuals_of(egui::Theme::Dark, visuals());
  ctx.set_theme(egui::ThemePreference::Dark);
  ctx.all_styles_mut(|style| {
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size.y = CONTROL_H;
    style.spacing.slider_rail_height = 4.0;
    style.spacing.scroll = egui::style::ScrollStyle { bar_width: 8.0, floating: true, ..egui::style::ScrollStyle::thin() };
    style.text_styles = [
      (TextStyle::Heading, display(24.0)),
      (TextStyle::Body, FontId::proportional(14.0)),
      (TextStyle::Button, FontId::proportional(14.0)),
      (TextStyle::Small, FontId::proportional(12.0)),
      (TextStyle::Monospace, mono(13.0)),
    ]
    .into();
    style.animation_time = 0.15;
  });
}

fn visuals() -> Visuals {
  let mut v = Visuals::dark();
  v.panel_fill = BG;
  v.window_fill = PANEL;
  v.extreme_bg_color = SUNKEN;
  v.faint_bg_color = CONTROLS;
  v.window_stroke = hairline();
  v.window_corner_radius = RADIUS;
  v.menu_corner_radius = RADIUS;
  v.window_shadow = shadow();
  v.popup_shadow = shadow();
  v.override_text_color = Some(INK);
  v.hyperlink_color = ACCENT_INK;
  v.selection.bg_fill = ACCENT;
  v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
  v.text_cursor.stroke = Stroke::new(1.5, Color32::WHITE);
  v.error_fg_color = BAD;
  for (w, fill, stroke, fg) in [
    (&mut v.widgets.noninteractive, CONTROLS, LINE, MUTED),
    (&mut v.widgets.inactive, CONTROLS, LINE, INK),
    (&mut v.widgets.hovered, WASH_HOVER, LINE_STRONG, Color32::WHITE),
    (&mut v.widgets.active, WASH_HOVER, LINE_STRONG, Color32::WHITE),
    (&mut v.widgets.open, WASH_HOVER, LINE_STRONG, Color32::WHITE),
  ] {
    w.bg_fill = fill;
    w.weak_bg_fill = fill;
    w.bg_stroke = Stroke::new(1.0, stroke);
    w.corner_radius = RADIUS;
    w.fg_stroke = Stroke::new(1.0, fg);
    w.expansion = 0.0;
  }
  v
}

/// Inter (body), Archivo (display) and Geist Mono (labels), all SIL OFL 1.1 (see
/// `assets/fonts/OFL-*.txt`). egui's own fonts stay behind them for symbols they lack.
fn install_fonts(ctx: &egui::Context) {
  const INTER: &[u8] = include_bytes!("../../assets/fonts/Inter-Variable.ttf");
  const ARCHIVO: &[u8] = include_bytes!("../../assets/fonts/Archivo-Variable.ttf");
  const GEIST_MONO: &[u8] = include_bytes!("../../assets/fonts/GeistMono-Variable.ttf");

  let mut fonts = FontDefinitions::default();
  let fallback = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
  for (name, bytes, family) in [
    ("Inter", INTER, FontFamily::Proportional),
    ("Archivo", ARCHIVO, FontFamily::Name("display".into())),
    ("GeistMono", GEIST_MONO, FontFamily::Monospace),
  ] {
    fonts.font_data.insert(name.to_owned(), std::sync::Arc::new(FontData::from_static(bytes)));
    let mut stack = vec![name.to_owned()];
    stack.extend(fallback.iter().cloned());
    fonts.families.insert(family, stack);
  }
  ctx.set_fonts(fonts);
}
