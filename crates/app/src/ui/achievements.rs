//! One game's achievements, filtered by state, tier and text. Only the rows in view are laid
//! out; the rest just reserve their last known height, so long lists scroll smoothly.

use std::sync::Arc;

use archi_core::Tier;
use eframe::egui::{self, Rect, TextEdit, Ui, vec2};

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

      let hint = self.tr("ui-search");
      ui.add(TextEdit::singleline(&mut self.achievement_query).hint_text(hint).desired_width(220.0).margin(egui::vec2(10.0, 6.0)));
    });
    ui.add_space(14.0);

    let game_id = &definition.game.id;
    let query = fold(self.achievement_query.trim());
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
      // A secret is only found by what's shown for it, never by its real name.
      .filter(|a| {
        query.is_empty() || {
          let (name, description) = if a.hidden && self.unlocked_at(game_id, &a.id).is_none() {
            ("ui-hidden-name", "ui-hidden-desc")
          } else {
            (a.name.as_str(), a.description.as_str())
          };
          fold(&self.tr(name)).contains(&query) || fold(&self.tr(description)).contains(&query)
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
    // Heights depend on the width (descriptions wrap): measure again after a resize.
    let width = ui.available_width();
    if (width - self.row_width).abs() > 0.5 {
      self.row_heights.clear();
      self.row_width = width;
    }
    // Rows touch: their hairlines separate them.
    ui.scope(|ui| {
      ui.spacing_mut().item_spacing.y = 0.0;
      for achievement in shown {
        let key = format!("{game_id}/{}", achievement.id);
        if let Some(&height) = self.row_heights.get(&key) {
          let slot = Rect::from_min_size(ui.cursor().min, vec2(width, height));
          if !ui.is_rect_visible(slot) {
            ui.allocate_space(vec2(width, height));
            continue;
          }
        }
        let rect = self.achievement_row(ui, game_id, achievement);
        self.row_heights.insert(key, rect.height());
      }
    });
  }
}

/// Lowercase without accents, so "dano" finds "daño" and "angel" finds "Ángel".
fn fold(text: &str) -> String {
  text
    .to_lowercase()
    .chars()
    .map(|c| match c {
      'á' | 'à' | 'ä' | 'â' => 'a',
      'é' | 'è' | 'ë' | 'ê' => 'e',
      'í' | 'ì' | 'ï' | 'î' => 'i',
      'ó' | 'ò' | 'ö' | 'ô' => 'o',
      'ú' | 'ù' | 'ü' | 'û' => 'u',
      'ñ' => 'n',
      c => c,
    })
    .collect()
}

#[cfg(test)]
mod tests {
  #[test]
  fn search_ignores_case_and_accents() {
    assert_eq!(super::fold("Ángel de la GUARDA: daño"), "angel de la guarda: dano");
  }
}
