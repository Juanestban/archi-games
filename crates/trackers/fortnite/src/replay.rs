//! Reads what a Fortnite `.replay` file says about the match: the recording player's end-of-match
//! stats and the team's placement. Only the file's header and its *event* chunks are read; the
//! network data (the bulk of the file) is skipped without being interpreted.
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
  /// As stored by the game; see [`MatchSummary::stats`] for the unit.
  pub total_traveled: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MatchSummary {
  pub stats: Option<MatchStats>,
  /// 1 = won.
  pub placement: Option<u32>,
  pub total_players: Option<u32>,
}

impl MatchSummary {
  /// This match's contribution to the cumulative [`REPLAY_MODE`] stats, or `None` when the
  /// replay has no end-of-match stats (left early, spectated, a creative island…).
  pub fn stats(&self) -> Option<BTreeMap<&'static str, u64>> {
    let s = self.stats.as_ref()?;
    // No match covers 50 km: a bigger value can only be centimetres.
    let meters = if s.total_traveled > 50_000 { s.total_traveled / 100 } else { s.total_traveled };
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
    let _group = e.fstring()?;
    let metadata = e.fstring()?;
    let _start = e.u32()?;
    let _end = e.u32()?;
    let data_size = usize::try_from(e.i32()?).map_err(|_| ReplayError::Malformed)?;
    if metadata != MATCH_STATS && metadata != TEAM_STATS {
      continue;
    }
    let data = e.take(data_size)?;
    let data = match &cipher {
      Some(cipher) => decrypt(cipher, data)?,
      None => data.to_vec(),
    };
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
    let mut body = Vec::new();
    fstring(&mut body, "evt1");
    fstring(&mut body, metadata);
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

  #[test]
  fn reads_encrypted_match_and_team_stats() {
    let summary = parse(&replay(false, 3_900, 1)).unwrap();
    assert_eq!(summary.placement, Some(1));
    assert_eq!(summary.total_players, Some(100));
    let stats = summary.stats().unwrap();
    assert_eq!(stats["matches"], 1);
    assert_eq!(stats["wins"], 1);
    assert_eq!(stats["eliminations"], 6);
    assert_eq!(stats["damage"], 1250);
    assert_eq!(stats["damage_taken"], 0);
    assert_eq!(stats["materials_used"], 1100);
    assert_eq!(stats["distance_m"], 3_900);
    assert_eq!(stats["accuracy"], 42);
  }

  #[test]
  fn centimetres_become_metres_and_losses_are_not_wins() {
    let stats = parse(&replay(false, 390_000, 7)).unwrap().stats().unwrap();
    assert_eq!(stats["distance_m"], 3_900);
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
    assert_eq!(parse(&out).unwrap().stats(), None);
  }
}
