//! The main window, laid out like ArtCraft's shell: a custom title bar, a sidebar flush with
//! the window's edge and a content panel with a breadcrumb header. Reads go through the UI's
//! own database connection; anything that tracks or writes stats goes to the worker.

mod achievements;
mod chrome;
mod games;
mod history;
mod icons;
mod notify;
mod profile;
mod settings;
mod theme;
mod widgets;

use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock, mpsc};
use std::time::Duration;

use archi_core::{AchievementDef, GameDefinition, GameProgress, StatSnapshot, Tier};
use archi_storage::{AccountSummary, LinkedAccount, PLATFORM_ID, Storage, Timestamp, UnlockRecord};
use eframe::egui::{self, Align2, Color32, Frame, Id, Margin, Rect, RichText, ScrollArea, Sense, Ui, pos2, vec2};
use fluent_bundle::FluentValue;
use icons::Icon;

use crate::catalog::Catalog;
use crate::desktop::{self, SingleInstance};
use crate::i18n::{I18n, LanguageChoice};
use crate::replays;
use crate::sound::{self, Sounds};
use crate::tray::{self, ShellEvent, Tray};
use crate::update;
use crate::worker::{self, Command, Event, Problem, ReplayRef, Status, UpdateInfo};

const SIDEBAR_WIDTH: f32 = 236.0;
const HEADER_HEIGHT: f32 = 52.0;
const ROW_HEIGHT: f32 = 34.0;
/// Page transition: fade and rise.
const PAGE_IN: f64 = 0.28;
/// Bars and rings fill up after a page appears.
const FILL_IN: f64 = 0.8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
  Profile,
  Games,
  Achievements,
  History,
  Settings,
}

impl Screen {
  fn token(self) -> &'static str {
    match self {
      Self::Profile => "ui-nav-profile",
      Self::Games => "ui-nav-games",
      Self::Achievements => "ui-nav-achievements",
      Self::History => "ui-nav-history",
      Self::Settings => "ui-nav-settings",
    }
  }

  fn icon(self) -> Icon {
    match self {
      Self::Profile => Icon::User,
      Self::Games => Icon::Gamepad,
      Self::Achievements => Icon::Trophy,
      Self::History => Icon::Clock,
      Self::Settings => Icon::Gear,
    }
  }
}

/// Where the self-update stands, for the banner and Settings > About.
#[derive(Debug, Clone, PartialEq)]
enum UpdateUi {
  Idle,
  Checking,
  UpToDate,
  Available(UpdateInfo),
  Downloading(UpdateInfo, f32),
  Installing,
  Failed(Option<UpdateInfo>, Problem),
}

impl UpdateUi {
  fn info(&self) -> Option<&UpdateInfo> {
    match self {
      Self::Available(info) | Self::Downloading(info, _) | Self::Failed(Some(info), _) => Some(info),
      _ => None,
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
  /// egui time when `screen` was shown, for the page transition.
  shown_at: f64,

  /// Per game id, for every game in the catalog.
  games: BTreeMap<String, GameUi>,
  platform_progress: GameProgress,
  summary: AccountSummary,
  history: Vec<UnlockRecord>,
  load_failed: bool,

  // Achievements screen.
  shown_game: String,
  state_filter: StateFilter,
  tier_filter: Option<Tier>,

  // Settings screen: the game whose account and key are shown.
  settings_game: String,

  notifier: notify::Notifier,
  sounds: Sounds,
  /// 0–100.
  volume: u8,

  tray: Option<Tray>,
  shell_events: mpsc::Receiver<ShellEvent>,
  /// Set by the tray's Quit: closing then really exits instead of hiding.
  quitting: bool,
  autostart: bool,
  autostart_failed: bool,
  /// Started with Windows: hide on the first frame (eframe shows the window after it).
  start_hidden: bool,
  update: UpdateUi,
  auto_update: bool,
  /// Replays already read that the player may delete; offered in a banner.
  replays_to_review: Vec<ReplayRef>,
  auto_delete_replays: bool,
  /// Set when the new version is starting: close on the next frame.
  close_now: bool,
  _instance: SingleInstance,
}

/// What `main` builds for the app.
pub struct Parts {
  pub catalog: Arc<Catalog>,
  pub storage: Storage,
  pub worker: worker::Handle,
  /// For the tray's events; `shell_events` receives them.
  pub shell: mpsc::Sender<ShellEvent>,
  pub shell_events: mpsc::Receiver<ShellEvent>,
  pub instance: SingleInstance,
  pub start_hidden: bool,
}

impl ArchiApp {
  pub fn new(cc: &eframe::CreationContext<'_>, parts: Parts) -> Self {
    let Parts { catalog, storage, worker, shell, shell_events, instance, start_hidden } = parts;
    theme::apply(&cc.egui_ctx);
    let language = storage.setting(LanguageChoice::SETTING).ok().flatten().map(|code| LanguageChoice::from_code(&code)).unwrap_or_default();
    let summary = storage.account_summary(&catalog.all(), &catalog.rules).unwrap_or_else(|_| empty_summary(&catalog));
    let volume = storage.setting(sound::VOLUME_SETTING).ok().flatten().and_then(|v| v.parse().ok()).unwrap_or(sound::DEFAULT_VOLUME);
    let mut app = Self {
      i18n: I18n::new(language.resolve()),
      language,
      screen: Screen::Profile,
      shown_at: 0.0,
      games: catalog.games().iter().map(|game| (game.game.id.clone(), GameUi::default())).collect(),
      platform_progress: GameProgress::default(),
      summary,
      history: Vec::new(),
      load_failed: false,
      shown_game: archi_fortnite::GAME_ID.into(),
      state_filter: StateFilter::All,
      tier_filter: None,
      settings_game: archi_fortnite::GAME_ID.into(),
      notifier: notify::Notifier::default(),
      sounds: Sounds::new(),
      volume,
      tray: None,
      shell_events,
      quitting: false,
      autostart: desktop::autostart_enabled(desktop::AUTOSTART_ENTRY),
      autostart_failed: false,
      start_hidden,
      update: UpdateUi::Idle,
      auto_update: storage.setting(update::AUTO_CHECK_SETTING).ok().flatten().as_deref() != Some("false"),
      replays_to_review: Vec::new(),
      auto_delete_replays: storage.setting(replays::AUTO_DELETE_SETTING).ok().flatten().as_deref() == Some("true"),
      close_now: false,
      _instance: instance,
      catalog,
      storage,
      worker,
    };
    app.reload();
    // Without a tray icon, closing the window must quit, or the app would be unreachable.
    app.tray = Tray::new(&cc.egui_ctx, shell, &app.tray_labels()).ok();
    app
  }

  fn tray_labels(&self) -> tray::Labels {
    tray::Labels { show: self.tr("ui-tray-open"), poll: self.tr("ui-poll-now"), quit: self.tr("ui-tray-quit") }
  }

  fn handle_shell_events(&mut self, ctx: &egui::Context) {
    while let Ok(event) = self.shell_events.try_recv() {
      match event {
        ShellEvent::Show => {
          ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
          ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
          ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        ShellEvent::PollNow => self.worker.send(Command::PollNow),
        ShellEvent::Quit => {
          self.quitting = true;
          ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
      }
    }
  }

  /// Closing the window keeps the app in the tray.
  fn close_to_tray(&self, ctx: &egui::Context) {
    if ctx.input(|i| i.viewport().close_requested()) && !self.quitting && self.tray.is_some() {
      ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
      ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
    }
  }

  fn set_autostart(&mut self, enabled: bool) {
    let result = if enabled {
      desktop::autostart_command().and_then(|command| desktop::set_autostart(desktop::AUTOSTART_ENTRY, Some(&command)))
    } else {
      desktop::set_autostart(desktop::AUTOSTART_ENTRY, None)
    };
    self.autostart_failed = result.is_err();
    self.autostart = desktop::autostart_enabled(desktop::AUTOSTART_ENTRY);
  }

  /// Re-reads what the worker may have written.
  fn reload(&mut self) {
    let catalog = Arc::clone(&self.catalog);
    let all = catalog.all();
    let loaded = (|| {
      self.summary = self.storage.account_summary(&all, &self.catalog.rules)?;
      self.history = self.storage.unlocks()?;
      for (game_id, game) in &mut self.games {
        game.account = self.storage.account(game_id)?;
      }
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
        Event::Status(game_id, status) => self.games.entry(game_id).or_default().status = status,
        Event::Progress(game_id, progress) => self.games.entry(game_id).or_default().progress = progress,
        Event::Unlocked(unlocks) => {
          let level_before = self.summary.level.level;
          self.reload();
          let level = self.summary.level.level;
          self.notifier.push(notify::toasts(&self.catalog, &unlocks, (level > level_before).then_some(level)));
        }
        Event::Linked(game_id) => {
          let game = self.games.entry(game_id).or_default();
          game.link_result = Some(Ok(()));
          game.name_input.clear();
          self.reload();
        }
        Event::LinkFailed(game_id, problem) => self.games.entry(game_id).or_default().link_result = Some(Err(problem)),
        Event::UpdateAvailable(info) => self.update = UpdateUi::Available(info),
        Event::UpToDate => self.update = UpdateUi::UpToDate,
        Event::UpdateProgress(fraction) => {
          if let Some(info) = self.update.info().cloned() {
            self.update = UpdateUi::Downloading(info, fraction);
          }
        }
        Event::UpdateFailed(problem) => self.update = UpdateUi::Failed(self.update.info().cloned(), problem),
        Event::ReplaysToReview(files) => self.replays_to_review = files,
        Event::UpdateReady => {
          self.update = UpdateUi::Installing;
          self.quitting = true;
          self.close_now = true;
        }
      }
    }
  }

  fn go(&mut self, ui: &Ui, screen: Screen) {
    if screen != self.screen {
      self.screen = screen;
      self.shown_at = ui.input(|i| i.time);
    }
  }

  /// 0→1 over `duration` seconds since the page appeared.
  fn page_t(&self, ui: &Ui, duration: f64) -> f32 {
    let t = ((ui.input(|i| i.time) - self.shown_at) / duration) as f32;
    if t < 1.0 {
      ui.ctx().request_repaint();
    }
    widgets::ease_out(t)
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
    if game_id == PLATFORM_ID { &self.platform_progress } else { &self.game(game_id).progress }
  }

  fn game(&self, game_id: &str) -> &GameUi {
    static NONE: LazyLock<GameUi> = LazyLock::new(GameUi::default);
    self.games.get(game_id).unwrap_or(&NONE)
  }

  /// The name shown for the player: the first linked account, in catalog order.
  fn display_name(&self) -> Option<&str> {
    self.catalog.games().iter().find_map(|game| self.game(&game.game.id).account.as_ref()).map(|account| account.display_name.as_str())
  }

  /// (unlocked, total) achievements of a game.
  fn completion(&self, definition: &GameDefinition) -> (usize, usize) {
    let unlocked = definition.achievements.iter().filter(|a| self.unlocked_at(&definition.game.id, &a.id).is_some()).count();
    (unlocked, definition.achievements.len())
  }

  fn title_bar(&mut self, ui: &mut Ui) {
    chrome::title_bar_drag(ui);
    let rect = ui.max_rect();
    let mut bar = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let ui = &mut bar;
    ui.add_space(14.0);
    let (mark, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
    ui.painter().rect_filled(mark, theme::RADIUS, theme::ACCENT);
    icons::trophy(ui.painter(), mark.shrink(4.0), Color32::WHITE, true);
    ui.add_space(4.0);
    widgets::hud(ui, "Archi Games", theme::INK);

    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
      ui.spacing_mut().item_spacing.x = 0.0;
      chrome::caption_buttons(ui);
      ui.add_space(12.0);
      self.status_pill(ui);
    });
  }

  /// Tracking at a glance, over every game: a dot and a mono label.
  fn status_pill(&self, ui: &mut Ui) {
    let problem = self
      .games
      .iter()
      .find_map(|(game_id, game)| game.status.problem.map(|problem| problem.token(game_id)))
      .or(self.load_failed.then(|| Problem::Storage.token("")));
    let any = |test: fn(&GameUi) -> bool| self.games.values().any(test);
    let busy = any(|game| game.status.busy);
    let (color, token) = match problem {
      Some(_) => (theme::BAD, "ui-status-problem"),
      None if busy => (theme::ACCENT_400, "ui-busy"),
      None if !any(|game| game.status.has_api_key && game.account.is_some()) => (theme::FAINT, "ui-status-setup"),
      None if any(|game| game.status.game_running) => (theme::GOOD, "ui-status-in-game"),
      None => (theme::GOOD, "ui-status-tracking"),
    };
    let label = widgets::caps(ui, &self.tr(token), theme::HUD_SIZE, theme::MUTED);
    let (rect, response) = ui.allocate_exact_size(vec2(label.size().x + 34.0, 24.0), Sense::hover());
    ui.painter().rect(rect, theme::RADIUS, theme::WASH, theme::hairline(), egui::StrokeKind::Inside);
    let dot = pos2(rect.left() + 12.0, rect.center().y);
    if busy {
      // Pulses while a check runs.
      let pulse = ((ui.input(|i| i.time) * 4.0).sin() * 0.5 + 0.5) as f32;
      ui.painter().circle_filled(dot, 3.0 + 3.0 * pulse, theme::fade(color, 0.35 * (1.0 - pulse)));
      ui.ctx().request_repaint();
    }
    ui.painter().circle_filled(dot, 3.0, color);
    ui.painter().galley(pos2(rect.left() + 22.0, rect.center().y - label.size().y / 2.0), label, theme::MUTED);
    if let Some(problem) = problem {
      response.on_hover_text(self.tr(&problem));
    }
  }

  fn sidebar(&mut self, ui: &mut Ui) {
    let rect = ui.max_rect();
    let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(12.0, 8.0))));
    let ui = &mut inner;
    ui.spacing_mut().item_spacing.y = 2.0;

    let mut selected_rect = None;
    widgets::section(ui, &self.tr("ui-sidebar-library"));
    for screen in [Screen::Profile, Screen::Games, Screen::Achievements, Screen::History] {
      let (row, clicked) = self.nav_row(ui, screen);
      if screen == self.screen {
        selected_rect = Some(row);
      }
      if clicked {
        self.go(ui, screen);
      }
    }

    // Settings and the account sit at the bottom.
    let bottom = Rect::from_min_max(pos2(rect.left() + 12.0, rect.bottom() - 110.0), pos2(rect.right() - 12.0, rect.bottom() - 12.0));
    let mut foot = ui.new_child(egui::UiBuilder::new().max_rect(bottom));
    let (row, clicked) = self.nav_row(&mut foot, Screen::Settings);
    if self.screen == Screen::Settings {
      selected_rect = Some(row);
    }
    if clicked {
      self.go(&foot, Screen::Settings);
    }
    foot.add_space(8.0);
    self.account_card(&mut foot);

    // The selection marker slides between rows.
    if let Some(row) = selected_rect {
      let y = ui.ctx().animate_value_with_time(Id::unique("nav-marker"), row.center().y, 0.18);
      ui.painter().rect_filled(Rect::from_center_size(pos2(rect.left() + 3.0, y), vec2(2.0, 18.0)), 0.0, Color32::WHITE);
    }
  }

  fn nav_row(&self, ui: &mut Ui, screen: Screen) -> (Rect, bool) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
    let selected = self.screen == screen;
    let hover = ui.ctx().animate_bool_with_time(response.id, response.hovered() || selected, 0.12);
    let p = ui.painter();
    p.rect_filled(
      rect,
      theme::RADIUS,
      theme::fade(if selected { theme::WASH_SELECTED } else { theme::WASH }, hover.max(if selected { 1.0 } else { 0.0 })),
    );
    let ink = widgets::lerp(theme::SIDEBAR_INK, Color32::WHITE, hover);
    icons::paint(p, icons::square(pos2(rect.left() + 18.0, rect.center().y), 16.0), screen.icon(), ink);
    let weight = if selected { 600.0 } else { 450.0 };
    let galley = ui.painter().layout_job(egui::text::LayoutJob::single_section(
      self.tr(screen.token()),
      egui::TextFormat {
        font_id: egui::FontId::proportional(14.0),
        color: ink,
        coords: egui::epaint::text::VariationCoords::new([("wght", weight)]),
        ..Default::default()
      },
    ));
    ui.painter().galley(pos2(rect.left() + 36.0, rect.center().y - galley.size().y / 2.0), galley, ink);
    (rect, response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked())
  }

  /// Avatar, the player's name, level and a thin XP bar.
  fn account_card(&self, ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0), Sense::hover());
    let p = ui.painter();
    p.rect(rect, theme::RADIUS, theme::WASH, theme::hairline(), egui::StrokeKind::Inside);
    let name = self.display_name().map_or_else(|| self.tr("ui-account-none-short"), str::to_owned);
    let avatar = pos2(rect.left() + 26.0, rect.center().y);
    p.circle_filled(avatar, 15.0, theme::ACCENT);
    let initial = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
    p.text(avatar, Align2::CENTER_CENTER, initial, theme::display(14.0), Color32::WHITE);
    p.text(pos2(avatar.x + 24.0, rect.top() + 17.0), Align2::LEFT_CENTER, name, egui::FontId::proportional(13.5), theme::INK);
    let level = self.summary.level;
    let label = self.tr_args("ui-level", &[("level", level.level.into())]);
    let galley = widgets::caps(ui, &label, 10.0, theme::MUTED);
    ui.painter().galley(pos2(avatar.x + 24.0, rect.top() + 28.0), galley, theme::MUTED);
    let bar = Rect::from_min_size(pos2(avatar.x + 24.0, rect.bottom() - 12.0), vec2(rect.right() - avatar.x - 36.0, 3.0));
    let fraction = level.xp_into_level as f32 / level.xp_for_next.max(1) as f32;
    let fraction = ui.ctx().animate_value_with_time(Id::unique("sidebar-xp"), fraction, 0.6);
    ui.painter().rect_filled(bar, 0.0, theme::SUNKEN);
    ui.painter().rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * fraction, bar.height())), 0.0, theme::ACCENT);
  }

  /// Breadcrumbs: "ARCHI GAMES › [SCREEN]", the leaf inverted like ArtCraft's.
  fn page_header(&self, ui: &mut Ui) {
    let rect = ui.max_rect();
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, theme::hairline());
    let mut bar =
      ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(28.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let ui = &mut bar;
    widgets::hud(ui, "Archi Games", theme::MUTED);
    let (chevron, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
    icons::paint(ui.painter(), chevron, Icon::ChevronRight, theme::FAINT);
    let galley = widgets::caps(ui, &self.tr(self.screen.token()), theme::HUD_SIZE, theme::BG);
    let (crumb, _) = ui.allocate_exact_size(galley.size() + vec2(14.0, 10.0), Sense::hover());
    ui.painter().rect_filled(crumb, 0.0, theme::INK);
    ui.painter().galley(crumb.center() - galley.size() / 2.0, galley, theme::BG);
  }

  /// A strip above the page while an update is available, downloading or failed.
  fn update_banner(&mut self, ui: &mut Ui) {
    let Some(info) = self.update.info().cloned() else {
      if self.update != UpdateUi::Installing {
        return;
      }
      let text = self.tr("ui-update-installing");
      banner_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        widgets::hud(ui, &text, theme::ACCENT_INK);
      });
      return;
    };
    let mut action = None;
    banner_frame().show(ui, |ui| {
      ui.set_width(ui.available_width());
      ui.horizontal(|ui| {
        match &self.update {
          UpdateUi::Downloading(_, fraction) => {
            let percent = (fraction * 100.0).round() as u32;
            widgets::hud(
              ui,
              &self.tr_args("ui-update-downloading", &[("version", info.version.as_str().into()), ("percent", percent.into())]),
              theme::INK,
            );
            widgets::progress(ui, 220.0, 4.0, *fraction, theme::ACCENT);
          }
          UpdateUi::Failed(_, problem) => {
            ui.label(RichText::new(self.tr(&problem.token(""))).color(theme::BAD));
          }
          _ => {
            widgets::hud(ui, &self.tr_args("ui-update-available", &[("version", info.version.as_str().into())]), theme::INK);
          }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
          if matches!(self.update, UpdateUi::Downloading(..)) {
            return;
          }
          let install = if matches!(self.update, UpdateUi::Failed(..)) { "ui-update-retry" } else { "ui-update-install" };
          if widgets::button(ui, &self.tr(install), widgets::Kind::Accent).clicked() {
            action = Some(Command::InstallUpdate);
          }
          if widgets::button(ui, &self.tr("ui-update-notes"), widgets::Kind::Secondary).clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(&info.page));
          }
        });
      });
    });
    if let Some(command) = action {
      self.update = UpdateUi::Downloading(info, 0.0);
      self.worker.send(command);
    }
  }

  /// Asks whether to delete the replays just read. Deleting waits for the game to close.
  fn replays_banner(&mut self, ui: &mut Ui) {
    if self.replays_to_review.is_empty() {
      return;
    }
    let count = self.replays_to_review.len() as u64;
    let megabytes = self.replays_to_review.iter().map(|file| file.size).sum::<u64>() as f64 / (1024.0 * 1024.0);
    let text = self.tr_args("ui-replays-review", &[("count", count.into()), ("size", format!("{megabytes:.1}").into())]);
    let waiting = self.game(archi_fortnite::GAME_ID).status.game_running.then(|| self.tr("ui-replays-review-wait"));
    let (delete, keep, always) = (self.tr("ui-replays-delete"), self.tr("ui-replays-keep"), self.tr("ui-replays-delete-always"));
    let mut action = None;
    banner_frame().show(ui, |ui| {
      ui.set_width(ui.available_width());
      ui.horizontal(|ui| {
        ui.vertical(|ui| {
          widgets::hud(ui, &text, theme::INK);
          if let Some(waiting) = &waiting {
            ui.label(theme::muted(waiting.clone()));
          }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
          ui.add_enabled_ui(waiting.is_none(), |ui| {
            if widgets::button(ui, &delete, widgets::Kind::Accent).clicked() {
              action = Some(ReplayAction::Delete);
            }
          });
          if widgets::button(ui, &keep, widgets::Kind::Secondary).clicked() {
            action = Some(ReplayAction::Keep);
          }
          ui.add_enabled_ui(waiting.is_none(), |ui| {
            if widgets::button(ui, &always, widgets::Kind::Secondary).clicked() {
              action = Some(ReplayAction::DeleteAlways);
            }
          });
        });
      });
    });
    let Some(action) = action else { return };
    let files = std::mem::take(&mut self.replays_to_review);
    match action {
      ReplayAction::Keep => self.worker.send(Command::KeepReplays(files)),
      ReplayAction::Delete => self.worker.send(Command::DeleteReplays(files)),
      ReplayAction::DeleteAlways => {
        self.set_auto_delete_replays(true);
        self.worker.send(Command::DeleteReplays(files));
      }
    }
  }

  pub(super) fn set_auto_delete_replays(&mut self, enabled: bool) {
    self.auto_delete_replays = enabled;
    // Not worth interrupting over: the choice still applies this session.
    let _ = self.storage.set_setting(replays::AUTO_DELETE_SETTING, if enabled { "true" } else { "false" });
    if enabled {
      self.worker.send(Command::ScanReplays);
    }
  }

  fn content(&mut self, ui: &mut Ui) {
    self.update_banner(ui);
    self.replays_banner(ui);
    let t = self.page_t(ui, PAGE_IN);
    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
      Frame::NONE.inner_margin(Margin { left: 28, right: 28, top: 24, bottom: 28 }).show(ui, |ui| {
        ui.set_opacity(t);
        ui.add_space((1.0 - t) * 16.0);
        match self.screen {
          Screen::Profile => self.profile_screen(ui),
          Screen::Games => self.games_screen(ui),
          Screen::Achievements => self.achievements_screen(ui),
          Screen::History => self.history_screen(ui),
          Screen::Settings => self.settings_screen(ui),
        }
      });
    });
  }

  /// An achievement as a list row: badge, name, description, then its unlock date or
  /// progress; reward on the right.
  fn achievement_row(&self, ui: &mut Ui, game_id: &str, achievement: &AchievementDef) {
    let unlocked_at = self.unlocked_at(game_id, &achievement.id);
    let secret = achievement.hidden && unlocked_at.is_none();
    let fill = self.page_t(ui, FILL_IN);
    widgets::hover_row(ui, Id::unique(("achievement", game_id, &achievement.id)), |ui| {
      ui.horizontal(|ui| {
        widgets::tier_badge(ui, achievement.tier, unlocked_at.is_some(), 46.0);
        ui.add_space(10.0);
        ui.vertical(|ui| {
          ui.spacing_mut().item_spacing.y = 3.0;
          let (name, description) = if secret {
            (self.tr("ui-hidden-name"), self.tr("ui-hidden-desc"))
          } else {
            (self.tr(&achievement.name), self.tr(&achievement.description))
          };
          ui.label(theme::body(name, 15.0, 600.0));
          ui.label(theme::muted(description));
          if let Some(at) = unlocked_at {
            widgets::hud(ui, &self.tr_args("ui-unlocked-when", &[("when", self.when(at).into())]), theme::GOOD);
          } else if !secret && let Some(progress) = self.progress_of(game_id).progress(achievement) {
            let current = progress.current.min(progress.target);
            ui.horizontal(|ui| {
              let fraction = current as f32 / progress.target.max(1) as f32;
              widgets::progress(ui, 220.0, 4.0, fraction * fill, theme::ACCENT);
              ui.label(RichText::new(format!("{current} / {}", progress.target)).font(theme::mono(11.0)).color(theme::MUTED));
            });
          }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
          ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
            let xp = self.catalog.rules.xp_for(achievement.tier);
            let color = theme::tier_color(achievement.tier);
            ui.label(RichText::new(self.tr_args("ui-xp-reward", &[("xp", xp.into())])).font(theme::mono(13.0)).color(color));
            widgets::hud(ui, &self.tr(theme::tier_token(achievement.tier)), theme::fade(color, 0.75));
            if achievement.pack.is_some() {
              widgets::hud(ui, &self.tr("ui-pack"), theme::FAINT);
            }
          });
        });
      });
    });
  }

  /// A row of the unlock history.
  fn unlock_row(&self, ui: &mut Ui, record: &UnlockRecord) {
    let Some(definition) = self.catalog.get(&record.game_id) else { return };
    let Some(achievement) = definition.get(&record.achievement_id) else { return };
    widgets::hover_row(ui, Id::unique(("unlock", &record.game_id, &record.achievement_id)), |ui| {
      ui.horizontal(|ui| {
        widgets::tier_badge(ui, achievement.tier, true, 38.0);
        ui.add_space(10.0);
        ui.vertical(|ui| {
          ui.spacing_mut().item_spacing.y = 3.0;
          ui.label(theme::body(self.tr(&achievement.name), 14.5, 600.0));
          widgets::hud(ui, &format!("{} · {}", self.tr(&definition.game.name), self.when(record.unlocked_at)), theme::MUTED);
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
          let xp = self.catalog.rules.xp_for(achievement.tier);
          ui.label(
            RichText::new(self.tr_args("ui-xp-reward", &[("xp", xp.into())])).font(theme::mono(13.0)).color(theme::tier_color(achievement.tier)),
          );
        });
      });
    });
  }
}

/// A game's state as the UI knows it, and its Settings inputs.
#[derive(Default)]
struct GameUi {
  status: Status,
  progress: GameProgress,
  account: Option<LinkedAccount>,
  api_key_input: String,
  /// Whatever identifies the account: Epic name, Steam profile…
  name_input: String,
  link_result: Option<Result<(), Problem>>,
}

#[derive(Clone, Copy)]
enum ReplayAction {
  Delete,
  Keep,
  DeleteAlways,
}

fn banner_frame() -> Frame {
  Frame::NONE
    .fill(theme::fade(theme::ACCENT, 0.14))
    .stroke(egui::Stroke::new(1.0, theme::fade(theme::ACCENT, 0.5)))
    .inner_margin(Margin::symmetric(28, 10))
}

fn empty_summary(catalog: &Catalog) -> AccountSummary {
  AccountSummary { total_xp: 0, level: catalog.rules.level_for(0), by_tier: Default::default(), platinums: 0, game_unlocked: 0 }
}

impl eframe::App for ArchiApp {
  fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    self.handle_events();
    self.handle_shell_events(ctx);
    if std::mem::take(&mut self.close_now) {
      ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    // The main window may be minimized or hidden while a game runs; the
    // notification window still needs `ui` to run.
    if self.notifier.is_active() {
      ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
      ctx.request_repaint();
    }
  }

  /// Transparent, so the notification window has no background. The main
  /// window paints its own panels over it.
  fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
    [0.0; 4]
  }

  fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
    // Keeps "5 min ago" and the like current.
    ui.ctx().request_repaint_after(Duration::from_secs(30));
    if std::mem::take(&mut self.start_hidden) && self.tray.is_some() {
      ui.ctx().send_viewport_cmd(egui::ViewportCommand::Visible(false));
    }
    self.close_to_tray(ui.ctx());
    self.notifications(ui.ctx());

    egui::Panel::top("title-bar").exact_size(chrome::TITLE_BAR_HEIGHT).frame(Frame::NONE.fill(theme::CHROME)).show(ui, |ui| self.title_bar(ui));
    egui::Panel::left("sidebar").exact_size(SIDEBAR_WIDTH).resizable(false).frame(Frame::NONE.fill(theme::CHROME)).show(ui, |ui| self.sidebar(ui));
    egui::CentralPanel::no_frame().frame(Frame::NONE.fill(theme::BG)).show(ui, |ui| {
      let rect = ui.max_rect();
      ui.painter().vline(rect.left() + 0.5, rect.y_range(), theme::hairline());
      ui.painter().hline(rect.x_range(), rect.top() + 0.5, theme::hairline());
      egui::Panel::top("page-header").exact_size(HEADER_HEIGHT).frame(Frame::NONE).show(ui, |ui| self.page_header(ui));
      egui::CentralPanel::no_frame().show(ui, |ui| self.content(ui));
    });
    chrome::window_edges(ui.ctx());
  }
}
