//! The tracker thread: a tokio runtime that watches the game's process,
//! polls the API when due and stores unlocks. It owns its own database
//! connection; the UI talks to it through [`Command`]s and gets [`Event`]s
//! back, each one waking the UI with `request_repaint`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use archi_core::{GameProgress, PollPolicy, Poller, TrackError};
use archi_fortnite::{FortniteTracker, GAME_ID, PROCESS_NAMES};
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
  SetApiKey(String),
  Link(String),
  PollNow,
  /// `manual`: the player asked, so "up to date" and failures are reported too.
  CheckUpdate {
    manual: bool,
  },
  InstallUpdate,
}

/// A newer release, as the UI shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
  pub version: String,
  pub notes: String,
  pub page: String,
}

pub enum Event {
  Status(Status),
  Progress(GameProgress),
  /// In notification order (platinum, then meta-achievements, last).
  Unlocked(Vec<Unlock>),
  Linked,
  LinkFailed(Problem),
  UpdateAvailable(UpdateInfo),
  UpToDate,
  /// Download progress, 0–1.
  UpdateProgress(f32),
  UpdateFailed(Problem),
  /// The new version is starting: the app must quit now.
  UpdateReady,
}

#[derive(Debug, Clone, Default)]
pub struct Status {
  pub has_api_key: bool,
  pub game_running: bool,
  pub last_poll: Option<Timestamp>,
  pub busy: bool,
  pub problem: Option<Problem>,
  /// Replays read for the linked account so far.
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
  pub fn token(self) -> &'static str {
    match self {
      Self::NoApiKey => "ui-error-no-api-key",
      Self::Private => "ui-error-private",
      Self::NotFound => "ui-error-not-found",
      Self::Unauthorized => "ui-error-unauthorized",
      Self::RateLimited => "ui-error-rate-limited",
      Self::Network => "ui-error-network",
      Self::Unexpected => "ui-error-unexpected",
      Self::Storage => "ui-error-storage",
      Self::Credentials => "ui-error-credentials",
      Self::Update => "ui-error-update",
      Self::UpdateIntegrity => "ui-error-update-integrity",
    }
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

  fn status(&self, status: &Status) {
    self.send(Event::Status(status.clone()));
  }
}

async fn run(catalog: &Catalog, db_path: &Path, mut commands: UnboundedReceiver<Command>, events: &Events) {
  let mut status = Status::default();
  let storage = match Storage::open(db_path) {
    Ok(storage) => storage,
    Err(_) => {
      status.problem = Some(Problem::Storage);
      return events.status(&status);
    }
  };

  let mut tracker = match secrets::api_key(&storage) {
    Ok(key) => key.and_then(|key| FortniteTracker::new(key).ok()),
    Err(_) => {
      status.problem = Some(Problem::Credentials);
      None
    }
  };
  status.has_api_key = tracker.is_some();
  let mut session = match storage.account(GAME_ID) {
    Ok(Some(account)) => start(&storage, catalog, &account.account_id, &mut status, events),
    Ok(None) => None,
    Err(_) => {
      status.problem = Some(Problem::Storage);
      None
    }
  };
  events.status(&status);

  let mut poller = Poller::new(PollPolicy::default());
  let mut force_poll = false;
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

    // ADR 0001: the process list by name only, never a handle to the game.
    let running = process::any_running(PROCESS_NAMES).unwrap_or(false);
    if running != status.game_running {
      status.game_running = running;
      events.status(&status);
      if !running {
        next_replay_scan = Instant::now() + REPLAY_AFTER_EXIT;
      }
    }
    // ADR 0001: replays only while the game is closed.
    if !running
      && Instant::now() >= next_replay_scan
      && let Some(session) = &mut session
    {
      next_replay_scan = Instant::now() + REPLAY_SCAN;
      scan_replays(&storage, session, &mut status, events);
    }

    let due = poller.poll_due(running, Instant::now());
    if (due || force_poll)
      && let (Some(tracker), Some(session)) = (&tracker, &mut session)
    {
      force_poll = false;
      poll(session, tracker, &mut poller, &mut status, events).await;
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
      Some(Command::PollNow) => force_poll = true,
      Some(Command::CheckUpdate { manual }) => match &updater {
        Some(updater) => check_update(updater, install, manual, &mut pending_update, events).await,
        None => events.send(Event::UpdateFailed(Problem::Update)),
      },
      Some(Command::InstallUpdate) => {
        if let (Some(updater), Some(update)) = (&updater, &pending_update) {
          install_update(updater, update, events).await;
        }
      }
      Some(Command::SetApiKey(key)) => {
        let key = key.trim();
        match secrets::set_api_key(key) {
          Ok(()) => {
            tracker = FortniteTracker::new(key).ok();
            status.has_api_key = tracker.is_some();
            status.problem = None;
            force_poll = true;
          }
          Err(_) => status.problem = Some(Problem::Credentials),
        }
        events.status(&status);
      }
      Some(Command::Link(name)) => {
        let Some(api) = &tracker else {
          events.send(Event::LinkFailed(Problem::NoApiKey));
          continue;
        };
        status.busy = true;
        events.status(&status);
        match link(&storage, api, name.trim()).await {
          Ok(account_id) => {
            session = start(&storage, catalog, &account_id, &mut status, events);
            force_poll = true; // The new account's baseline.
            events.send(Event::Linked);
          }
          Err(problem) => events.send(Event::LinkFailed(problem)),
        }
        status.busy = false;
        events.status(&status);
      }
    }
  }
}

/// Replays the account's history and reports what it holds.
fn start<'a>(storage: &'a Storage, catalog: &'a Catalog, account_id: &str, status: &mut Status, events: &Events) -> Option<Session<'a>> {
  match Session::start(storage, &catalog.fortnite, &catalog.platform, &catalog.rules, account_id) {
    Ok((session, retroactive)) => {
      if !retroactive.is_empty() {
        events.send(Event::Unlocked(retroactive));
      }
      events.send(Event::Progress(session.progress().clone()));
      Some(session)
    }
    Err(_) => {
      status.problem = Some(Problem::Storage);
      None
    }
  }
}

async fn link(storage: &Storage, tracker: &FortniteTracker, name: &str) -> Result<String, Problem> {
  use archi_core::GameTracker;

  let account = tracker.find_account(name).await.map_err(|error| Problem::from(&error))?;
  let linked = LinkedAccount { game_id: GAME_ID.into(), account_id: account.id, display_name: account.display_name, linked_at: unix_now() };
  storage.link_account(&linked).map_err(|_| Problem::Storage)?;
  Ok(linked.account_id)
}

async fn poll(session: &mut Session<'_>, tracker: &FortniteTracker, poller: &mut Poller, status: &mut Status, events: &Events) {
  status.busy = true;
  events.status(status);
  match session.poll(tracker).await {
    Ok(unlocks) => {
      status.problem = None;
      status.last_poll = Some(unix_now());
      if !unlocks.is_empty() {
        events.send(Event::Unlocked(unlocks));
      }
      events.send(Event::Progress(session.progress().clone()));
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
  events.status(status);
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
  let since = storage.account(GAME_ID).ok().flatten().map_or(0, |account| account.linked_at);
  // No folder yet means replays were never recorded: nothing to do.
  let found = replays::demos_dir().and_then(|dir| replays::finished(&dir, since, unix_now()).ok()).unwrap_or_default();
  let mut read_any = false;
  for candidate in found {
    if storage.replay_seen(GAME_ID, &candidate.name, candidate.size).unwrap_or(true) {
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
    events.send(Event::Progress(session.progress().clone()));
  }
  let count = storage.replay_count(GAME_ID, session.account_id()).unwrap_or(status.replays);
  if count != status.replays {
    status.replays = count;
    events.status(status);
  }
}
