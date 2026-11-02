//! Each game as a tile: completion ring, counts and platinum.

use std::sync::Arc;

use archi_core::Tier;
use eframe::egui::{Sense, Ui, vec2};

use super::{ArchiApp, FILL_IN, Screen, theme, widgets};

impl ArchiApp {
  pub(super) fn games_screen(&mut self, ui: &mut Ui) {
    ui.label(theme::heading(self.tr("ui-nav-games"), 30.0));
    ui.add_space(16.0);
    let catalog = Arc::clone(&self.catalog);
    let fill = self.page_t(ui, FILL_IN);

    for definition in catalog.games() {
      let (unlocked, total) = self.completion(definition);
      let fraction = unlocked as f32 / total.max(1) as f32;
      let platinum = definition.platinum().is_some_and(|p| self.unlocked_at(&definition.game.id, &p.id).is_some());
      let mut open = false;
      widgets::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
          let (rect, _) = ui.allocate_exact_size(vec2(96.0, 96.0), Sense::hover());
          widgets::ring(
            ui.painter(),
            rect.center(),
            40.0,
            5.0,
            fraction * fill,
            if platinum { theme::tier_color(Tier::Platinum) } else { theme::ACCENT },
          );
          let percent = (fraction * fill * 100.0).round() as u32;
          let label = widgets::display_text(ui, &format!("{percent}%"), 20.0, theme::INK);
          ui.painter().galley(rect.center() - label.size() / 2.0, label, theme::INK);

          ui.add_space(18.0);
          ui.vertical(|ui| {
            ui.add_space(10.0);
            ui.label(theme::heading(self.tr(&definition.game.name), 24.0));
            ui.add_space(4.0);
            widgets::hud(ui, &self.tr_args("ui-unlocked-count", &[("unlocked", unlocked.into()), ("total", total.into())]), theme::MUTED);
            ui.add_space(6.0);
            if platinum {
              widgets::hud(ui, &self.tr("ui-platinum-earned"), theme::tier_color(Tier::Platinum));
            } else {
              widgets::hud(ui, &self.tr("ui-platinum-pending"), theme::FAINT);
            }
          });
          ui.with_layout(eframe::egui::Layout::right_to_left(eframe::egui::Align::Center), |ui| {
            open = widgets::button(ui, &self.tr("ui-view-achievements"), widgets::Kind::Primary).clicked();
          });
        });
      });
      if open {
        self.shown_game = definition.game.id.clone();
        self.go(ui, Screen::Achievements);
      }
      ui.add_space(12.0);
    }
  }
}
