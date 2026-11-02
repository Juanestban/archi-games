//! Language, API key, Epic account and tracking status.

use eframe::egui::{self, RichText, TextEdit, Ui};

use super::{ArchiApp, theme};
use crate::i18n::{Language, LanguageChoice};
use crate::worker::Command;

impl ArchiApp {
  pub(super) fn settings_screen(&mut self, ui: &mut Ui) {
    theme::heading(ui, self.tr("ui-nav-settings"));
    self.language_card(ui);
    self.api_key_card(ui);
    self.account_card(ui);
    self.tracking_card(ui);
  }

  fn language_card(&mut self, ui: &mut Ui) {
    let system = self.tr("ui-language-system");
    // Language names stay in their own language.
    let label = |choice: LanguageChoice| match choice {
      LanguageChoice::System => system.clone(),
      LanguageChoice::Fixed(Language::Es) => "Español".to_owned(),
      LanguageChoice::Fixed(Language::En) => "English".to_owned(),
    };
    let mut choice = self.language;
    section(ui, &self.tr("ui-settings-language"), |ui| {
      egui::ComboBox::from_id_salt("language").selected_text(label(choice)).show_ui(ui, |ui| {
        for option in [LanguageChoice::System, LanguageChoice::Fixed(Language::Es), LanguageChoice::Fixed(Language::En)] {
          ui.selectable_value(&mut choice, option, label(option));
        }
      });
    });
    if choice != self.language {
      self.language = choice;
      self.i18n.set_language(choice.resolve());
      // Not worth interrupting over: the choice still applies this session.
      let _ = self.storage.set_setting(LanguageChoice::SETTING, choice.code());
    }
  }

  fn api_key_card(&mut self, ui: &mut Ui) {
    let title = self.tr("ui-settings-api-key");
    let help = self.tr("ui-settings-api-key-help");
    let state = if self.status.has_api_key {
      RichText::new(self.tr("ui-settings-api-key-saved")).color(theme::SUCCESS)
    } else {
      RichText::new(self.tr("ui-settings-api-key-missing")).color(theme::DANGER)
    };
    let save = self.tr("ui-save");
    section(ui, &title, |ui| {
      ui.label(theme::muted(help));
      ui.label(state);
      ui.horizontal(|ui| {
        ui.add(TextEdit::singleline(&mut self.api_key_input).password(true).desired_width(360.0));
        if ui.add_enabled(!self.api_key_input.trim().is_empty(), egui::Button::new(save)).clicked() {
          self.worker.send(Command::SetApiKey(std::mem::take(&mut self.api_key_input)));
        }
      });
    });
  }

  fn account_card(&mut self, ui: &mut Ui) {
    let title = self.tr("ui-settings-account");
    let linked = self.account.as_ref().map(|account| self.tr_args("ui-account-linked", &[("name", account.display_name.as_str().into())]));
    let hint = self.tr("ui-settings-epic-name");
    let link = self.tr("ui-link");
    let result = self.link_result.map(|result| match result {
      Ok(()) => RichText::new(self.tr("ui-linked-ok")).color(theme::SUCCESS),
      Err(problem) => RichText::new(self.tr(problem.token())).color(theme::DANGER),
    });
    section(ui, &title, |ui| {
      if let Some(linked) = linked {
        ui.label(linked);
      }
      ui.horizontal(|ui| {
        ui.add(TextEdit::singleline(&mut self.epic_name_input).hint_text(hint).desired_width(260.0));
        let enabled = !self.status.busy && !self.epic_name_input.trim().is_empty();
        if ui.add_enabled(enabled, egui::Button::new(link)).clicked() {
          self.link_result = None;
          self.worker.send(Command::Link(self.epic_name_input.clone()));
        }
      });
      if let Some(result) = result {
        ui.label(result);
      }
    });
  }

  fn tracking_card(&mut self, ui: &mut Ui) {
    let game = self.tr(&self.catalog.fortnite.game.name);
    let running = self.tr_args(if self.status.game_running { "ui-game-running" } else { "ui-game-closed" }, &[("game", game.into())]);
    let when = self.status.last_poll.map_or_else(|| self.tr("ui-never"), |at| self.when(at));
    let last_poll = self.tr_args("ui-last-poll", &[("when", when.into())]);
    let problem = self.status.problem.map(|problem| self.tr(problem.token()));
    let button = if self.status.busy { self.tr("ui-busy") } else { self.tr("ui-poll-now") };
    let can_poll = !self.status.busy && self.status.has_api_key && self.account.is_some();
    section(ui, &self.tr("ui-settings-tracking"), |ui| {
      ui.label(running);
      ui.label(theme::muted(last_poll));
      if let Some(problem) = problem {
        ui.label(RichText::new(problem).color(theme::DANGER));
      }
      if ui.add_enabled(can_poll, egui::Button::new(button)).clicked() {
        self.worker.send(Command::PollNow);
      }
    });
  }
}

fn section(ui: &mut Ui, title: &str, add_contents: impl FnOnce(&mut Ui)) {
  theme::card().show(ui, |ui| {
    ui.set_width(ui.available_width());
    ui.label(RichText::new(title).size(17.0).strong());
    ui.add_space(6.0);
    add_contents(ui);
  });
  ui.add_space(10.0);
}
