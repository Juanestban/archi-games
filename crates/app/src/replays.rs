//! Finds Fortnite's finished replays on this PC. ADR 0001: only files the game has closed
//! (it isn't running and they haven't changed for a while), only read, never written.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use archi_fortnite::replay;
use archi_storage::{ReplayFile, Timestamp};

/// A file this recent may still be being written.
const SETTLE_SECS: Timestamp = 30;

/// `%LOCALAPPDATA%\FortniteGame\Saved\Demos`, where Fortnite saves replays on PC.
pub fn demos_dir() -> Option<PathBuf> {
  std::env::var_os("LOCALAPPDATA").map(|dir| Path::new(&dir).join("FortniteGame").join("Saved").join("Demos"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
  pub path: PathBuf,
  pub name: String,
  pub size: u64,
  pub modified: Timestamp,
}

/// `.replay` files modified after `since` and settled by `now`, oldest first.
pub fn finished(dir: &Path, since: Timestamp, now: Timestamp) -> io::Result<Vec<Candidate>> {
  let mut found = Vec::new();
  for entry in std::fs::read_dir(dir)? {
    let entry = entry?;
    let path = entry.path();
    if path.extension().is_none_or(|ext| !ext.eq_ignore_ascii_case("replay")) {
      continue;
    }
    let meta = entry.metadata()?;
    let modified = meta.modified()?.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as Timestamp);
    if meta.is_file() && modified >= since && now - modified >= SETTLE_SECS {
      found.push(Candidate { name: entry.file_name().to_string_lossy().into_owned(), size: meta.len(), modified, path });
    }
  }
  found.sort_by_key(|c| (c.modified, c.name.clone()));
  Ok(found)
}

/// What a file adds; an unreadable file adds nothing (and is still marked as read).
pub fn read(candidate: &Candidate) -> ReplayFile {
  let stats = std::fs::read(&candidate.path)
    .ok()
    .and_then(|bytes| replay::parse(&bytes).ok())
    .and_then(|summary| summary.stats())
    .map(|stats| stats.into_iter().map(|(stat, value)| (stat.to_owned(), value)).collect::<BTreeMap<_, _>>());
  ReplayFile { file_name: candidate.name.clone(), file_size: candidate.size, stats }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn finds_settled_replays_since_a_time_oldest_first() {
    let dir = std::env::temp_dir().join(format!("archi-replays-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["b.replay", "a.REPLAY", "notes.txt"] {
      std::fs::write(dir.join(name), b"x").unwrap();
    }
    let now = std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as Timestamp;

    assert!(finished(&dir, 0, now).unwrap().is_empty(), "just written: not settled yet");
    let later = finished(&dir, 0, now + 60).unwrap();
    assert_eq!(later.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["a.REPLAY", "b.replay"]);
    assert!(finished(&dir, now + 10, now + 60).unwrap().is_empty(), "older than the link");

    // Not a real replay: read as nothing to add, not an error.
    assert_eq!(read(&later[0]).stats, None);
    std::fs::remove_dir_all(&dir).unwrap();
  }
}
