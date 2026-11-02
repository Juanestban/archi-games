//! One game's achievements, filtered by state and tier.

use std::sync::Arc;

use archi_core::Tier;
use eframe::egui::{self, Ui};

use super::{ArchiApp, StateFilter, theme};

impl ArchiApp {
  pub(super) fn achievements_screen(&mut self, ui: &mut Ui) {
    theme::heading(ui, self.tr("ui-nav-achievements"));
    let catalog = Arc::clone(&self.catalog);
    let Some(definition) = catalog.get(&self.shown_game) else { return };

    ui.horizontal(|ui| {
      egui::ComboBox::from_id_salt("game").selected_text(self.tr(&definition.game.name)).show_ui(ui, |ui| {
        for game in catalog.all() {
          let name = self.tr(&game.game.name);
          ui.selectable_value(&mut self.shown_game, game.game.id.clone(), name);
        }
      });
      ui.add_space(12.0);
      for (filter, token) in
        [(StateFilter::All, "ui-filter-all"), (StateFilter::Unlocked, "ui-filter-unlocked"), (StateFilter::Locked, "ui-filter-locked")]
      {
        let label = self.tr(token);
        ui.selectable_value(&mut self.state_filter, filter, label);
      }
      ui.add_space(12.0);
      let tier_label = self.tier_filter.map_or_else(|| self.tr("ui-filter-any-tier"), |tier| self.tr(theme::tier_token(tier)));
      egui::ComboBox::from_id_salt("tier").selected_text(tier_label).show_ui(ui, |ui| {
        let any = self.tr("ui-filter-any-tier");
        ui.selectable_value(&mut self.tier_filter, None, any);
        for tier in [Tier::Bronze, Tier::Silver, Tier::Gold, Tier::Platinum] {
          let name = self.tr(theme::tier_token(tier));
          ui.selectable_value(&mut self.tier_filter, Some(tier), name);
        }
      });
    });
    ui.add_space(12.0);

    let game_id = &definition.game.id;
    let shown: Vec<_> = definition
      .achievements
      .iter()
      .filter(|a| self.tier_filter.is_none_or(|tier| a.tier == tier))
      .filter(|a| {
        let unlocked = self.unlocked_at(game_id, &a.id).is_some();
        match self.state_filter {
          StateFilter::All => true,
          StateFilter::Unlocked => unlocked,
          StateFilter::Locked => !unlocked,
        }
      })
      .collect();
    if shown.is_empty() {
      ui.label(theme::muted(self.tr("ui-no-matches")));
    }
    for achievement in shown {
      self.achievement_card(ui, game_id, achievement);
    }
  }
}
