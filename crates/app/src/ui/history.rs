//! Every unlock, newest first.

use eframe::egui::Ui;

use super::{ArchiApp, theme};

impl ArchiApp {
  pub(super) fn history_screen(&mut self, ui: &mut Ui) {
    theme::heading(ui, self.tr("ui-nav-history"));
    if self.history.is_empty() {
      ui.label(theme::muted(self.tr("ui-nothing-yet")));
    }
    for record in &self.history {
      self.unlock_card(ui, record);
    }
  }
}
