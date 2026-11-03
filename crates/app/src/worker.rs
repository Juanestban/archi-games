//! The tracker thread: a tokio runtime that watches each game's process,
//! polls its API when due and stores unlocks. It owns its own database
//! connection; the UI talks to it through [`Command`]s and gets [`Event`]s
//! back, each one waking the UI with `request_repaint`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use archi_core::{AccountInfo, GameDefinition, GameProgress, GameTracker, PollPolicy, Poller, StatSnapshot, TrackError};
use archi_cs2::Cs2Tracker;
use archi_fortnite::FortniteTracker;
use archi_storage::{LinkedAccount, Storage, Timestamp, Unlock, unix_now};
use eframe::egui;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::catalog::Catalog;
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

/// First automatic update check after start, then the interval between checks.
const UPDATE_FIRST_CHECK: Duration = Duration::from_secs(15);
const UPDATE_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

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
  /// The player's answer about replays already read: delete them or keep them.
  DeleteReplays(Vec<ReplayRef>),
  KeepReplays(Vec<ReplayRef>),
  /// Look for replays now, e.g. right after turning automatic deletion on.
  ScanReplays,
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
  Unexpected,
  Storage,
  Credentials,
  Update,
  /// The download didn't match the release's checksums; nothing was installed.
  UpdateIntegrity,
}

impl Problem {
  /// The message's token. Private profiles, unknown accounts and network errors are worded
  /// per game (`<game>-error-…`); `game_id` is ignored for the rest.
  pub fn token(self, game_id: &str) -> String {
    let token = match self {
      Self::Private => return format!("{game_id}-error-private"),
      Self::NotFound => return format!("{game_id}-error-not-found"),
      Self::Network => return format!("{game_id}-error-network"),
      Self::NoApiKey => "ui-error-no-api-key",
      Self::Unauthorized => "ui-error-unauthorized",
      Self::RateLimited => "ui-error-rate-limited",
      Self::Unexpected => "ui-error-unexpected",
      Self::Storage => "ui-error-storage",
      Self::Credentials => "ui-error-credentials",
      Self::Update => "ui-error-update",
      Self::UpdateIntegrity => "ui-error-update-integrity",
    };
    token.to_owned()
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
      TrackError::Unexpected(_) => Self::Unexpected,
    }
  }
}

/// A game the app can track: how to recognise it and how to reach its API.
struct Game {
  id: &'static str,
  process_names: &'static [&'static str],
  /// The credential holding the player's API key.
  api_key: &'static str,
}

const GAMES: [Game; 2] = [
  Game { id: archi_fortnite::GAME_ID, process_names: archi_fortnite::PROCESS_NAMES, api_key: archi_fortnite::API_KEY_SETTING },
  Game { id: archi_cs2::GAME_ID, process_names: archi_cs2::PROCESS_NAMES, api_key: archi_cs2::API_KEY_SETTING },
];

/// `GameTracker` isn't object-safe (async methods), so the trackers are an enum.
enum Tracker {
  Fortnite(FortniteTracker),
  Cs2(Cs2Tracker),
}

impl Tracker {
  fn new(game_id: &str, key: &str) -> Option<Self> {
    match game_id {
      archi_fortnite::GAME_ID => FortniteTracker::new(key).ok().map(Self::Fortnite),
      archi_cs2::GAME_ID => Cs2Tracker::new(key).ok().map(Self::Cs2),
      _ => None,
    }
  }
}

impl GameTracker for Tracker {
  fn game_id(&self) -> &str {
    match self {
      Self::Fortnite(tracker) => tracker.game_id(),
      Self::Cs2(tracker) => tracker.game_id(),
    }
  }

  fn process_names(&self) -> &[&str] {
    match self {
      Self::Fortnite(tracker) => tracker.process_names(),
      Self::Cs2(tracker) => tracker.process_names(),
    }
  }

  async fn find_account(&self, name: &str) -> Result<AccountInfo, TrackError> {
    match self {
      Self::Fortnite(tracker) => tracker.find_account(name).await,
      Self::Cs2(tracker) => tracker.find_account(name).await,
    }
  }

  async fn fetch(&self, account_id: &str) -> Result<StatSnapshot, TrackError> {
    match self {
      Self::Fortnite(tracker) => tracker.fetch(account_id).await,
      Self::Cs2(tracker) => tracker.fetch(account_id).await,
    }
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
    let tracker = match secrets::api_key(&storage, game.api_key) {
      Ok(key) => key.and_then(|key| Tracker::new(game.id, &key)),
      Err(_) => {
        status.problem = Some(Problem::Credentials);
        None
      }
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
    events.status(game.id, &status);
    slots.push(Slot { game, definition, tracker, session, poller: Poller::new(PollPolicy::default()), force_poll: false, status });
  }

  let updater = Updater::new().ok();
  let install = Install::detect();
  let mut next_update_check = Instant::now() + UPDATE_FIRST_CHECK;
  let mut pending_update: Option<Update> = None;
  let mut next_replay_scan = Instant::now();
  loop {
    if Instant::now() >= next_update_check {
      next_update_check = Instant::now() + UPDATE_INTERVAL;
      let auto = storage.setting(update::AUTO_CHECK_SETTING).ok().flatten().as_deref() != Some("false");
      if auto && let Some(updater) = &updater {
        check_update(updater, install, false, &mut pending_update, events).await;
      }
    }

    for slot in &mut slots {
      // ADR 0001: the process list by name only, never a handle to the game.
      let running = process::any_running(slot.game.process_names).unwrap_or(false);
      if running != slot.status.game_running {
        slot.status.game_running = running;
        events.status(slot.game.id, &slot.status);
        if !running && slot.game.id == archi_fortnite::GAME_ID {
          next_replay_scan = Instant::now() + REPLAY_AFTER_EXIT;
        }
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
    };
    match command {
      None => {}
      Some(Command::PollNow) => slots.iter_mut().for_each(|slot| slot.force_poll = true),
      Some(Command::CheckUpdate { manual }) => match &updater {
        Some(updater) => check_update(updater, install, manual, &mut pending_update, events).await,
        None => events.send(Event::UpdateFailed(Problem::Update)),
      },
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
      Some(Command::SetApiKey { game, key }) => {
        let Some(slot) = slots.iter_mut().find(|slot| slot.game.id == game) else { continue };
        let key = key.trim();
        match secrets::set_api_key(slot.game.api_key, key) {
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

async fn check_update(updater: &Updater, install: Install, manual: bool, pending: &mut Option<Update>, events: &Events) {
  match updater.check(install).await {
    Ok(Some(update)) => {
      events.send(Event::UpdateAvailable(UpdateInfo { version: update.version.to_string(), notes: update.notes.clone(), page: update.page.clone() }));
      *pending = Some(update);
    }
    Ok(None) if manual => events.send(Event::UpToDate),
    Ok(None) => {}
    // Automatic checks fail quietly (offline, GitHub down); the next one retries.
    Err(_) if manual => events.send(Event::UpdateFailed(Problem::Update)),
    Err(_) => {}
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
    match session.record_replay(replays::read(&candidate), unix_now()) {
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
