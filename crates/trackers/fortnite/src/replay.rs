//! Reads what a Fortnite `.replay` file says about the match: the recording player's end-of-match
//! stats, the team's placement and every knock and elimination in the match (who, whom and where).
//! Only the file's header and its *event* chunks are read; the network data (the bulk of the file,
//! where things like chests, jumps or vehicles would be) is skipped without being interpreted.
//!
//! The game writes these files itself (Settings > Game > Record Replays). Per ADR 0001 they are
//! only opened read-only, once the game has closed them. Event data is AES-256-ECB encrypted
//! with a key stored in the file's header.
//!
//! Format: Unreal's local file replay streamer, as documented by FortniteReplayDecompressor
//! (github.com/Shiqan/FortniteReplayDecompressor, MIT).

use std::collections::BTreeMap;

use aes::Aes256;
use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt, KeyInit};
use archi_core::MATCHES;
use thiserror::Error;

/// Snapshot mode the replay stats are added under (see `games/fortnite/achievements.toml`).
pub const REPLAY_MODE: &str = "replay";

const FILE_MAGIC: u32 = 0x1CA2_E27F;
const CHUNK_EVENT: u32 = 3;
const MATCH_STATS: &str = "AthenaMatchStats";
const TEAM_STATS: &str = "AthenaMatchTeamStats";
const ELIMINATION_GROUP: &str = "playerElim";
/// The elimination layout read (seen in real replays, 2026-11); other versions are skipped.
const ELIMINATION_VERSION: u32 = 10;
/// An elimination this far away (metres) counts as a long-range one.
pub const LONG_RANGE_M: f64 = 100.0;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReplayError {
  #[error("not a replay file")]
  NotAReplay,
  #[error("the replay is still being recorded")]
  StillRecording,
  #[error("the replay is truncated or malformed")]
  Malformed,
  #[error("an event could not be decrypted")]
  Decryption,
}

/// The recording player's numbers at the end of the match.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchStats {
  pub accuracy: f32,
  pub assists: u32,
  pub eliminations: u32,
  pub weapon_damage: u32,
  pub other_damage: u32,
  pub revives: u32,
  pub damage_taken: u32,
  pub damage_to_structures: u32,
  pub materials_gathered: u32,
  pub materials_used: u32,
  /// In centimetres.
  pub total_traveled: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MatchSummary {
  pub stats: Option<MatchStats>,
  /// 1 = won.
  pub placement: Option<u32>,
  pub total_players: Option<u32>,
  /// Every knock and elimination in the match, everyone's.
  pub eliminations: Vec<Elimination>,
}

/// One knock or elimination. Players are Epic account ids (lowercase hex); bots have none.
#[derive(Debug, Clone, PartialEq)]
pub struct Elimination {
  pub eliminated: Option<String>,
  pub eliminator: Option<String>,
  /// A knock (down but not out) rather than an elimination.
  pub knocked: bool,
  /// The game's weapon category number; its meaning isn't mapped yet.
  pub gun: u8,
  /// Between both players; `None` when the game didn't record a position.
  pub distance_m: Option<f64>,
}

impl MatchSummary {
  /// This match's contribution to the cumulative [`REPLAY_MODE`] stats for the player with
  /// `account_id` (their Epic id), or `None` when the replay has no end-of-match stats (left
  /// early, spectated, a creative island…).
  pub fn stats(&self, account_id: &str) -> Option<BTreeMap<&'static str, u64>> {
    let s = self.stats.as_ref()?;
    let account_id = account_id.to_ascii_lowercase();
    let mine: Vec<&Elimination> =
      self.eliminations.iter().filter(|e| e.eliminator.as_deref() == Some(&account_id) && e.eliminated.as_deref() != Some(&account_id)).collect();
    let distances: Vec<f64> = mine.iter().filter_map(|e| e.distance_m).collect();
    let knocks = mine.iter().filter(|e| e.knocked).count() as u64;
    let long_range = distances.iter().filter(|&&d| d >= LONG_RANGE_M).count() as u64;
    let longest = distances.iter().copied().fold(0.0, f64::max) as u64;
    // Centimetres, as confirmed with real replays (a 16-minute match: 325 760).
    let meters = s.total_traveled / 100;
    let won = self.placement == Some(1);
    Some(BTreeMap::from([
      (MATCHES, 1),
      ("wins", u64::from(won)),
      ("eliminations", s.eliminations.into()),
      ("assists", s.assists.into()),
      ("damage", s.weapon_damage.into()),
      ("other_damage", s.other_damage.into()),
      ("damage_taken", s.damage_taken.into()),
      ("structure_damage", s.damage_to_structures.into()),
      ("revives", s.revives.into()),
      ("materials_gathered", s.materials_gathered.into()),
      ("materials_used", s.materials_used.into()),
      ("distance_m", meters.into()),
      // Summed like the rest, so a one-match delta is that match's accuracy in percent.
      ("accuracy", (s.accuracy.clamp(0.0, 1.0) * 100.0).round() as u64),
      ("knocks", knocks),
      ("long_range_eliminations", long_range),
      // Summed like the rest, so a one-match delta is that match's longest.
      ("longest_elimination_m", longest),
    ]))
  }
}

pub fn parse(bytes: &[u8]) -> Result<MatchSummary, ReplayError> {
  let mut r = Reader { bytes, pos: 0 };
  if r.u32()? != FILE_MAGIC {
    return Err(ReplayError::NotAReplay);
  }
  let version = r.u32()?;
  if version >= 7 {
    // Custom versions: a 16-byte GUID and a 4-byte version each.
    let count = r.u32()? as usize;
    r.skip(count.checked_mul(20).ok_or(ReplayError::Malformed)?)?;
  }
  let _length_ms = r.u32()?;
  let _network_version = r.u32()?;
  let _changelist = r.u32()?;
  let _friendly_name = r.fstring()?;
  let live = r.u32()? != 0;
  if version >= 3 {
    let _timestamp = r.u64()?;
  }
  if version >= 2 {
    let _compressed = r.u32()?;
  }
  let key = if version >= 6 {
    let encrypted = r.u32()? != 0;
    let size = r.u32()? as usize;
    let key = r.take(size)?;
    encrypted.then_some(key)
  } else {
    None
  };
  if live {
    return Err(ReplayError::StillRecording);
  }
  let cipher = match key {
    Some(key) => Some(Aes256::new_from_slice(key).map_err(|_| ReplayError::Decryption)?),
    None => None,
  };

  let mut summary = MatchSummary::default();
  while !r.at_end() {
    let kind = r.u32()?;
    let size = usize::try_from(r.i32()?).map_err(|_| ReplayError::Malformed)?;
    let chunk = r.take(size)?;
    if kind != CHUNK_EVENT {
      continue;
    }
    let mut e = Reader { bytes: chunk, pos: 0 };
    let _id = e.fstring()?;
    let group = e.fstring()?;
    let metadata = e.fstring()?;
    let _start = e.u32()?;
    let _end = e.u32()?;
    let data_size = usize::try_from(e.i32()?).map_err(|_| ReplayError::Malformed)?;
    let elimination = group == ELIMINATION_GROUP;
    if metadata != MATCH_STATS && metadata != TEAM_STATS && !elimination {
      continue;
    }
    let data = e.take(data_size)?;
    let data = match &cipher {
      Some(cipher) => decrypt(cipher, data)?,
      None => data.to_vec(),
    };
    if elimination {
      // An unknown layout loses the eliminations, never the match.
      summary.eliminations.extend(parse_elimination(&data));
      continue;
    }
    let mut d = Reader { bytes: &data, pos: 0 };
    let _unknown = d.u32()?;
    if metadata == MATCH_STATS {
      summary.stats = Some(MatchStats {
        accuracy: d.f32()?,
        assists: d.u32()?,
        eliminations: d.u32()?,
        weapon_damage: d.u32()?,
        other_damage: d.u32()?,
        revives: d.u32()?,
        damage_taken: d.u32()?,
        damage_to_structures: d.u32()?,
        materials_gathered: d.u32()?,
        materials_used: d.u32()?,
        total_traveled: d.u32()?,
      });
    } else {
      summary.placement = Some(d.u32()?);
      summary.total_players = Some(d.u32()?);
    }
  }
  Ok(summary)
}

/// Version 10: a byte, both players' transforms (rotation, location and scale as doubles),
/// both players, the weapon category and whether it was a knock.
fn parse_elimination(data: &[u8]) -> Option<Elimination> {
  let mut d = Reader { bytes: data, pos: 0 };
  if d.u32().ok()? != ELIMINATION_VERSION {
    return None;
  }
  d.skip(1).ok()?;
  let eliminated_at = location(&mut d)?;
  let eliminator_at = location(&mut d)?;
  let eliminated = player(&mut d)?;
  let eliminator = player(&mut d)?;
  let gun = d.array::<1>().ok()?[0];
  let knocked = d.u32().ok()? != 0;
  // The game leaves a missing position at the origin.
  let known = |at: &[f64; 3]| at.iter().any(|&c| c != 0.0);
  let distance_m = (known(&eliminated_at) && known(&eliminator_at))
    .then(|| eliminated_at.iter().zip(eliminator_at).map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt() / 100.0);
  Some(Elimination { eliminated, eliminator, knocked, gun, distance_m })
}

/// A transform's location, in centimetres; its rotation and scale are skipped.
fn location(d: &mut Reader<'_>) -> Option<[f64; 3]> {
  d.skip(4 * 8).ok()?;
  let at = [d.f64().ok()?, d.f64().ok()?, d.f64().ok()?];
  d.skip(3 * 8).ok()?;
  at.iter().all(|c| c.is_finite()).then_some(at)
}

/// A player's Epic id; bots (named or not) have none. `None` for an unknown kind.
fn player(d: &mut Reader<'_>) -> Option<Option<String>> {
  match d.array::<1>().ok()?[0] {
    0x11 => {
      let size = usize::from(d.array::<1>().ok()?[0]);
      Some(Some(d.take(size).ok()?.iter().map(|b| format!("{b:02x}")).collect()))
    }
    0x10 => d.fstring().ok().map(|_| None),
    0x03 => Some(None),
    _ => None,
  }
}

/// AES-256-ECB with PKCS#7 padding.
fn decrypt(cipher: &Aes256, data: &[u8]) -> Result<Vec<u8>, ReplayError> {
  if data.is_empty() || !data.len().is_multiple_of(16) {
    return Err(ReplayError::Decryption);
  }
  let mut out = data.to_vec();
  for block in out.as_chunks_mut::<16>().0 {
    cipher.decrypt_block(GenericArray::from_mut_slice(block));
  }
  let pad = usize::from(*out.last().ok_or(ReplayError::Decryption)?);
  if pad == 0 || pad > 16 || out[out.len() - pad..].iter().any(|&b| usize::from(b) != pad) {
    return Err(ReplayError::Decryption);
  }
  out.truncate(out.len() - pad);
  Ok(out)
}

/// Little-endian reads over a byte slice, failing (never panicking) past its end.
struct Reader<'a> {
  bytes: &'a [u8],
  pos: usize,
}

impl<'a> Reader<'a> {
  fn at_end(&self) -> bool {
    self.pos >= self.bytes.len()
  }

  fn take(&mut self, n: usize) -> Result<&'a [u8], ReplayError> {
    let end = self.pos.checked_add(n).filter(|&end| end <= self.bytes.len()).ok_or(ReplayError::Malformed)?;
    let slice = &self.bytes[self.pos..end];
    self.pos = end;
    Ok(slice)
  }

  fn skip(&mut self, n: usize) -> Result<(), ReplayError> {
    self.take(n).map(|_| ())
  }

  fn array<const N: usize>(&mut self) -> Result<[u8; N], ReplayError> {
    Ok(self.take(N)?.try_into().expect("take returns N bytes"))
  }

  fn u32(&mut self) -> Result<u32, ReplayError> {
    Ok(u32::from_le_bytes(self.array()?))
  }

  fn i32(&mut self) -> Result<i32, ReplayError> {
    Ok(i32::from_le_bytes(self.array()?))
  }

  fn u64(&mut self) -> Result<u64, ReplayError> {
    Ok(u64::from_le_bytes(self.array()?))
  }

  fn f64(&mut self) -> Result<f64, ReplayError> {
    Ok(f64::from_le_bytes(self.array()?))
  }

  fn f32(&mut self) -> Result<f32, ReplayError> {
    Ok(f32::from_le_bytes(self.array()?))
  }

  /// Unreal's FString: a signed length counting the NUL; negative means UTF-16.
  fn fstring(&mut self) -> Result<String, ReplayError> {
    let len = self.i32()?;
    if len == 0 {
      return Ok(String::new());
    }
    let text = if len > 0 {
      String::from_utf8_lossy(self.take(len as usize)?).into_owned()
    } else {
      let units = len.unsigned_abs() as usize;
      let raw = self.take(units.checked_mul(2).ok_or(ReplayError::Malformed)?)?;
      let wide: Vec<u16> = raw.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
      String::from_utf16_lossy(&wide)
    };
    Ok(text.trim_end_matches('\0').to_owned())
  }
}

#[cfg(test)]
mod tests {
  use aes::cipher::BlockEncrypt;

  use super::*;

  const KEY: [u8; 32] = [7; 32];

  fn fstring(out: &mut Vec<u8>, s: &str) {
    out.extend(((s.len() + 1) as i32).to_le_bytes());
    out.extend(s.as_bytes());
    out.push(0);
  }

  fn encrypt(data: &[u8]) -> Vec<u8> {
    let pad = 16 - data.len() % 16;
    let mut out = data.to_vec();
    out.extend(std::iter::repeat_n(pad as u8, pad));
    let cipher = Aes256::new_from_slice(&KEY).unwrap();
    for block in out.as_chunks_mut::<16>().0 {
      cipher.encrypt_block(GenericArray::from_mut_slice(block));
    }
    out
  }

  fn event(metadata: &str, data: &[u8]) -> Vec<u8> {
    event_in(metadata, metadata, data)
  }

  fn event_in(group: &str, metadata: &str, data: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    fstring(&mut body, "evt1");
    fstring(&mut body, group);
    fstring(&mut body, metadata);
    body.extend(0u32.to_le_bytes());
    body.extend(0u32.to_le_bytes());
    let data = encrypt(data);
    body.extend((data.len() as i32).to_le_bytes());
    body.extend(data);
    let mut chunk = CHUNK_EVENT.to_le_bytes().to_vec();
    chunk.extend((body.len() as i32).to_le_bytes());
    chunk.extend(body);
    chunk
  }

  /// A version-7 replay with a network-data chunk, match stats and team stats.
  fn replay(live: bool, traveled: u32, placement: u32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(FILE_MAGIC.to_le_bytes());
    out.extend(7u32.to_le_bytes());
    out.extend(1u32.to_le_bytes());
    out.extend([0; 20]);
    out.extend(1_200_000u32.to_le_bytes());
    out.extend(2u32.to_le_bytes());
    out.extend(123u32.to_le_bytes());
    fstring(&mut out, "Unsaved Replay");
    out.extend(u32::from(live).to_le_bytes());
    out.extend(0u64.to_le_bytes());
    out.extend(1u32.to_le_bytes());
    out.extend(1u32.to_le_bytes());
    out.extend(32u32.to_le_bytes());
    out.extend(KEY);
    // Network data: skipped.
    out.extend(1u32.to_le_bytes());
    out.extend(5i32.to_le_bytes());
    out.extend([0xAA; 5]);
    let mut stats = vec![0u8; 4];
    stats.extend(0.42f32.to_le_bytes());
    for value in [2u32, 6, 1250, 80, 3, 0, 900, 3400, 1100, traveled] {
      stats.extend(value.to_le_bytes());
    }
    out.extend(event(MATCH_STATS, &stats));
    let mut team = vec![0u8; 4];
    team.extend(placement.to_le_bytes());
    team.extend(100u32.to_le_bytes());
    out.extend(event(TEAM_STATS, &team));
    out
  }

  const ME: &str = "eab1d999dcf4421d8c94d4f720f21edf";

  /// A version-10 elimination as real replays store it.
  fn elimination(eliminated: Option<&str>, eliminator: Option<&str>, at: [f64; 3], from: [f64; 3], knocked: bool) -> Vec<u8> {
    let mut out = ELIMINATION_VERSION.to_le_bytes().to_vec();
    out.push(4);
    for location in [at, from] {
      for value in [0.0, 0.0, 0.0, 1.0].into_iter().chain(location).chain([1.0; 3]) {
        out.extend(f64::to_le_bytes(value));
      }
    }
    for player in [eliminated, eliminator] {
      match player {
        Some(id) => {
          out.extend([0x11, 16]);
          out.extend((0..16).map(|i| u8::from_str_radix(&id[i * 2..i * 2 + 2], 16).unwrap()));
        }
        None => out.push(0x03),
      }
    }
    out.push(3);
    out.extend(u32::from(knocked).to_le_bytes());
    out.extend([0; 4]);
    out
  }

  #[test]
  fn counts_the_players_knocks_and_long_range_eliminations() {
    let other = "b082afd6f11e4d1aa03bc14b59c2ae6f";
    let mut bytes = replay(false, 1, 3);
    for event in [
      elimination(Some(other), Some(ME), [0.0, 12_000.0, 0.0], [100.0, 0.0, 0.0], true),
      elimination(Some(other), Some(ME), [0.0, 900.0, 0.0], [100.0, 0.0, 0.0], false),
      // No position for the eliminator: no distance.
      elimination(None, Some(ME), [5.0, 5.0, 5.0], [0.0, 0.0, 0.0], false),
      // Someone else's, and the player's own death: neither counts.
      elimination(Some(other), Some(other), [0.0, 50_000.0, 0.0], [1.0, 0.0, 0.0], false),
      elimination(Some(ME), Some(other), [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], false),
    ] {
      bytes.extend(event_in(ELIMINATION_GROUP, "versionedEvent", &event));
    }
    let summary = parse(&bytes).unwrap();
    assert_eq!(summary.eliminations.len(), 5);
    assert_eq!(summary.eliminations[0].distance_m.map(f64::round), Some(120.0));
    assert_eq!(summary.eliminations[2].distance_m, None);
    let stats = summary.stats(&ME.to_uppercase()).unwrap();
    assert_eq!(stats["knocks"], 1);
    assert_eq!(stats["long_range_eliminations"], 1);
    assert_eq!(stats["longest_elimination_m"], 120);
    // Another layout version: skipped, the match still counts.
    let mut other_version = elimination(None, Some(ME), [1.0; 3], [2.0; 3], true);
    other_version[0] = 11;
    let mut bytes = replay(false, 1, 3);
    bytes.extend(event_in(ELIMINATION_GROUP, "versionedEvent", &other_version));
    assert_eq!(parse(&bytes).unwrap().stats(ME).unwrap()["knocks"], 0);
  }

  #[test]
  fn reads_encrypted_match_and_team_stats() {
    let summary = parse(&replay(false, 3_900, 1)).unwrap();
    assert_eq!(summary.placement, Some(1));
    assert_eq!(summary.total_players, Some(100));
    let stats = summary.stats(ME).unwrap();
    assert_eq!(stats["matches"], 1);
    assert_eq!(stats["wins"], 1);
    assert_eq!(stats["eliminations"], 6);
    assert_eq!(stats["damage"], 1250);
    assert_eq!(stats["damage_taken"], 0);
    assert_eq!(stats["materials_used"], 1100);
    assert_eq!(stats["distance_m"], 39);
    assert_eq!(stats["accuracy"], 42);
  }

  #[test]
  fn centimetres_become_metres_and_losses_are_not_wins() {
    // Real replays: 325 760 cm in a top-3 match of about 16 minutes.
    let stats = parse(&replay(false, 325_760, 7)).unwrap().stats(ME).unwrap();
    assert_eq!(stats["distance_m"], 3_257);
    assert_eq!(stats["wins"], 0);
  }

  #[test]
  fn rejects_live_truncated_and_foreign_files() {
    assert_eq!(parse(&replay(true, 1, 1)), Err(ReplayError::StillRecording));
    let full = replay(false, 1, 1);
    assert_eq!(parse(&full[..full.len() - 10]), Err(ReplayError::Malformed));
    assert_eq!(parse(b"PK\x03\x04 a zip file"), Err(ReplayError::NotAReplay));
    assert_eq!(parse(&[]), Err(ReplayError::Malformed));
  }

  #[test]
  fn a_replay_without_match_stats_has_nothing_to_add() {
    let mut out = replay(false, 1, 1);
    // Keep only the header and the network chunk (drop both events).
    let header_and_network = out.len() - event(MATCH_STATS, &[0; 48]).len() - event(TEAM_STATS, &[0; 12]).len();
    out.truncate(header_and_network);
    assert_eq!(parse(&out).unwrap().stats(ME), None);
  }

  /// Lists every event of real replays, decrypted, as little-endian u32 words: what else the
  /// files hold. `REPLAY_DIR=... cargo test -p archi-fortnite dump_events -- --ignored --nocapture`.
  #[test]
  #[ignore = "needs real replay files"]
  fn dump_events() {
    let dir = std::env::var("REPLAY_DIR").expect("REPLAY_DIR");
    for entry in std::fs::read_dir(dir).unwrap() {
      let path = entry.unwrap().path();
      let bytes = std::fs::read(&path).unwrap();
      println!("== {}", path.display());
      let mut r = Reader { bytes: &bytes, pos: 0 };
      assert_eq!(r.u32().unwrap(), FILE_MAGIC);
      let version = r.u32().unwrap();
      if version >= 7 {
        let count = r.u32().unwrap() as usize;
        r.skip(count * 20).unwrap();
      }
      let (_, _, _) = (r.u32().unwrap(), r.u32().unwrap(), r.u32().unwrap());
      let _ = r.fstring().unwrap();
      let _ = r.u32().unwrap();
      if version >= 3 {
        let _ = r.u64().unwrap();
      }
      if version >= 2 {
        let _ = r.u32().unwrap();
      }
      let _ = r.u32().unwrap();
      let size = r.u32().unwrap() as usize;
      let cipher = Aes256::new_from_slice(r.take(size).unwrap()).unwrap();
      let mut kinds = BTreeMap::new();
      while !r.at_end() {
        let kind = r.u32().unwrap();
        let size = r.i32().unwrap() as usize;
        let chunk = r.take(size).unwrap();
        *kinds.entry(kind).or_insert(0) += 1;
        if kind != CHUNK_EVENT {
          continue;
        }
        let mut e = Reader { bytes: chunk, pos: 0 };
        let (id, group, metadata) = (e.fstring().unwrap(), e.fstring().unwrap(), e.fstring().unwrap());
        let (start, end) = (e.u32().unwrap(), e.u32().unwrap());
        let data_size = e.i32().unwrap() as usize;
        let data = e.take(data_size).unwrap();
        let plain = decrypt(&cipher, data).unwrap_or_else(|_| data.to_vec());
        let word = |w: &[u8]| u32::from_le_bytes(std::array::from_fn(|i| w.get(i).copied().unwrap_or(0)));
        let words: Vec<String> = plain.chunks(4).map(|w| format!("{:x}", word(w))).collect();
        let text: String = plain.iter().map(|&b| if b.is_ascii_graphic() { b as char } else { '.' }).collect();
        println!("event group={group} meta={metadata} id={id} t={start}-{end} size={}\n  words={}\n  text={text}", plain.len(), words.join(" "));
      }
      println!("chunk kinds: {kinds:?}");
    }
  }

  /// Reads real replays: `REPLAY_DIR=<folder with .replay copies> cargo test -p archi-fortnite
  /// -- --ignored --nocapture`.
  #[test]
  #[ignore = "needs real replay files"]
  fn reads_real_replays() {
    let dir = std::env::var("REPLAY_DIR").expect("REPLAY_DIR");
    for entry in std::fs::read_dir(dir).unwrap() {
      let path = entry.unwrap().path();
      let summary = parse(&std::fs::read(&path).unwrap());
      println!(
        "{}\n  {summary:?}\n  {:?}",
        path.display(),
        summary.as_ref().ok().and_then(|s| s.stats(&std::env::var("ACCOUNT_ID").unwrap_or_default()))
      );
      // A replay still being recorded can't be read yet; the app never sees one (game closed).
      if !matches!(summary, Err(ReplayError::StillRecording | ReplayError::Malformed)) {
        assert!(summary.unwrap().stats.is_some(), "no match stats in {}", path.display());
      }
    }
  }
}
