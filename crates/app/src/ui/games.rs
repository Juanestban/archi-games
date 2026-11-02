//! Each game's completion and platinum.

use std::sync::Arc;

use eframe::egui::{self, RichText, Ui};

use super::{ArchiApp, Screen, theme};

impl ArchiApp {
  pub(super) fn games_screen(&mut self, ui: &mut Ui) {
    theme::heading(ui, self.tr("ui-nav-games"));
    let catalog = Arc::clone(&self.catalog);

    for definition in catalog.games() {
      let (unlocked, total) = self.completion(definition);
      let platinum = definition.platinum().is_some_and(|p| self.unlocked_at(&definition.game.id, &p.id).is_some());
      theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
          ui.label(RichText::new(self.tr(&definition.game.name)).size(22.0).strong());
          if platinum {
            ui.add_space(8.0);
            ui.label(RichText::new(self.tr("ui-platinum-earned")).color(theme::tier_color(archi_core::Tier::Platinum)));
          }
        });
        ui.add_space(6.0);
        let fraction = unlocked as f32 / total.max(1) as f32;
        let percent = (fraction * 100.0).round() as u32;
        ui.add(egui::ProgressBar::new(fraction).text(self.tr_args("ui-completion", &[("percent", percent.into())])).fill(theme::ACCENT));
        ui.label(theme::muted(self.tr_args("ui-unlocked-count", &[("unlocked", unlocked.into()), ("total", total.into())])));
        ui.add_space(6.0);
        if ui.button(self.tr("ui-view-achievements")).clicked() {
          self.shown_game = definition.game.id.clone();
          self.screen = Screen::Achievements;
        }
      });
      ui.add_space(10.0);
    }
  }
}
