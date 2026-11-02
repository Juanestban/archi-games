//! Language, API key, Epic account, tracking status and sound.

use eframe::egui::{self, RichText, TextEdit, Ui};

use super::{ArchiApp, theme, widgets};
use crate::i18n::{Language, LanguageChoice};
use crate::sound;
use crate::update;
use crate::worker::Command;

const LANGUAGES: [LanguageChoice; 3] = [LanguageChoice::System, LanguageChoice::Fixed(Language::Es), LanguageChoice::Fixed(Language::En)];

impl ArchiApp {
  pub(super) fn settings_screen(&mut self, ui: &mut Ui) {
    ui.label(theme::heading(self.tr("ui-nav-settings"), 30.0));
    ui.add_space(16.0);
    self.account_settings(ui);
    self.api_key_settings(ui);
    self.tracking_settings(ui);
    self.sound_settings(ui);
    self.system_settings(ui);
    self.language_settings(ui);
    self.about_settings(ui);
  }

  fn about_settings(&mut self, ui: &mut Ui) {
    let version = self.tr_args("ui-version", &[("version", env!("CARGO_PKG_VERSION").into())]);
    let license = self.tr("ui-license");
    let auto_label = self.tr("ui-update-auto");
    let help = self.tr("ui-update-help");
    let busy = matches!(self.update, super::UpdateUi::Checking | super::UpdateUi::Downloading(..) | super::UpdateUi::Installing);
    let check = self.tr("ui-update-check");
    let state = match &self.update {
      super::UpdateUi::Checking => Some((self.tr("ui-update-checking"), theme::MUTED)),
      super::UpdateUi::UpToDate => Some((self.tr("ui-update-latest"), theme::GOOD)),
      super::UpdateUi::Available(info) => {
        Some((self.tr_args("ui-update-available", &[("version", info.version.as_str().into())]), theme::ACCENT_INK))
      }
      super::UpdateUi::Failed(_, problem) => Some((self.tr(problem.token()), theme::BAD)),
      _ => None,
    };
    let mut auto = self.auto_update;
    let mut check_clicked = false;
    block(ui, &self.tr("ui-settings-about"), |ui| {
      ui.label(theme::heading(format!("Archi Games · {version}"), 18.0));
      widgets::hud(ui, &license, theme::MUTED);
      ui.add_space(8.0);
      ui.horizontal(|ui| {
        widgets::toggle(ui, &mut auto);
        ui.label(theme::body(auto_label, 14.0, 500.0));
      });
      ui.label(theme::muted(help));
      ui.add_space(4.0);
      ui.horizontal(|ui| {
        ui.add_enabled_ui(!busy, |ui| check_clicked = widgets::button(ui, &check, widgets::Kind::Secondary).clicked());
        if let Some((text, color)) = state {
          ui.label(RichText::new(text).color(color));
        }
      });
    });
    if auto != self.auto_update {
      self.auto_update = auto;
      // Not worth interrupting over: the choice still applies this session.
      let _ = self.storage.set_setting(update::AUTO_CHECK_SETTING, if auto { "true" } else { "false" });
    }
    if check_clicked {
      self.update = super::UpdateUi::Checking;
      self.worker.send(Command::CheckUpdate { manual: true });
    }
  }

  fn system_settings(&mut self, ui: &mut Ui) {
    let label = self.tr("ui-autostart");
    let help = self.tr("ui-autostart-help");
    let failed = self.autostart_failed.then(|| self.tr("ui-error-autostart"));
    let mut enabled = self.autostart;
    block(ui, &self.tr("ui-settings-system"), |ui| {
      ui.horizontal(|ui| {
        widgets::toggle(ui, &mut enabled);
        ui.label(theme::body(label, 14.0, 500.0));
      });
      ui.label(theme::muted(help));
      if let Some(failed) = failed {
        ui.label(RichText::new(failed).color(theme::BAD));
      }
    });
    if enabled != self.autostart {
      self.set_autostart(enabled);
    }
  }

  fn language_settings(&mut self, ui: &mut Ui) {
    // Language names stay in their own language.
    let names: Vec<String> = LANGUAGES
      .iter()
      .map(|choice| match choice {
        LanguageChoice::System => self.tr("ui-language-system"),
        LanguageChoice::Fixed(Language::Es) => "Español".to_owned(),
        LanguageChoice::Fixed(Language::En) => "English".to_owned(),
      })
      .collect();
    let current = LANGUAGES.iter().position(|c| *c == self.language).unwrap_or(0);
    let mut picked = None;
    block(ui, &self.tr("ui-settings-language"), |ui| picked = widgets::segmented(ui, &names, current));
    if let Some(i) = picked {
      let choice = LANGUAGES[i];
      self.language = choice;
      self.i18n.set_language(choice.resolve());
      if let Some(tray) = &self.tray {
        tray.set_labels(&self.tray_labels());
      }
      // Not worth interrupting over: the choice still applies this session.
      let _ = self.storage.set_setting(LanguageChoice::SETTING, choice.code());
    }
  }

  fn api_key_settings(&mut self, ui: &mut Ui) {
    let help = self.tr("ui-settings-api-key-help");
    let (state, color) = if self.status.has_api_key {
      (self.tr("ui-settings-api-key-saved"), theme::GOOD)
    } else {
      (self.tr("ui-settings-api-key-missing"), theme::BAD)
    };
    let save = self.tr("ui-save");
    block(ui, &self.tr("ui-settings-api-key"), |ui| {
      ui.label(theme::muted(help));
      widgets::hud(ui, &state, color);
      ui.add_space(4.0);
      ui.horizontal(|ui| {
        ui.add(TextEdit::singleline(&mut self.api_key_input).password(true).desired_width(360.0).margin(egui::vec2(10.0, 8.0)));
        ui.add_enabled_ui(!self.api_key_input.trim().is_empty(), |ui| {
          if widgets::button(ui, &save, widgets::Kind::Primary).clicked() {
            self.worker.send(Command::SetApiKey(std::mem::take(&mut self.api_key_input)));
          }
        });
      });
    });
  }

  fn account_settings(&mut self, ui: &mut Ui) {
    let linked = self.account.as_ref().map(|account| account.display_name.clone());
    let hint = self.tr("ui-settings-epic-name");
    let link = self.tr("ui-link");
    let result = self.link_result.map(|result| match result {
      Ok(()) => (self.tr("ui-linked-ok"), theme::GOOD),
      Err(problem) => (self.tr(problem.token()), theme::BAD),
    });
    block(ui, &self.tr("ui-settings-account"), |ui| {
      if let Some(linked) = linked {
        ui.label(theme::heading(linked, 18.0));
      }
      ui.horizontal(|ui| {
        ui.add(TextEdit::singleline(&mut self.epic_name_input).hint_text(hint).desired_width(260.0).margin(egui::vec2(10.0, 8.0)));
        let enabled = !self.status.busy && !self.epic_name_input.trim().is_empty();
        ui.add_enabled_ui(enabled, |ui| {
          if widgets::button(ui, &link, widgets::Kind::Secondary).clicked() {
            self.link_result = None;
            self.worker.send(Command::Link(self.epic_name_input.clone()));
          }
        });
      });
      if let Some((text, color)) = result {
        ui.label(RichText::new(text).color(color));
      }
    });
  }

  fn tracking_settings(&mut self, ui: &mut Ui) {
    let game = self.tr(&self.catalog.fortnite.game.name);
    let running = self.tr_args(if self.status.game_running { "ui-game-running" } else { "ui-game-closed" }, &[("game", game.into())]);
    let when = self.status.last_poll.map_or_else(|| self.tr("ui-never"), |at| self.when(at));
    let last_poll = self.tr_args("ui-last-poll", &[("when", when.into())]);
    let problem = self.status.problem.map(|problem| self.tr(problem.token()));
    let replays = if self.status.replays > 0 {
      (self.tr_args("ui-replays-count", &[("count", self.status.replays.into())]), theme::MUTED)
    } else {
      (self.tr("ui-replays-none"), theme::FAINT)
    };
    let button = if self.status.busy { self.tr("ui-busy") } else { self.tr("ui-poll-now") };
    let can_poll = !self.status.busy && self.status.has_api_key && self.account.is_some();
    block(ui, &self.tr("ui-settings-tracking"), |ui| {
      ui.label(running);
      widgets::hud(ui, &last_poll, theme::MUTED);
      ui.label(RichText::new(replays.0).color(replays.1));
      if let Some(problem) = problem {
        ui.label(RichText::new(problem).color(theme::BAD));
      }
      ui.add_space(4.0);
      ui.add_enabled_ui(can_poll, |ui| {
        if widgets::button(ui, &button, widgets::Kind::Secondary).clicked() {
          self.worker.send(Command::PollNow);
        }
      });
    });
  }

  fn sound_settings(&mut self, ui: &mut Ui) {
    let help = self.tr("ui-settings-sound-help");
    let volume_label = self.tr("ui-volume");
    let test = self.tr("ui-notification-test");
    let mut volume = self.volume;
    let mut test_clicked = false;
    block(ui, &self.tr("ui-settings-sound"), |ui| {
      ui.label(theme::muted(help));
      ui.horizontal(|ui| {
        widgets::hud(ui, &volume_label, theme::MUTED);
        ui.add(egui::Slider::new(&mut volume, 0..=100).trailing_fill(true));
      });
      ui.add_space(4.0);
      test_clicked = widgets::button(ui, &test, widgets::Kind::Accent).clicked();
    });
    if volume != self.volume {
      self.volume = volume;
      // Not worth interrupting over: the volume still applies this session.
      let _ = self.storage.set_setting(sound::VOLUME_SETTING, &volume.to_string());
    }
    if test_clicked {
      self.test_notifications();
    }
  }
}

/// A settings block: a section label over a hairline card.
fn block(ui: &mut Ui, title: &str, add_contents: impl FnOnce(&mut Ui)) {
  widgets::section(ui, title);
  widgets::card().show(ui, |ui| {
    ui.set_width(ui.available_width());
    add_contents(ui);
  });
  ui.add_space(18.0);
}
