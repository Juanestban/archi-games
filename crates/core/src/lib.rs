//! Game-agnostic achievement engine: definitions, evaluation, XP and levels.
//!
//! Nothing in this crate knows about any specific game. Trackers turn game
//! data into [`StatSnapshot`]s and the engine decides what gets unlocked.

pub mod definition;
pub mod engine;
pub mod poll;
pub mod progression;
pub mod stats;
pub mod tracker;

pub use definition::{AchievementDef, Condition, DefinitionError, GameDefinition, Predicate, Tier};
pub use engine::{GameProgress, Progress};
pub use poll::{PollPolicy, Poller};
pub use progression::{LevelInfo, Progression};
pub use stats::{MATCHES, MODE_ALL, StatSnapshot};
pub use tracker::{AccountInfo, GameTracker, TrackError};
