//! One game's achievements, filtered by state and tier.

use std::sync::Arc;

use archi_core::Tier;
use eframe::egui::Ui;

use super::{ArchiApp, StateFilter, theme, widgets};

const TIERS: [Option<Tier>; 5] = [None, Some(Tier::Bronze), Some(Tier::Silver), Some(Tier::Gold), Some(Tier::Platinum)];
const STATES: [StateFilter; 3] = [StateFilter::All, StateFilter::Unlocked, StateFilter::Locked];

impl ArchiApp {
  pub(super) fn achievements_screen(&mut self, ui: &mut Ui) {
    ui.label(theme::heading(self.tr("ui-nav-achievements"), 30.0));
    ui.add_space(16.0);
    let catalog = Arc::clone(&self.catalog);
    let Some(definition) = catalog.get(&self.shown_game) else { return };

    ui.horizontal_wrapped(|ui| {
      ui.spacing_mut().item_spacing = eframe::egui::vec2(12.0, 10.0);
      let games = catalog.all();
      let names: Vec<String> = games.iter().map(|g| self.tr(&g.game.name)).collect();
      let current = games.iter().position(|g| g.game.id == self.shown_game).unwrap_or(0);
      if let Some(i) = widgets::segmented(ui, &names, current) {
        self.shown_game = games[i].game.id.clone();
      }

      let states: Vec<String> = ["ui-filter-all", "ui-filter-unlocked", "ui-filter-locked"].iter().map(|t| self.tr(t)).collect();
      let current = STATES.iter().position(|s| *s == self.state_filter).unwrap_or(0);
      if let Some(i) = widgets::segmented(ui, &states, current) {
        self.state_filter = STATES[i];
      }

      let tiers: Vec<String> =
        TIERS.iter().map(|t| t.map_or_else(|| self.tr("ui-filter-any-tier"), |tier| self.tr(theme::tier_token(tier)))).collect();
      let current = TIERS.iter().position(|t| *t == self.tier_filter).unwrap_or(0);
      if let Some(i) = widgets::segmented(ui, &tiers, current) {
        self.tier_filter = TIERS[i];
      }
    });
    ui.add_space(14.0);

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
      return;
    }
    // A hairline on top closes the list like a table.
    let top = ui.cursor().top();
    ui.painter().hline(ui.max_rect().x_range(), top, theme::hairline());
    // Rows touch: their hairlines separate them.
    ui.scope(|ui| {
      ui.spacing_mut().item_spacing.y = 0.0;
      for achievement in shown {
        self.achievement_row(ui, game_id, achievement);
      }
    });
  }
}
