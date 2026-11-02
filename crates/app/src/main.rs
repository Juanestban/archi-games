//! Development console until the UI (roadmap step 4): store the API key, link
//! the account, poll and print unlocks. Its output is plain English on
//! purpose; the UI will use the Fluent tokens.

mod process;
mod session;

use std::error::Error;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use archi_core::{GameDefinition, GameTracker, PollPolicy, Poller, Progression, TrackError};
use archi_fortnite::{API_KEY_SETTING, API_KEY_URL, FortniteTracker, GAME_ID};
use archi_storage::{LinkedAccount, Storage, Unlock, unix_now};
use session::{Session, SessionError};

const USAGE: &str = "usage: archi-games [command]
  key <api-key>     save your fortnite-api.com API key
  link <epic-name>  link your Epic account
  status            level, XP and achievements
  poll              poll the API once
  run               keep polling (default)";

/// How often the process list is checked; polls follow the `PollPolicy`.
const PROCESS_CHECK: Duration = Duration::from_secs(5);

/// Wait after a rate limit that didn't say how long.
const RATE_LIMIT_BACKOFF: Duration = Duration::from_secs(5 * 60);

struct App {
  storage: Storage,
  fortnite: GameDefinition,
  platform: GameDefinition,
  rules: Progression,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().skip(1).collect();
  match run(&args).await {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("error: {error}");
      ExitCode::FAILURE
    }
  }
}

async fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
  let app = App {
    storage: Storage::open(&Storage::default_path()?)?,
    fortnite: GameDefinition::from_toml_str(include_str!("../../../games/fortnite/achievements.toml"))?,
    platform: GameDefinition::from_toml_str(include_str!("../../../games/platform/achievements.toml"))?,
    rules: Progression::default(),
  };
  let args: Vec<&str> = args.iter().map(String::as_str).collect();
  match args.as_slice() {
    ["key", key] => {
      app.storage.set_setting(API_KEY_SETTING, key.trim())?;
      println!("API key saved");
    }
    ["link", name @ ..] if !name.is_empty() => link(&app, &name.join(" ")).await?,
    ["status"] => status(&app)?,
    ["poll"] => poll_once(&app).await?,
    [] | ["run"] => watch(&app).await?,
    _ => return Err(USAGE.into()),
  }
  Ok(())
}

fn tracker(app: &App) -> Result<FortniteTracker, Box<dyn Error>> {
  let key =
    app.storage.setting(API_KEY_SETTING)?.ok_or_else(|| format!("no API key: get one at {API_KEY_URL}, then run `archi-games key <api-key>`"))?;
  Ok(FortniteTracker::new(key)?)
}

fn account_id(app: &App) -> Result<String, Box<dyn Error>> {
  let account = app.storage.account(GAME_ID)?.ok_or("no linked account: run `archi-games link <epic-name>`")?;
  Ok(account.account_id)
}

fn explain(error: TrackError) -> Box<dyn Error> {
  match error {
    TrackError::Private => "the account's stats are private: in Fortnite, turn on Settings > Account and Privacy > Show on Career Leaderboard".into(),
    error => error.into(),
  }
}

async fn link(app: &App, name: &str) -> Result<(), Box<dyn Error>> {
  let account = tracker(app)?.find_account(name).await.map_err(explain)?;
  app.storage.link_account(&LinkedAccount {
    game_id: GAME_ID.into(),
    account_id: account.id.clone(),
    display_name: account.display_name.clone(),
    linked_at: unix_now(),
  })?;
  println!("linked {} ({})", account.display_name, account.id);
  Ok(())
}

fn status(app: &App) -> Result<(), Box<dyn Error>> {
  let summary = app.storage.account_summary(&[&app.fortnite, &app.platform], &app.rules)?;
  let level = summary.level;
  println!("level {} ({}/{} XP to next, {} XP total)", level.level, level.xp_into_level, level.xp_for_next, summary.total_xp);

  let Some(account) = app.storage.account(GAME_ID)? else {
    println!("no linked account");
    return Ok(());
  };
  println!("{}: {}", app.fortnite.game.id, account.display_name);
  let (session, _) = Session::start(&app.storage, &app.fortnite, &app.platform, &app.rules, &account.account_id)?;
  for achievement in &app.fortnite.achievements {
    let mark = if session.progress().is_unlocked(&achievement.id) { "x" } else { " " };
    let progress = session.progress().progress(achievement).map(|p| format!(" {}/{}", p.current, p.target)).unwrap_or_default();
    let hidden = if achievement.hidden { " (hidden)" } else { "" };
    println!("  [{mark}] {:?} {}{progress}{hidden}", achievement.tier, achievement.id);
  }
  Ok(())
}

async fn poll_once(app: &App) -> Result<(), Box<dyn Error>> {
  let tracker = tracker(app)?;
  let (mut session, retroactive) = Session::start(&app.storage, &app.fortnite, &app.platform, &app.rules, &account_id(app)?)?;
  announce(app, &retroactive)?;
  let unlocks = session.poll(&tracker).await.map_err(|e| match e {
    SessionError::Track(e) => explain(e),
    e => e.into(),
  })?;
  if unlocks.is_empty() {
    println!("no new unlocks");
  }
  announce(app, &unlocks)
}

async fn watch(app: &App) -> Result<(), Box<dyn Error>> {
  let tracker = tracker(app)?;
  let (mut session, retroactive) = Session::start(&app.storage, &app.fortnite, &app.platform, &app.rules, &account_id(app)?)?;
  announce(app, &retroactive)?;
  println!("watching {} (Ctrl+C to stop)", app.fortnite.game.id);

  let mut poller = Poller::new(PollPolicy::default());
  let mut was_running = false;
  loop {
    let running = process::any_running(tracker.process_names()).unwrap_or_else(|error| {
      eprintln!("could not list processes: {error}");
      false
    });
    if running != was_running {
      println!("{} {}", app.fortnite.game.id, if running { "started" } else { "closed" });
      was_running = running;
    }

    let now = Instant::now();
    if poller.poll_due(running, now) {
      match session.poll(&tracker).await {
        Ok(unlocks) => announce(app, &unlocks)?,
        Err(SessionError::Track(TrackError::RateLimited { retry_after })) => {
          let wait = retry_after.unwrap_or(RATE_LIMIT_BACKOFF);
          eprintln!("rate limited, waiting {}s", wait.as_secs());
          poller.defer(now + wait);
        }
        // Network hiccups and API errors are retried on the next poll.
        Err(SessionError::Track(error)) => eprintln!("poll failed: {}", explain(error)),
        Err(error) => return Err(error.into()),
      }
    }
    tokio::time::sleep(PROCESS_CHECK).await;
  }
}

/// Prints unlocks, then the new level if it went up.
fn announce(app: &App, unlocks: &[Unlock]) -> Result<(), Box<dyn Error>> {
  if unlocks.is_empty() {
    return Ok(());
  }
  let after = app.storage.account_summary(&[&app.fortnite, &app.platform], &app.rules)?;
  let gained: u64 = unlocks.iter().map(|u| app.rules.xp_for(u.tier)).sum();
  for unlock in unlocks {
    println!("UNLOCKED [{:?}] {}/{} +{} XP", unlock.tier, unlock.game_id, unlock.achievement_id, app.rules.xp_for(unlock.tier));
  }
  if app.rules.level_for(after.total_xp.saturating_sub(gained)).level < after.level.level {
    println!("LEVEL UP -> {}", after.level.level);
  }
  Ok(())
}
