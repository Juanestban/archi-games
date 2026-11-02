//! Level, XP bar, trophies by tier and the latest unlocks.

use archi_core::Tier;
use eframe::egui::{self, RichText, Ui};

use super::{ArchiApp, theme};

const RECENT: usize = 5;

impl ArchiApp {
  pub(super) fn profile_screen(&mut self, ui: &mut Ui) {
    theme::heading(ui, self.tr("ui-nav-profile"));
    let level = self.summary.level;

    theme::card().show(ui, |ui| {
      ui.set_width(ui.available_width());
      ui.label(RichText::new(self.tr_args("ui-level", &[("level", level.level.into())])).size(34.0).strong().color(theme::ACCENT));
      let fraction = level.xp_into_level as f32 / level.xp_for_next.max(1) as f32;
      let next = self.tr_args(
        "ui-xp-progress",
        &[("current", level.xp_into_level.into()), ("next", level.xp_for_next.into()), ("level", (level.level + 1).into())],
      );
      ui.add(egui::ProgressBar::new(fraction).text(next).fill(theme::ACCENT).desired_height(22.0));
      ui.add_space(4.0);
      ui.label(theme::muted(self.tr_args("ui-xp-total", &[("xp", self.summary.total_xp.into())])));
      ui.add_space(4.0);
      match &self.account {
        Some(account) => ui.label(theme::muted(self.tr_args("ui-account-linked", &[("name", account.display_name.as_str().into())]))),
        None => ui.label(RichText::new(self.tr("ui-account-none")).color(theme::DANGER)),
      };
    });
    ui.add_space(12.0);

    ui.columns(4, |columns| {
      for (column, tier) in columns.iter_mut().zip([Tier::Platinum, Tier::Gold, Tier::Silver, Tier::Bronze]) {
        let count = self.summary.by_tier.get(&tier).copied().unwrap_or(0);
        theme::card().show(column, |ui| {
          ui.set_width(ui.available_width());
          ui.vertical_centered(|ui| {
            theme::tier_badge(ui, tier, count > 0, 44.0);
            ui.label(RichText::new(count.to_string()).size(24.0).strong());
            ui.label(theme::muted(self.tr(theme::tier_token(tier))));
          });
        });
      }
    });
    ui.add_space(20.0);

    ui.label(RichText::new(self.tr("ui-recent")).size(18.0).strong());
    ui.add_space(8.0);
    if self.history.is_empty() {
      ui.label(theme::muted(self.tr("ui-nothing-yet")));
    }
    for record in self.history.iter().take(RECENT) {
      self.unlock_card(ui, record);
    }
  }
}
