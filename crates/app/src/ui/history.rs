//! Every unlock, newest first.

use eframe::egui::Ui;

use super::{ArchiApp, theme};

impl ArchiApp {
  pub(super) fn history_screen(&mut self, ui: &mut Ui) {
    ui.label(theme::heading(self.tr("ui-nav-history"), 30.0));
    ui.add_space(16.0);
    if self.history.is_empty() {
      ui.label(theme::muted(self.tr("ui-nothing-yet")));
      return;
    }
    let top = ui.cursor().top();
    ui.painter().hline(ui.max_rect().x_range(), top, theme::hairline());
    // Rows touch: their hairlines separate them.
    ui.scope(|ui| {
      ui.spacing_mut().item_spacing.y = 0.0;
      for record in &self.history {
        self.unlock_row(ui, record);
      }
    });
  }
}
