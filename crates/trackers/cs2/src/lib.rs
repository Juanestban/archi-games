//! Counter-Strike 2 through Valve's Game State Integration (GSI): with a config file in CS2's
//! `cfg` folder, the game itself POSTs its state as JSON to a local URL while you play. Nothing
//! reads or touches the game's process (ADR 0001); the game sends what Valve allows.
//!
//! Steam's Web API isn't used: its CS2 counters are CS:GO's, frozen since CS2 came out (checked
//! with a real account on 2026-11-03).

pub mod gsi;
pub mod install;

pub const GAME_ID: &str = "cs2";

/// The game itself; Steam alone doesn't mean a match.
pub const PROCESS_NAMES: &[&str] = &["cs2.exe"];

/// Where the app listens for CS2's updates: loopback only.
pub const GSI_PORT: u16 = 43117;
