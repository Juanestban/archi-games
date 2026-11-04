//! Per game (account, API key, tracking), then sound, system, language and updates.

use eframe::egui::{self, RichText, TextEdit, Ui};

use super::{ArchiApp, theme, widgets};
use crate::i18n::{Language, LanguageChoice};
use crate::sound;
use crate::update;
use crate::worker::{Command, Integration};

const LANGUAGES: [LanguageChoice; 3] = [LanguageChoice::System, LanguageChoice::Fixed(Language::Es), LanguageChoice::Fixed(Language::En)];

impl ArchiApp {
  pub(super) fn settings_screen(&mut self, ui: &mut Ui) {
    ui.label(theme::heading(self.tr("ui-nav-settings"), 30.0));
    ui.add_space(16.0);
    self.game_picker(ui);
    if self.settings_game == archi_cs2::GAME_ID || self.settings_game == archi_lol::GAME_ID {
      self.integration_settings(ui);
    } else {
      self.account_settings(ui);
      self.api_key_settings(ui);
    }
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
      super::UpdateUi::Failed(_, problem) => Some((self.tr(&problem.token("")), theme::BAD)),
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
      // Other texts, other row heights.
      self.row_heights.clear();
      if let Some(tray) = &self.tray {
        tray.set_labels(&self.tray_labels());
      }
      // Not worth interrupting over: the choice still applies this session.
      let _ = self.storage.set_setting(LanguageChoice::SETTING, choice.code());
    }
  }

  /// Which game's account, key and tracking the next blocks show.
  fn game_picker(&mut self, ui: &mut Ui) {
    let games: Vec<(String, String)> = self.catalog.games().iter().map(|game| (game.game.id.clone(), self.tr(&game.game.name))).collect();
    let names: Vec<String> = games.iter().map(|(_, name)| name.clone()).collect();
    let current = games.iter().position(|(id, _)| *id == self.settings_game).unwrap_or(0);
    let mut picked = None;
    block(ui, &self.tr("ui-settings-game"), |ui| picked = widgets::segmented(ui, &names, current));
    if let Some(i) = picked {
      self.settings_game = games[i].0.clone();
    }
  }

  fn api_key_settings(&mut self, ui: &mut Ui) {
    let game_id = self.settings_game.clone();
    let help = self.tr(&format!("{game_id}-api-key-help"));
    let (state, color) = if self.game(&game_id).status.has_api_key {
      (self.tr("ui-settings-api-key-saved"), theme::GOOD)
    } else {
      (self.tr("ui-settings-api-key-missing"), theme::BAD)
    };
    let save = self.tr("ui-save");
    let title = self.tr(&format!("{game_id}-api-key"));
    let Some(game) = self.games.get_mut(&game_id) else { return };
    let mut send = None;
    block(ui, &title, |ui| {
      ui.label(theme::muted(help));
      widgets::hud(ui, &state, color);
      ui.add_space(4.0);
      ui.horizontal(|ui| {
        ui.add(TextEdit::singleline(&mut game.api_key_input).password(true).desired_width(360.0).margin(egui::vec2(10.0, 8.0)));
        ui.add_enabled_ui(!game.api_key_input.trim().is_empty(), |ui| {
          if widgets::button(ui, &save, widgets::Kind::Primary).clicked() {
            send = Some(std::mem::take(&mut game.api_key_input));
          }
        });
      });
    });
    if let Some(key) = send {
      self.worker.send(Command::SetApiKey { game: game_id, key });
    }
  }

  fn account_settings(&mut self, ui: &mut Ui) {
    let game_id = self.settings_game.clone();
    let hint = self.tr(&format!("{game_id}-account-hint"));
    let title = self.tr(&format!("{game_id}-account"));
    let link = self.tr("ui-link");
    let result = self.game(&game_id).link_result.map(|result| match result {
      Ok(()) => (self.tr("ui-linked-ok"), theme::GOOD),
      Err(problem) => (self.tr(&problem.token(&game_id)), theme::BAD),
    });
    let Some(game) = self.games.get_mut(&game_id) else { return };
    let linked = game.account.as_ref().map(|account| account.display_name.clone());
    let mut send = None;
    block(ui, &title, |ui| {
      if let Some(linked) = linked {
        ui.label(theme::heading(linked, 18.0));
      }
      ui.horizontal(|ui| {
        ui.add(TextEdit::singleline(&mut game.name_input).hint_text(hint).desired_width(320.0).margin(egui::vec2(10.0, 8.0)));
        let enabled = !game.status.busy && !game.name_input.trim().is_empty();
        ui.add_enabled_ui(enabled, |ui| {
          if widgets::button(ui, &link, widgets::Kind::Secondary).clicked() {
            send = Some(game.name_input.clone());
          }
        });
      });
      if let Some((text, color)) = result {
        ui.label(RichText::new(text).color(color));
      }
    });
    if let Some(name) = send {
      game.link_result = None;
      self.worker.send(Command::Link { game: game_id, name });
    }
  }

  /// A game fed locally: what the integration is doing, and (CS2) turning it on or off. League's
  /// needs no setup, so it has no button.
  fn integration_settings(&mut self, ui: &mut Ui) {
    let game_id = self.settings_game.clone();
    let game = self.game(&game_id);
    let integration = game.status.integration;
    let account =
      game.account.as_ref().map(|account| self.tr_args(&format!("{game_id}-integration-account"), &[("name", account.display_name.as_str().into())]));
    let help = self.tr(&format!("{game_id}-integration-help"));
    let (state, color) = match integration {
      Integration::Off => (self.tr(&format!("{game_id}-integration-off")), theme::FAINT),
      Integration::Waiting => (self.tr(&format!("{game_id}-integration-waiting")), theme::ACCENT_INK),
      Integration::Receiving => (self.tr(&format!("{game_id}-integration-receiving")), theme::GOOD),
    };
    let button = (game_id == archi_cs2::GAME_ID).then(|| {
      if integration == Integration::Off {
        (self.tr(&format!("{game_id}-integration-enable")), widgets::Kind::Primary)
      } else {
        (self.tr(&format!("{game_id}-integration-disable")), widgets::Kind::Secondary)
      }
    });
    let mut clicked = false;
    block(ui, &self.tr(&format!("{game_id}-integration")), |ui| {
      ui.label(theme::muted(help));
      ui.add_space(4.0);
      ui.label(RichText::new(state).color(color));
      if let Some(account) = account {
        ui.label(theme::heading(account, 16.0));
      }
      if let Some((label, kind)) = button {
        ui.add_space(4.0);
        clicked = widgets::button(ui, &label, kind).clicked();
      }
    });
    if clicked {
      self.worker.send(Command::SetIntegration { game: game_id, enabled: integration == Integration::Off });
    }
  }

  fn tracking_settings(&mut self, ui: &mut Ui) {
    let game_id = self.settings_game.clone();
    let game = self.game(&game_id);
    let (status, linked) = (game.status.clone(), game.account.is_some());
    // A game fed by the game itself (CS2, League) isn't polled.
    let local = game_id == archi_cs2::GAME_ID || game_id == archi_lol::GAME_ID;
    let name = self.catalog.get(&game_id).map(|definition| self.tr(&definition.game.name)).unwrap_or_default();
    let running = match (local, status.game_running) {
      (true, true) => self.tr(&format!("{game_id}-running")),
      (true, false) => self.tr(&format!("{game_id}-closed")),
      (false, true) => self.tr_args("ui-game-running", &[("game", name.into())]),
      (false, false) => self.tr_args("ui-game-closed", &[("game", name.into()), ("note", self.tr(&format!("{game_id}-closed-note")).into())]),
    };
    let when = status.last_poll.map_or_else(|| self.tr("ui-never"), |at| self.when(at));
    let last_poll = if local {
      self.tr_args(&format!("{game_id}-last-match"), &[("when", when.into())])
    } else {
      self.tr_args("ui-last-poll", &[("when", when.into())])
    };
    let problem = status.problem.map(|problem| self.tr(&problem.token(&game_id)));
    // Replays are Fortnite's only.
    let replays = (game_id == archi_fortnite::GAME_ID).then(|| {
      if status.replays > 0 {
        (self.tr_args("ui-replays-count", &[("count", status.replays.into())]), theme::MUTED)
      } else {
        (self.tr("ui-replays-none"), theme::FAINT)
      }
    });
    let auto_delete_label = self.tr("ui-replays-auto-delete");
    let auto_delete_help = self.tr("ui-replays-auto-delete-help");
    let mut auto_delete = self.auto_delete_replays;
    let button = if status.busy { self.tr("ui-busy") } else { self.tr("ui-poll-now") };
    let can_poll = !status.busy && status.has_api_key && linked;
    block(ui, &self.tr("ui-settings-tracking"), |ui| {
      ui.label(running);
      widgets::hud(ui, &last_poll, theme::MUTED);
      if let Some((text, color)) = &replays {
        ui.label(RichText::new(text).color(*color));
      }
      if let Some(problem) = problem {
        ui.label(RichText::new(problem).color(theme::BAD));
      }
      if !local {
        ui.add_space(4.0);
        ui.add_enabled_ui(can_poll, |ui| {
          if widgets::button(ui, &button, widgets::Kind::Secondary).clicked() {
            self.worker.send(Command::PollNow);
          }
        });
      }
      if replays.is_some() {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
          widgets::toggle(ui, &mut auto_delete);
          ui.label(theme::body(auto_delete_label, 14.0, 500.0));
        });
        ui.label(theme::muted(auto_delete_help));
      }
    });
    if auto_delete != self.auto_delete_replays {
      self.set_auto_delete_replays(auto_delete);
    }
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
