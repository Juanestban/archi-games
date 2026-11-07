//! The tracker thread: a tokio runtime that watches each game's process,
//! polls its API when due (or, for CS2, listens on localhost for what the game
//! sends; for League of Legends, asks the game's own local server during a match)
//! and stores unlocks. It owns its own database connection; the UI talks
//! to it through [`Command`]s and gets [`Event`]s back, each one waking the UI
//! with `request_repaint`.

use std::hash::{BuildHasher, RandomState};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use archi_core::{AccountInfo, GameDefinition, GameProgress, GameTracker, PollPolicy, Poller, StatSnapshot, TrackError};
use archi_cs2::gsi::{self, MatchTracker};
use archi_cs2::{GSI_PORT, install as cs2_install};
use archi_fortnite::FortniteTracker;
use archi_lol::LiveClient;
use archi_lol::live::{GameData, MatchTracker as LolMatches};
use archi_storage::{LinkedAccount, ReplayFile, Storage, Timestamp, Unlock, unix_now};
use eframe::egui;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::task::JoinHandle;

use crate::catalog::Catalog;
use crate::desktop;
use crate::process;
use crate::replays;
use crate::secrets;
use crate::session::{Session, SessionError};
use crate::update::{self, Install, Update, UpdateError, Updater};

/// How often the process list is checked; polls follow the `PollPolicy`.
const PROCESS_CHECK: Duration = Duration::from_secs(5);

/// Wait after a rate limit that didn't say how long.
const RATE_LIMIT_BACKOFF: Duration = Duration::from_secs(5 * 60);

/// How often finished replays are looked for while the game is closed, and the wait after it
/// closes (the last replay is still being saved).
const REPLAY_SCAN: Duration = Duration::from_secs(2 * 60);
const REPLAY_AFTER_EXIT: Duration = Duration::from_secs(35);

/// The token CS2 must send back with every update; set while the integration is on.
const GSI_TOKEN_SETTING: &str = "cs2.gsi_token";

/// Largest update accepted from CS2 (they're a few KB).
const GSI_MAX_BODY: usize = 1 << 20;

/// First automatic update check after start, then the interval between checks, and the retry
/// after a failed one (offline right after Windows starts, GitHub down…).
const UPDATE_FIRST_CHECK: Duration = Duration::from_secs(3);
const UPDATE_INTERVAL: Duration = Duration::from_secs(60 * 60);
const UPDATE_RETRY: Duration = Duration::from_secs(5 * 60);
/// Opening the window checks again, unless a check ran this recently.
const UPDATE_SHOW_GAP: Duration = Duration::from_secs(5 * 60);

pub enum Command {
  SetApiKey {
    game: String,
    key: String,
  },
  /// `name`: whatever identifies the account for that game (Epic name, Steam profile…).
  Link {
    game: String,
    name: String,
  },
  /// Every game with a linked account.
  PollNow,
  /// `manual`: the player asked, so "up to date" and failures are reported too.
  CheckUpdate {
    manual: bool,
  },
  InstallUpdate,
  /// The window was opened (from the tray or a second launch): a good time to look for updates.
  WindowShown,
  /// The player's answer about replays already read: delete them or keep them.
  DeleteReplays(Vec<ReplayRef>),
  KeepReplays(Vec<ReplayRef>),
  /// Look for replays now, e.g. right after turning automatic deletion on.
  ScanReplays,
  /// Turns a game's local integration (CS2's Game State Integration) on or off.
  SetIntegration {
    game: String,
    enabled: bool,
  },
}

/// A replay already read that the player hasn't kept or deleted yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayRef {
  pub name: String,
  pub size: u64,
}

/// A newer release, as the UI shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
  pub version: String,
  pub notes: String,
  pub page: String,
}

pub enum Event {
  /// A game's tracking state, by game id.
  Status(String, Status),
  Progress(String, GameProgress),
  /// In notification order (platinum, then meta-achievements, last).
  Unlocked(Vec<Unlock>),
  Linked(String),
  LinkFailed(String, Problem),
  UpdateAvailable(UpdateInfo),
  UpToDate,
  /// Download progress, 0–1.
  UpdateProgress(f32),
  UpdateFailed(Problem),
  /// The new version is starting: the app must quit now.
  UpdateReady,
  /// Replays read and still on disk that the player may want to delete (the whole list).
  ReplaysToReview(Vec<ReplayRef>),
}

#[derive(Debug, Clone, Default)]
pub struct Status {
  pub has_api_key: bool,
  pub game_running: bool,
  pub last_poll: Option<Timestamp>,
  pub busy: bool,
  pub problem: Option<Problem>,
  /// Fortnite replays read for the linked account so far.
  pub replays: u32,
  pub integration: Integration,
}

/// A game's local integration (CS2's Game State Integration, League's Live Client Data), as
/// Settings shows it. League's is always on: `Waiting` until a match answers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Integration {
  #[default]
  Off,
  /// The config file is in place and the app listens; nothing has come in yet.
  Waiting,
  /// CS2 is sending its state.
  Receiving,
}

/// Something the user may need to act on, shown translated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
  NoApiKey,
  Private,
  NotFound,
  Unauthorized,
  RateLimited,
  Network,
  /// The stats service itself is down; polls go on and it clears by itself.
  Unavailable,
  Unexpected,
  Storage,
  Credentials,
  Update,
  /// The download didn't match the release's checksums; nothing was installed.
  UpdateIntegrity,
  /// Another program uses the integration's local port.
  PortInUse,
  /// The integration's config file couldn't be written or removed.
  Install,
}

impl Problem {
  /// The message's token. Private profiles, unknown accounts and network errors are worded
  /// per game (`<game>-error-…`); `game_id` is ignored for the rest.
  pub fn token(self, game_id: &str) -> String {
    let token = match self {
      Self::Private => return format!("{game_id}-error-private"),
      Self::NotFound => return format!("{game_id}-error-not-found"),
      Self::Network => return format!("{game_id}-error-network"),
      Self::PortInUse => return format!("{game_id}-error-port"),
      Self::Install => return format!("{game_id}-error-install"),
      Self::NoApiKey => "ui-error-no-api-key",
      Self::Unauthorized => "ui-error-unauthorized",
      Self::RateLimited => "ui-error-rate-limited",
      Self::Unavailable => "ui-error-unavailable",
      Self::Unexpected => "ui-error-unexpected",
      Self::Storage => "ui-error-storage",
      Self::Credentials => "ui-error-credentials",
      Self::Update => "ui-error-update",
      Self::UpdateIntegrity => "ui-error-update-integrity",
    };
    token.to_owned()
  }

  /// Out of the player's hands and retried on its own: Settings has nothing to fix, so the
  /// status pill keeps showing whether the game runs.
  pub fn is_transient(self) -> bool {
    matches!(self, Self::Network | Self::RateLimited | Self::Unavailable)
  }
}

impl From<&TrackError> for Problem {
  fn from(error: &TrackError) -> Self {
    match error {
      TrackError::NotFound => Self::NotFound,
      TrackError::Private => Self::Private,
      TrackError::Unauthorized => Self::Unauthorized,
      TrackError::RateLimited { .. } => Self::RateLimited,
      TrackError::Network(_) => Self::Network,
      TrackError::Unavailable(_) => Self::Unavailable,
      TrackError::Unexpected(_) => Self::Unexpected,
    }
  }
}

/// A game the app can track: how to recognise it and where its data comes from.
struct Game {
  id: &'static str,
  process_names: &'static [&'static str],
  /// The credential holding the player's API key; `None` for a game fed locally (CS2).
  api_key: Option<&'static str>,
}

const GAMES: [Game; 3] = [
  Game { id: archi_fortnite::GAME_ID, process_names: archi_fortnite::PROCESS_NAMES, api_key: Some(archi_fortnite::API_KEY_SETTING) },
  Game { id: archi_cs2::GAME_ID, process_names: archi_cs2::PROCESS_NAMES, api_key: None },
  Game { id: archi_lol::GAME_ID, process_names: archi_lol::PROCESS_NAMES, api_key: None },
];

/// `GameTracker` isn't object-safe (async methods), so the API trackers are an enum.
enum Tracker {
  Fortnite(FortniteTracker),
}

impl Tracker {
  fn new(game_id: &str, key: &str) -> Option<Self> {
    match game_id {
      archi_fortnite::GAME_ID => FortniteTracker::new(key).ok().map(Self::Fortnite),
      _ => None,
    }
  }
}

impl GameTracker for Tracker {
  fn game_id(&self) -> &str {
    match self {
      Self::Fortnite(tracker) => tracker.game_id(),
    }
  }

  fn process_names(&self) -> &[&str] {
    match self {
      Self::Fortnite(tracker) => tracker.process_names(),
    }
  }

  async fn find_account(&self, name: &str) -> Result<AccountInfo, TrackError> {
    match self {
      Self::Fortnite(tracker) => tracker.find_account(name).await,
    }
  }

  async fn fetch(&self, account_id: &str) -> Result<StatSnapshot, TrackError> {
    match self {
      Self::Fortnite(tracker) => tracker.fetch(account_id).await,
    }
  }
}

/// The local server CS2 posts to while the integration is on.
struct GsiServer {
  token: String,
  task: JoinHandle<()>,
  matches: MatchTracker,
}

impl Drop for GsiServer {
  fn drop(&mut self) {
    self.task.abort();
  }
}

/// One game's live state in the worker.
struct Slot<'a> {
  game: &'static Game,
  definition: &'a GameDefinition,
  tracker: Option<Tracker>,
  session: Option<Session<'a>>,
  poller: Poller,
  force_poll: bool,
  status: Status,
}

pub struct Handle {
  commands: UnboundedSender<Command>,
  events: mpsc::Receiver<Event>,
}

impl Handle {
  pub fn send(&self, command: Command) {
    // Fails only if the thread is gone, and then there's no one to tell.
    let _ = self.commands.send(command);
  }

  pub fn try_recv(&self) -> Option<Event> {
    self.events.try_recv().ok()
  }
}

/// The database must already exist (migrated) at `db_path`.
pub fn spawn(ctx: egui::Context, catalog: Arc<Catalog>, db_path: PathBuf) -> std::io::Result<Handle> {
  let (commands, command_rx) = unbounded_channel();
  let (event_tx, events) = mpsc::channel();
  let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
  thread::Builder::new().name("tracker".into()).spawn(move || {
    let events = Events { tx: event_tx, ctx };
    runtime.block_on(run(&catalog, &db_path, command_rx, &events));
  })?;
  Ok(Handle { commands, events })
}

struct Events {
  tx: mpsc::Sender<Event>,
  ctx: egui::Context,
}

impl Events {
  fn send(&self, event: Event) {
    if self.tx.send(event).is_ok() {
      self.ctx.request_repaint();
    }
  }

  fn status(&self, game_id: &str, status: &Status) {
    self.send(Event::Status(game_id.to_owned(), status.clone()));
  }
}

async fn run(catalog: &Catalog, db_path: &Path, mut commands: UnboundedReceiver<Command>, events: &Events) {
  let storage = match Storage::open(db_path) {
    Ok(storage) => storage,
    Err(_) => {
      let status = Status { problem: Some(Problem::Storage), ..Status::default() };
      for game in &GAMES {
        events.status(game.id, &status);
      }
      return;
    }
  };

  let mut slots: Vec<Slot<'_>> = Vec::new();
  for game in &GAMES {
    let Some(definition) = catalog.get(game.id) else { continue };
    let mut status = Status::default();
    let tracker = match game.api_key.map(|name| secrets::api_key(&storage, name)) {
      Some(Ok(key)) => key.and_then(|key| Tracker::new(game.id, &key)),
      Some(Err(_)) => {
        status.problem = Some(Problem::Credentials);
        None
      }
      None => None,
    };
    status.has_api_key = tracker.is_some();
    let session = match storage.account(game.id) {
      Ok(Some(account)) => start(&storage, catalog, definition, &account.account_id, &mut status, events),
      Ok(None) => None,
      Err(_) => {
        status.problem = Some(Problem::Storage);
        None
      }
    };
    if game.id == archi_lol::GAME_ID {
      status.integration = Integration::Waiting;
    }
    events.status(game.id, &status);
    slots.push(Slot { game, definition, tracker, session, poller: Poller::new(PollPolicy::default()), force_poll: false, status });
  }

  // CS2's integration, if the player turned it on: listen again (and put the file back if
  // something removed it).
  let (gsi_tx, mut gsi_rx) = unbounded_channel();
  let mut gsi = None;
  if let Some(token) = storage.setting(GSI_TOKEN_SETTING).ok().flatten()
    && let Some(slot) = slots.iter_mut().find(|slot| slot.game.id == archi_cs2::GAME_ID)
  {
    if let Some(dir) = cs2_cfg_dir()
      && !cs2_install::installed(&dir)
    {
      let _ = cs2_install::install(&dir, GSI_PORT, &token);
    }
    gsi = listen_gsi(token, &gsi_tx, &mut slot.status).await;
    events.status(slot.game.id, &slot.status);
  }

  // League of Legends: the game's own server, asked only while a match runs.
  let lol_client = LiveClient::new().ok();
  let mut lol_matches = LolMatches::default();

  let updater = Updater::new().ok();
  let install = Install::detect();
  let mut update_schedule = UpdateSchedule::new(Instant::now());
  let mut pending_update: Option<Update> = None;
  let mut next_replay_scan = Instant::now();
  loop {
    if update_schedule.due(Instant::now()) {
      let auto = storage.setting(update::AUTO_CHECK_SETTING).ok().flatten().as_deref() != Some("false");
      let checked = match &updater {
        Some(updater) if auto => check_update(updater, install, false, &mut pending_update, events).await,
        _ => true,
      };
      update_schedule.done(Instant::now(), checked);
    }

    for slot in &mut slots {
      // ADR 0001: the process list by name only, never a handle to the game.
      let running = process::any_running(slot.game.process_names).unwrap_or(false);
      if running != slot.status.game_running {
        slot.status.game_running = running;
        if !running && slot.status.integration == Integration::Receiving {
          slot.status.integration = Integration::Waiting;
        }
        events.status(slot.game.id, &slot.status);
        if !running && slot.game.id == archi_fortnite::GAME_ID {
          next_replay_scan = Instant::now() + REPLAY_AFTER_EXIT;
        }
      }
    }
    if let Some(slot) = slots.iter_mut().find(|slot| slot.game.id == archi_lol::GAME_ID) {
      let data = match (&lol_client, slot.status.game_running) {
        (Some(client), true) => client.fetch().await.ok(),
        _ => None,
      };
      if !slot.status.game_running {
        lol_matches.reset();
      }
      let integration = if data.is_some() { Integration::Receiving } else { Integration::Waiting };
      if integration != slot.status.integration {
        slot.status.integration = integration;
        events.status(slot.game.id, &slot.status);
      }
      if let Some(data) = data {
        on_live_data(&storage, catalog, slot, &mut lol_matches, &data, events);
      }
    }

    // ADR 0001: replays only while the game is closed.
    if let Some(slot) = fortnite(&mut slots)
      && !slot.status.game_running
      && Instant::now() >= next_replay_scan
      && let Some(session) = &mut slot.session
    {
      next_replay_scan = Instant::now() + REPLAY_SCAN;
      scan_replays(&storage, session, &mut slot.status, events);
    }

    for slot in &mut slots {
      let due = slot.poller.poll_due(slot.status.game_running, Instant::now());
      if (due || slot.force_poll)
        && let (Some(tracker), Some(session)) = (&slot.tracker, &mut slot.session)
      {
        slot.force_poll = false;
        poll(session, tracker, &mut slot.poller, &mut slot.status, events).await;
      }
    }

    let command = tokio::select! {
      () = tokio::time::sleep(PROCESS_CHECK) => None,
      command = commands.recv() => match command {
        Some(command) => Some(command),
        None => return, // The UI closed.
      },
      Some(body) = gsi_rx.recv() => {
        if let (Some(server), Some(slot)) = (&mut gsi, slots.iter_mut().find(|slot| slot.game.id == archi_cs2::GAME_ID)) {
          on_gsi_update(&storage, catalog, slot, server, &body, events);
        }
        None
      }
    };
    match command {
      None => {}
      Some(Command::PollNow) => slots.iter_mut().for_each(|slot| slot.force_poll = true),
      Some(Command::CheckUpdate { manual }) => match &updater {
        Some(updater) => {
          let checked = check_update(updater, install, manual, &mut pending_update, events).await;
          update_schedule.done(Instant::now(), checked);
        }
        None => events.send(Event::UpdateFailed(Problem::Update)),
      },
      Some(Command::WindowShown) => update_schedule.window_shown(Instant::now()),
      Some(Command::InstallUpdate) => {
        if let (Some(updater), Some(update)) = (&updater, &pending_update) {
          install_update(updater, update, events).await;
        }
      }
      Some(Command::DeleteReplays(files)) => {
        // ADR 0001: files are only touched while the game is closed; the UI waits for it too.
        if let Some(slot) = fortnite(&mut slots)
          && !slot.status.game_running
        {
          if let Some(dir) = replays::demos_dir() {
            delete_replays(&storage, &dir, &files);
          }
          if let Some(session) = &slot.session {
            review_replays(&storage, session.account_id(), events);
          }
        }
      }
      Some(Command::KeepReplays(files)) => {
        for file in &files {
          let _ = storage.mark_replay_reviewed(archi_fortnite::GAME_ID, &file.name, file.size);
        }
        if let Some(session) = fortnite(&mut slots).and_then(|slot| slot.session.as_ref()) {
          review_replays(&storage, session.account_id(), events);
        }
      }
      Some(Command::ScanReplays) => next_replay_scan = Instant::now(),
      Some(Command::SetIntegration { game, enabled }) => {
        let Some(slot) = slots.iter_mut().find(|slot| slot.game.id == game && game == archi_cs2::GAME_ID) else { continue };
        slot.status.problem = None;
        if enabled {
          let token = storage.setting(GSI_TOKEN_SETTING).ok().flatten().unwrap_or_else(new_token);
          match cs2_cfg_dir() {
            None => slot.status.problem = Some(Problem::NotFound),
            Some(dir) => match cs2_install::install(&dir, GSI_PORT, &token) {
              Err(_) => slot.status.problem = Some(Problem::Install),
              Ok(()) => {
                if storage.set_setting(GSI_TOKEN_SETTING, &token).is_err() {
                  slot.status.problem = Some(Problem::Storage);
                }
                if gsi.is_none() {
                  gsi = listen_gsi(token, &gsi_tx, &mut slot.status).await;
                }
              }
            },
          }
        } else {
          gsi = None;
          let removed = cs2_cfg_dir().map_or(Ok(()), |dir| cs2_install::uninstall(&dir));
          let _ = storage.delete_setting(GSI_TOKEN_SETTING);
          slot.status.integration = Integration::Off;
          if removed.is_err() {
            slot.status.problem = Some(Problem::Install);
          }
        }
        events.status(slot.game.id, &slot.status);
      }
      Some(Command::SetApiKey { game, key }) => {
        let Some(slot) = slots.iter_mut().find(|slot| slot.game.id == game) else { continue };
        let Some(credential) = slot.game.api_key else { continue };
        let key = key.trim();
        match secrets::set_api_key(credential, key) {
          Ok(()) => {
            slot.tracker = Tracker::new(slot.game.id, key);
            slot.status.has_api_key = slot.tracker.is_some();
            slot.status.problem = None;
            slot.force_poll = true;
          }
          Err(_) => slot.status.problem = Some(Problem::Credentials),
        }
        events.status(slot.game.id, &slot.status);
      }
      Some(Command::Link { game, name }) => {
        let Some(slot) = slots.iter_mut().find(|slot| slot.game.id == game) else { continue };
        let Some(api) = &slot.tracker else {
          events.send(Event::LinkFailed(game, Problem::NoApiKey));
          continue;
        };
        slot.status.busy = true;
        events.status(slot.game.id, &slot.status);
        match link(&storage, slot.game.id, api, name.trim()).await {
          Ok(account_id) => {
            slot.session = start(&storage, catalog, slot.definition, &account_id, &mut slot.status, events);
            slot.force_poll = true; // The new account's baseline.
            events.send(Event::Linked(game));
          }
          Err(problem) => events.send(Event::LinkFailed(game, problem)),
        }
        slot.status.busy = false;
        events.status(slot.game.id, &slot.status);
      }
    }
  }
}

/// Fortnite's slot: replays are Fortnite's only.
fn fortnite<'s, 'a>(slots: &'s mut [Slot<'a>]) -> Option<&'s mut Slot<'a>> {
  slots.iter_mut().find(|slot| slot.game.id == archi_fortnite::GAME_ID)
}

/// Replays the account's history and reports what it holds.
fn start<'a>(
  storage: &'a Storage,
  catalog: &'a Catalog,
  definition: &'a GameDefinition,
  account_id: &str,
  status: &mut Status,
  events: &Events,
) -> Option<Session<'a>> {
  match Session::start(storage, definition, catalog.games(), &catalog.platform, &catalog.rules, account_id) {
    Ok((session, retroactive)) => {
      if !retroactive.is_empty() {
        events.send(Event::Unlocked(retroactive));
      }
      events.send(Event::Progress(definition.game.id.clone(), session.progress().clone()));
      Some(session)
    }
    Err(_) => {
      status.problem = Some(Problem::Storage);
      None
    }
  }
}

async fn link(storage: &Storage, game_id: &str, tracker: &Tracker, name: &str) -> Result<String, Problem> {
  let account = tracker.find_account(name).await.map_err(|error| Problem::from(&error))?;
  let linked = LinkedAccount { game_id: game_id.into(), account_id: account.id, display_name: account.display_name, linked_at: unix_now() };
  storage.link_account(&linked).map_err(|_| Problem::Storage)?;
  Ok(linked.account_id)
}

async fn poll(session: &mut Session<'_>, tracker: &Tracker, poller: &mut Poller, status: &mut Status, events: &Events) {
  let game_id = session.game_id().to_owned();
  status.busy = true;
  events.status(&game_id, status);
  match session.poll(tracker).await {
    Ok(unlocks) => {
      status.problem = None;
      status.last_poll = Some(unix_now());
      if !unlocks.is_empty() {
        events.send(Event::Unlocked(unlocks));
      }
      events.send(Event::Progress(game_id.clone(), session.progress().clone()));
    }
    Err(SessionError::Track(TrackError::RateLimited { retry_after })) => {
      poller.defer(Instant::now() + retry_after.unwrap_or(RATE_LIMIT_BACKOFF));
      status.problem = Some(Problem::RateLimited);
    }
    // Retried on the next due poll.
    Err(SessionError::Track(error)) => status.problem = Some(Problem::from(&error)),
    Err(SessionError::Storage(_)) => status.problem = Some(Problem::Storage),
  }
  status.busy = false;
  events.status(&game_id, status);
}

/// When to look for a new version without being asked: right after start, every hour, sooner
/// after a failure, and whenever the window is opened. Before, a copy left in the tray only
/// looked every 6 hours, so a release could go unnoticed for most of a day.
struct UpdateSchedule {
  next: Instant,
  last: Option<Instant>,
}

impl UpdateSchedule {
  fn new(now: Instant) -> Self {
    Self { next: now + UPDATE_FIRST_CHECK, last: None }
  }

  fn due(&self, now: Instant) -> bool {
    now >= self.next
  }

  fn window_shown(&mut self, now: Instant) {
    if self.last.is_none_or(|last| now.duration_since(last) >= UPDATE_SHOW_GAP) {
      self.next = self.next.min(now);
    }
  }

  /// `ok`: the check reached GitHub.
  fn done(&mut self, now: Instant, ok: bool) {
    self.last = Some(now);
    self.next = now + if ok { UPDATE_INTERVAL } else { UPDATE_RETRY };
  }
}

/// Whether GitHub answered.
async fn check_update(updater: &Updater, install: Install, manual: bool, pending: &mut Option<Update>, events: &Events) -> bool {
  match updater.check(install).await {
    Ok(Some(update)) => {
      events.send(Event::UpdateAvailable(UpdateInfo { version: update.version.to_string(), notes: update.notes.clone(), page: update.page.clone() }));
      *pending = Some(update);
      true
    }
    Ok(None) => {
      if manual {
        events.send(Event::UpToDate);
      }
      true
    }
    // Automatic checks fail quietly (offline, GitHub down); the schedule retries soon.
    Err(_) => {
      if manual {
        events.send(Event::UpdateFailed(Problem::Update));
      }
      false
    }
  }
}

/// Downloads, verifies and starts the new version. Only on the player's click (ADR 0002).
async fn install_update(updater: &Updater, update: &Update, events: &Events) {
  let last_percent = std::cell::Cell::new(-1);
  let progress = |fraction: f32| {
    let percent = (fraction * 100.0) as i32;
    if percent != last_percent.replace(percent) {
      events.send(Event::UpdateProgress(fraction));
    }
  };
  events.send(Event::UpdateProgress(0.0));
  let result = match updater.download(update, progress).await {
    Ok(path) => update::apply(update, &path),
    Err(error) => Err(error),
  };
  match result {
    Ok(()) => events.send(Event::UpdateReady),
    Err(UpdateError::Checksum) => events.send(Event::UpdateFailed(Problem::UpdateIntegrity)),
    Err(_) => events.send(Event::UpdateFailed(Problem::Update)),
  }
}

/// Reads the replays finished since the account was linked that weren't read yet.
fn scan_replays(storage: &Storage, session: &mut Session<'_>, status: &mut Status, events: &Events) {
  let since = storage.account(archi_fortnite::GAME_ID).ok().flatten().map_or(0, |account| account.linked_at);
  // No folder yet means replays were never recorded: nothing to do.
  let found = replays::demos_dir().and_then(|dir| replays::finished(&dir, since, unix_now()).ok()).unwrap_or_default();
  let mut read_any = false;
  for candidate in found {
    if storage.replay_seen(archi_fortnite::GAME_ID, &candidate.name, candidate.size).unwrap_or(true) {
      continue;
    }
    let file = replays::read(&candidate, session.account_id());
    match session.record_replay(file, unix_now()) {
      Ok(unlocks) => {
        read_any = true;
        if !unlocks.is_empty() {
          events.send(Event::Unlocked(unlocks));
        }
      }
      Err(_) => {
        status.problem = Some(Problem::Storage);
        break;
      }
    }
  }
  if read_any {
    events.send(Event::Progress(archi_fortnite::GAME_ID.into(), session.progress().clone()));
  }
  let count = storage.replay_count(archi_fortnite::GAME_ID, session.account_id()).unwrap_or(status.replays);
  if count != status.replays {
    status.replays = count;
    events.status(archi_fortnite::GAME_ID, status);
  }
  review_replays(storage, session.account_id(), events);
}

/// Deletes the replays already read if the player asked for that, or offers them for
/// deletion. Only called while the game is closed.
fn review_replays(storage: &Storage, account_id: &str, events: &Events) {
  let Some(dir) = replays::demos_dir() else { return };
  let mut pending = Vec::new();
  for (name, size) in storage.replays_to_review(archi_fortnite::GAME_ID, account_id).unwrap_or_default() {
    if replays::present(&dir, &name, size) {
      pending.push(ReplayRef { name, size });
    } else {
      // Deleted by hand or by Fortnite itself: nothing left to ask about.
      let _ = storage.mark_replay_reviewed(archi_fortnite::GAME_ID, &name, size);
    }
  }
  if storage.setting(replays::AUTO_DELETE_SETTING).ok().flatten().as_deref() == Some("true") {
    delete_replays(storage, &dir, &pending);
    pending.clear();
  }
  events.send(Event::ReplaysToReview(pending));
}

fn delete_replays(storage: &Storage, dir: &Path, files: &[ReplayRef]) {
  for file in files {
    let gone = match replays::delete(dir, &file.name, file.size) {
      Ok(()) => true,
      // Already gone, or no longer the file that was read: don't ask about it again.
      Err(error) => matches!(error.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::InvalidInput),
    };
    // Otherwise (e.g. in use) it stays on the list and is retried.
    if gone {
      let _ = storage.mark_replay_reviewed(archi_fortnite::GAME_ID, &file.name, file.size);
    }
  }
}

fn cs2_cfg_dir() -> Option<PathBuf> {
  desktop::steam_path().and_then(|steam| cs2_install::cfg_dir(&steam))
}

/// 128 random bits as hex: `RandomState` is seeded from the OS for each instance.
fn new_token() -> String {
  (0..2).map(|_| format!("{:016x}", RandomState::new().hash_one(std::time::SystemTime::now()))).collect()
}

/// Binds the loopback port and serves CS2's posts in the background.
async fn listen_gsi(token: String, tx: &UnboundedSender<Vec<u8>>, status: &mut Status) -> Option<GsiServer> {
  match TcpListener::bind((Ipv4Addr::LOCALHOST, GSI_PORT)).await {
    Ok(listener) => {
      status.integration = Integration::Waiting;
      let task = tokio::spawn(accept_gsi(listener, tx.clone()));
      Some(GsiServer { token, task, matches: MatchTracker::default() })
    }
    Err(_) => {
      status.integration = Integration::Off;
      status.problem = Some(Problem::PortInUse);
      None
    }
  }
}

async fn accept_gsi(listener: TcpListener, tx: UnboundedSender<Vec<u8>>) {
  loop {
    if let Ok((stream, _)) = listener.accept().await {
      tokio::spawn(serve_gsi(stream, tx.clone()));
    }
  }
}

/// Minimal HTTP/1.1: each request's body goes to the worker; the reply is an empty 200.
async fn serve_gsi(stream: TcpStream, tx: UnboundedSender<Vec<u8>>) {
  let mut stream = BufReader::new(stream);
  let mut line = String::new();
  loop {
    let mut length = None;
    loop {
      line.clear();
      // Bounded, so a client can't grow a header line forever.
      match (&mut stream).take(8 * 1024).read_line(&mut line).await {
        Ok(0) | Err(_) => return,
        Ok(_) => {}
      }
      let header = line.trim_end();
      if header.is_empty() {
        break;
      }
      if let Some((name, value)) = header.split_once(':')
        && name.eq_ignore_ascii_case("content-length")
      {
        length = value.trim().parse::<usize>().ok();
      }
    }
    let Some(length) = length.filter(|length| *length <= GSI_MAX_BODY) else { return };
    let mut body = vec![0; length];
    if stream.read_exact(&mut body).await.is_err() || tx.send(body).is_err() {
      return;
    }
    if stream.get_mut().write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").await.is_err() {
      return;
    }
  }
}

/// One update from CS2: links the local player's account the first time and stores each
/// match played to the end.
fn on_gsi_update<'a>(storage: &'a Storage, catalog: &'a Catalog, slot: &mut Slot<'a>, server: &mut GsiServer, body: &[u8], events: &Events) {
  let Some(payload) = gsi::parse(body) else { return };
  if payload.token() != Some(server.token.as_str()) {
    return;
  }
  if slot.status.integration != Integration::Receiving {
    slot.status.integration = Integration::Receiving;
    events.status(slot.game.id, &slot.status);
  }
  let now = unix_now();
  // Whoever plays on this PC is the account: linked (or switched) on their first update.
  if let (Some(provider), Some(player)) = (&payload.provider, payload.local_player())
    && slot.session.as_ref().is_none_or(|session| session.account_id() != provider.steamid)
  {
    let account =
      LinkedAccount { game_id: slot.game.id.into(), account_id: provider.steamid.clone(), display_name: player.name.clone(), linked_at: now };
    if storage.link_account(&account).is_err() {
      slot.status.problem = Some(Problem::Storage);
      return events.status(slot.game.id, &slot.status);
    }
    slot.session = start(storage, catalog, slot.definition, &account.account_id, &mut slot.status, events);
    if let Some(session) = &mut slot.session
      && session.start_from_zero(now).is_err()
    {
      slot.status.problem = Some(Problem::Storage);
    }
    events.send(Event::Linked(slot.game.id.into()));
  }
  let Some(done) = server.matches.update(&payload) else { return };
  let Some(session) = &mut slot.session else { return };
  if storage.replay_seen(slot.game.id, &done.id, 0).unwrap_or(true) {
    return;
  }
  match session.record_replay(ReplayFile { file_name: done.id, file_size: 0, stats: Some(done.stats) }, now) {
    Ok(unlocks) => {
      slot.status.last_poll = Some(now);
      if !unlocks.is_empty() {
        events.send(Event::Unlocked(unlocks));
      }
      events.send(Event::Progress(slot.game.id.into(), session.progress().clone()));
    }
    Err(_) => slot.status.problem = Some(Problem::Storage),
  }
  events.status(slot.game.id, &slot.status);
}

/// One answer from League's server during a match: links the player's account the first time
/// and stores the match when it ends.
fn on_live_data<'a>(storage: &'a Storage, catalog: &'a Catalog, slot: &mut Slot<'a>, matches: &mut LolMatches, data: &GameData, events: &Events) {
  let now = unix_now();
  // Whoever plays on this PC is the account: linked (or switched) in their first match.
  if let Some(account_id) = data.account_id()
    && slot.session.as_ref().is_none_or(|session| session.account_id() != account_id)
  {
    let account = LinkedAccount { game_id: slot.game.id.into(), account_id: account_id.into(), display_name: account_id.into(), linked_at: now };
    if storage.link_account(&account).is_err() {
      slot.status.problem = Some(Problem::Storage);
      return events.status(slot.game.id, &slot.status);
    }
    slot.session = start(storage, catalog, slot.definition, &account.account_id, &mut slot.status, events);
    if let Some(session) = &mut slot.session
      && session.start_from_zero(now).is_err()
    {
      slot.status.problem = Some(Problem::Storage);
    }
    events.send(Event::Linked(slot.game.id.into()));
  }
  let Some(done) = matches.update(data, u64::try_from(now).unwrap_or(0)) else { return };
  let Some(session) = &mut slot.session else { return };
  if session.account_id() != done.account_id || storage.replay_seen(slot.game.id, &done.id, 0).unwrap_or(true) {
    return;
  }
  match session.record_replay(ReplayFile { file_name: done.id, file_size: 0, stats: Some(done.stats) }, now) {
    Ok(unlocks) => {
      slot.status.last_poll = Some(now);
      if !unlocks.is_empty() {
        events.send(Event::Unlocked(unlocks));
      }
      events.send(Event::Progress(slot.game.id.into(), session.progress().clone()));
    }
    Err(_) => slot.status.problem = Some(Problem::Storage),
  }
  events.status(slot.game.id, &slot.status);
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn updates_are_checked_at_start_hourly_and_sooner_after_a_failure() {
    let start = Instant::now();
    let mut schedule = UpdateSchedule::new(start);
    assert!(!schedule.due(start));
    assert!(schedule.due(start + UPDATE_FIRST_CHECK));

    schedule.done(start, true);
    assert!(!schedule.due(start + UPDATE_INTERVAL - Duration::from_secs(1)));
    assert!(schedule.due(start + UPDATE_INTERVAL));

    schedule.done(start, false);
    assert!(schedule.due(start + UPDATE_RETRY));
  }

  #[test]
  fn opening_the_window_checks_unless_a_check_just_ran() {
    let start = Instant::now();
    let mut schedule = UpdateSchedule::new(start);
    schedule.done(start, true);

    schedule.window_shown(start + Duration::from_secs(60));
    assert!(!schedule.due(start + Duration::from_secs(60)));

    let later = start + UPDATE_SHOW_GAP;
    schedule.window_shown(later);
    assert!(schedule.due(later));
  }

  /// Two requests on one keep-alive connection, as CS2 sends them: both bodies arrive.
  #[tokio::test]
  async fn serves_gsi_posts_on_a_kept_alive_connection() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let (tx, mut rx) = unbounded_channel();
    let server = tokio::spawn(accept_gsi(listener, tx));

    let mut client = TcpStream::connect(address).await.unwrap();
    for body in [r#"{"a":1}"#, r#"{"b":22}"#] {
      let request =
        format!("POST /cs2 HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len());
      client.write_all(request.as_bytes()).await.unwrap();
      let mut reply = [0; 38];
      client.read_exact(&mut reply).await.unwrap();
      assert!(reply.starts_with(b"HTTP/1.1 200 OK"));
      assert_eq!(rx.recv().await.unwrap(), body.as_bytes());
    }
    // A body over the limit closes the connection without being passed on.
    let huge = format!("POST /cs2 HTTP/1.1\r\nContent-Length: {}\r\n\r\n", GSI_MAX_BODY + 1);
    client.write_all(huge.as_bytes()).await.unwrap();
    assert_eq!(client.read(&mut [0; 8]).await.unwrap(), 0);
    assert!(rx.try_recv().is_err());
    server.abort();
  }

  #[test]
  fn tokens_are_random_and_long() {
    let (a, b) = (new_token(), new_token());
    assert_eq!(a.len(), 32);
    assert_ne!(a, b);
  }
}
