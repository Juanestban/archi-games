//! The main window: navigation plus the Profile, Games, Achievements,
//! History and Settings screens. Reads go through the UI's own database
//! connection; anything that tracks or writes stats goes to the worker.

mod achievements;
mod games;
mod history;
mod profile;
mod settings;
mod theme;

use std::sync::Arc;
use std::time::Duration;

use archi_core::{AchievementDef, GameDefinition, GameProgress, StatSnapshot, Tier};
use archi_fortnite::GAME_ID;
use archi_storage::{AccountSummary, LinkedAccount, PLATFORM_ID, Storage, Timestamp, UnlockRecord};
use eframe::egui::{self, FontFamily, FontId, RichText, ScrollArea, Ui};
use fluent_bundle::FluentValue;

use crate::catalog::Catalog;
use crate::i18n::{I18n, LanguageChoice};
use crate::worker::{self, Event, Problem, Status};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
  Profile,
  Games,
  Achievements,
  History,
  Settings,
}

impl Screen {
  const ALL: [Self; 5] = [Self::Profile, Self::Games, Self::Achievements, Self::History, Self::Settings];

  fn token(self) -> &'static str {
    match self {
      Self::Profile => "ui-nav-profile",
      Self::Games => "ui-nav-games",
      Self::Achievements => "ui-nav-achievements",
      Self::History => "ui-nav-history",
      Self::Settings => "ui-nav-settings",
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StateFilter {
  All,
  Unlocked,
  Locked,
}

pub struct ArchiApp {
  catalog: Arc<Catalog>,
  storage: Storage,
  worker: worker::Handle,
  i18n: I18n,
  language: LanguageChoice,
  screen: Screen,

  status: Status,
  progress: GameProgress,
  platform_progress: GameProgress,
  summary: AccountSummary,
  history: Vec<UnlockRecord>,
  account: Option<LinkedAccount>,
  load_failed: bool,

  // Achievements screen.
  shown_game: String,
  state_filter: StateFilter,
  tier_filter: Option<Tier>,

  // Settings screen.
  api_key_input: String,
  epic_name_input: String,
  link_result: Option<Result<(), Problem>>,
}

impl ArchiApp {
  pub fn new(cc: &eframe::CreationContext<'_>, catalog: Arc<Catalog>, storage: Storage, worker: worker::Handle) -> Self {
    theme::apply(&cc.egui_ctx);
    let language = storage.setting(LanguageChoice::SETTING).ok().flatten().map(|code| LanguageChoice::from_code(&code)).unwrap_or_default();
    let summary = storage.account_summary(&catalog.all(), &catalog.rules).unwrap_or_else(|_| empty_summary(&catalog));
    let mut app = Self {
      i18n: I18n::new(language.resolve()),
      language,
      screen: Screen::Profile,
      status: Status::default(),
      progress: GameProgress::default(),
      platform_progress: GameProgress::default(),
      summary,
      history: Vec::new(),
      account: None,
      load_failed: false,
      shown_game: GAME_ID.into(),
      state_filter: StateFilter::All,
      tier_filter: None,
      api_key_input: String::new(),
      epic_name_input: String::new(),
      link_result: None,
      catalog,
      storage,
      worker,
    };
    app.reload();
    app
  }

  /// Re-reads what the worker may have written.
  fn reload(&mut self) {
    let catalog = Arc::clone(&self.catalog);
    let all = catalog.all();
    let loaded = (|| {
      self.summary = self.storage.account_summary(&all, &self.catalog.rules)?;
      self.history = self.storage.unlocks()?;
      self.account = self.storage.account(GAME_ID)?;
      let mut platform = GameProgress::with_baseline(StatSnapshot::new());
      platform.last = Some(self.summary.platform_stats());
      platform.unlocked = self.storage.unlocked_ids(PLATFORM_ID)?;
      self.platform_progress = platform;
      Ok::<_, archi_storage::StorageError>(())
    })();
    self.load_failed = loaded.is_err();
  }

  fn handle_events(&mut self) {
    while let Some(event) = self.worker.try_recv() {
      match event {
        Event::Status(status) => self.status = status,
        Event::Progress(progress) => self.progress = progress,
        Event::Unlocked(_) => self.reload(),
        Event::Linked => {
          self.link_result = Some(Ok(()));
          self.epic_name_input.clear();
          self.reload();
        }
        Event::LinkFailed(problem) => self.link_result = Some(Err(problem)),
      }
    }
  }

  fn tr(&self, id: &str) -> String {
    self.i18n.tr(id)
  }

  fn tr_args<'a>(&self, id: &str, args: &[(&'a str, FluentValue<'a>)]) -> String {
    self.i18n.tr_args(id, args)
  }

  fn when(&self, at: Timestamp) -> String {
    self.i18n.when(at, archi_storage::unix_now())
  }

  fn unlocked_at(&self, game_id: &str, achievement_id: &str) -> Option<Timestamp> {
    self.history.iter().find(|record| record.game_id == game_id && record.achievement_id == achievement_id).map(|record| record.unlocked_at)
  }

  fn progress_of(&self, game_id: &str) -> &GameProgress {
    if game_id == PLATFORM_ID { &self.platform_progress } else { &self.progress }
  }

  /// (unlocked, total) achievements of a game.
  fn completion(&self, definition: &GameDefinition) -> (usize, usize) {
    let unlocked = definition.achievements.iter().filter(|a| self.unlocked_at(&definition.game.id, &a.id).is_some()).count();
    (unlocked, definition.achievements.len())
  }

  fn nav(&mut self, ui: &mut Ui) {
    ui.label(RichText::new("ARCHI GAMES").font(FontId::new(20.0, FontFamily::Proportional)).strong().color(theme::ACCENT));
    ui.add_space(4.0);
    let level = self.tr_args("ui-level", &[("level", self.summary.level.level.into())]);
    ui.label(theme::muted(level));
    ui.add_space(20.0);

    for screen in Screen::ALL {
      let label = RichText::new(self.tr(screen.token())).size(16.0);
      let button = egui::Button::selectable(self.screen == screen, label).min_size(egui::vec2(ui.available_width(), 36.0));
      if ui.add(button).clicked() {
        self.screen = screen;
      }
      ui.add_space(4.0);
    }

    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
      if let Some(problem) = self.status.problem.or(self.load_failed.then_some(Problem::Storage)) {
        ui.label(RichText::new(self.tr(problem.token())).small().color(theme::DANGER));
      } else if self.status.busy {
        ui.label(theme::muted(self.tr("ui-busy")).small());
      }
    });
  }

  /// One achievement as a card: badge, name, description, then either its
  /// unlock date or its progress bar.
  fn achievement_card(&self, ui: &mut Ui, game_id: &str, achievement: &AchievementDef) {
    let unlocked_at = self.unlocked_at(game_id, &achievement.id);
    let secret = achievement.hidden && unlocked_at.is_none();
    theme::card().show(ui, |ui| {
      ui.set_width(ui.available_width());
      ui.horizontal(|ui| {
        theme::tier_badge(ui, achievement.tier, unlocked_at.is_some(), 40.0);
        ui.add_space(8.0);
        ui.vertical(|ui| {
          let (name, description) = if secret {
            (self.tr("ui-hidden-name"), self.tr("ui-hidden-desc"))
          } else {
            (self.tr(&achievement.name), self.tr(&achievement.description))
          };
          ui.label(RichText::new(name).size(16.0).strong());
          ui.label(theme::muted(description));
          if let Some(at) = unlocked_at {
            ui.label(RichText::new(self.tr_args("ui-unlocked-when", &[("when", self.when(at).into())])).small().color(theme::SUCCESS));
          } else if !secret && let Some(progress) = self.progress_of(game_id).progress(achievement) {
            let current = progress.current.min(progress.target);
            let fraction = current as f32 / progress.target.max(1) as f32;
            ui.add(egui::ProgressBar::new(fraction).text(format!("{current} / {}", progress.target)).desired_width(320.0).fill(theme::ACCENT));
          }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
          let xp = self.catalog.rules.xp_for(achievement.tier);
          ui.label(RichText::new(self.tr_args("ui-xp-reward", &[("xp", xp.into())])).strong().color(theme::tier_color(achievement.tier)));
          if achievement.pack.is_some() {
            ui.label(theme::muted(self.tr("ui-pack")).small());
          }
        });
      });
    });
    ui.add_space(8.0);
  }

  /// A row of the unlock history.
  fn unlock_card(&self, ui: &mut Ui, record: &UnlockRecord) {
    let Some(definition) = self.catalog.get(&record.game_id) else { return };
    let Some(achievement) = definition.get(&record.achievement_id) else { return };
    theme::card().show(ui, |ui| {
      ui.set_width(ui.available_width());
      ui.horizontal(|ui| {
        theme::tier_badge(ui, achievement.tier, true, 32.0);
        ui.add_space(8.0);
        ui.vertical(|ui| {
          ui.label(RichText::new(self.tr(&achievement.name)).strong());
          ui.label(theme::muted(format!("{} · {}", self.tr(&definition.game.name), self.when(record.unlocked_at))).small());
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
          let xp = self.catalog.rules.xp_for(achievement.tier);
          ui.label(RichText::new(self.tr_args("ui-xp-reward", &[("xp", xp.into())])).color(theme::tier_color(achievement.tier)));
        });
      });
    });
    ui.add_space(6.0);
  }
}

fn empty_summary(catalog: &Catalog) -> AccountSummary {
  AccountSummary { total_xp: 0, level: catalog.rules.level_for(0), by_tier: Default::default(), platinums: 0, game_unlocked: 0 }
}

impl eframe::App for ArchiApp {
  fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
    self.handle_events();
  }

  fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
    // Keeps "5 min ago" and the like current.
    ui.ctx().request_repaint_after(Duration::from_secs(30));

    egui::Panel::left("nav").exact_size(210.0).resizable(false).frame(theme::nav_frame()).show(ui, |ui| self.nav(ui));
    egui::CentralPanel::default_margins().frame(theme::content_frame()).show(ui, |ui| {
      ScrollArea::vertical().auto_shrink(false).show(ui, |ui| match self.screen {
        Screen::Profile => self.profile_screen(ui),
        Screen::Games => self.games_screen(ui),
        Screen::Achievements => self.achievements_screen(ui),
        Screen::History => self.history_screen(ui),
        Screen::Settings => self.settings_screen(ui),
      });
    });
  }
}
