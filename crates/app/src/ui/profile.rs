//! Level ring, XP, trophies by tier and the latest unlocks.

use archi_core::Tier;
use eframe::egui::{RichText, Sense, Ui, vec2};

use super::{ArchiApp, FILL_IN, theme, widgets};

const RECENT: usize = 5;

impl ArchiApp {
  pub(super) fn profile_screen(&mut self, ui: &mut Ui) {
    ui.label(theme::heading(self.tr("ui-nav-profile"), 30.0));
    ui.add_space(16.0);
    let fill = self.page_t(ui, FILL_IN);
    let level = self.summary.level;
    let fraction = level.xp_into_level as f32 / level.xp_for_next.max(1) as f32;

    widgets::card().show(ui, |ui| {
      ui.set_width(ui.available_width());
      ui.horizontal(|ui| {
        // The level inside a ring that fills with this level's XP.
        let (rect, _) = ui.allocate_exact_size(vec2(132.0, 132.0), Sense::hover());
        widgets::ring(ui.painter(), rect.center(), 58.0, 6.0, fraction * fill, theme::ACCENT);
        let label = widgets::caps(ui, &self.tr("ui-level-label"), 10.0, theme::MUTED);
        ui.painter().galley(rect.center() - vec2(label.size().x / 2.0, 34.0), label, theme::MUTED);
        let number = widgets::display_text(ui, &level.level.to_string(), 44.0, theme::INK);
        ui.painter().galley(rect.center() + vec2(-number.size().x / 2.0, 6.0 - number.size().y / 2.0), number, theme::INK);

        ui.add_space(24.0);
        ui.vertical(|ui| {
          ui.add_space(14.0);
          match self.display_name() {
            Some(name) => ui.label(theme::heading(name, 22.0)),
            None => ui.label(RichText::new(self.tr("ui-account-none")).color(theme::BAD)),
          };
          ui.add_space(10.0);
          widgets::hud(ui, "XP", theme::MUTED);
          let width = ui.available_width().min(460.0);
          widgets::progress(ui, width, 6.0, fraction * fill, theme::ACCENT);
          let next = self.tr_args(
            "ui-xp-progress",
            &[("current", level.xp_into_level.into()), ("next", level.xp_for_next.into()), ("level", (level.level + 1).into())],
          );
          ui.label(RichText::new(next).font(theme::mono(12.0)).color(theme::MUTED));
          ui.label(RichText::new(self.tr_args("ui-xp-total", &[("xp", self.summary.total_xp.into())])).font(theme::mono(12.0)).color(theme::FAINT));
        });
      });
    });
    ui.add_space(14.0);

    ui.columns(4, |columns| {
      for (column, tier) in columns.iter_mut().zip([Tier::Platinum, Tier::Gold, Tier::Silver, Tier::Bronze]) {
        let count = self.summary.by_tier.get(&tier).copied().unwrap_or(0);
        widgets::card().show(column, |ui| {
          ui.set_width(ui.available_width());
          ui.horizontal(|ui| {
            widgets::tier_badge(ui, tier, count > 0, 44.0);
            ui.add_space(6.0);
            ui.vertical(|ui| {
              ui.label(RichText::new(count.to_string()).font(theme::display(26.0)).color(theme::INK));
              widgets::hud(ui, &self.tr(theme::tier_token(tier)), theme::fade(theme::tier_color(tier), 0.85));
            });
          });
        });
      }
    });
    ui.add_space(22.0);

    widgets::section(ui, &self.tr("ui-recent"));
    if self.history.is_empty() {
      ui.label(theme::muted(self.tr("ui-nothing-yet")));
    }
    // Rows touch: their hairlines separate them.
    ui.scope(|ui| {
      ui.spacing_mut().item_spacing.y = 0.0;
      for record in self.history.iter().take(RECENT) {
        self.unlock_row(ui, record);
      }
    });
  }
}
