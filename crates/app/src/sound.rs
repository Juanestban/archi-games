//! Unlock sounds, synthesized at runtime so the app ships no audio files. The aim is the
//! feeling of a console trophy (a soft breath of air, then a crystalline chime that rings out
//! into a wide, airy tail) with original notes: richer the higher the tier, a slow bloom
//! for a platinum and a quick upward run for leveling up.

use std::f32::consts::TAU;
use std::num::NonZero;
use std::sync::{Arc, OnceLock};

use archi_core::Tier;
use rodio::buffer::SamplesBuffer;
use rodio::mixer::Mixer;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Source};

const SAMPLE_RATE: u32 = 44_100;
const RATE: f32 = SAMPLE_RATE as f32;

/// Settings key for the volume, 0–100.
pub const VOLUME_SETTING: &str = "sound.volume";
pub const DEFAULT_VOLUME: u8 = 70;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
  Tier(Tier),
  LevelUp,
}

const CUES: [Cue; 5] = [Cue::Tier(Tier::Bronze), Cue::Tier(Tier::Silver), Cue::Tier(Tier::Gold), Cue::Tier(Tier::Platinum), Cue::LevelUp];

/// Rendered cues (interleaved stereo), built once in the background.
static RENDERED: OnceLock<Vec<Arc<[f32]>>> = OnceLock::new();

fn rendered(cue: Cue) -> Arc<[f32]> {
  let all = RENDERED.get_or_init(|| CUES.iter().map(|&cue| samples(cue).into()).collect());
  Arc::clone(&all[CUES.iter().position(|&c| c == cue).expect("every cue is rendered")])
}

/// The default output device, if there is one. Without it sounds are silently skipped.
pub struct Sounds {
  sink: Option<MixerDeviceSink>,
}

impl Sounds {
  pub fn new() -> Self {
    let sink = DeviceSinkBuilder::open_default_sink().ok().map(|mut sink| {
      sink.log_on_drop(false);
      sink
    });
    // Rendering takes a moment (reverb tails); do it before the first unlock.
    std::thread::spawn(|| rendered(Cue::LevelUp));
    Self { sink }
  }

  /// `volume` in 0–100. Never blocks the UI: mixing in happens on a short-lived thread.
  pub fn play(&self, cue: Cue, volume: u8) {
    let Some(sink) = &self.sink else { return };
    if volume == 0 {
      return;
    }
    let mixer: Mixer = sink.mixer().clone();
    let gain = f32::from(volume.min(100)) / 100.0;
    std::thread::spawn(move || {
      let samples = rendered(cue).to_vec();
      let buffer = SamplesBuffer::new(NonZero::new(2).expect("stereo"), NonZero::new(SAMPLE_RATE).expect("non-zero rate"), samples);
      mixer.add(buffer.amplify(gain));
    });
  }
}

/// A struck glass-like bell: inharmonic partials, the high ones dying first.
struct Chime {
  freq: f32,
  start: f32,
  /// Seconds for the fundamental to fall by about 60 dB.
  ring: f32,
  gain: f32,
}

/// A soft swell of detuned sines under the chimes (the "air").
struct Pad {
  freqs: &'static [f32],
  start: f32,
  attack: f32,
  length: f32,
  gain: f32,
}

// Note frequencies (Hz).
const D4: f32 = 293.66;
const A4: f32 = 440.0;
const D5: f32 = 587.33;
const E5: f32 = 659.25;
const FS5: f32 = 739.99;
const A5: f32 = 880.0;
const B5: f32 = 987.77;
const CS6: f32 = 1108.73;
const D6: f32 = 1174.66;
const E6: f32 = 1318.51;
const FS6: f32 = 1479.98;
const A6: f32 = 1760.0;
const D7: f32 = 2349.32;

fn chime(freq: f32, start: f32, ring: f32, gain: f32) -> Chime {
  Chime { freq, start, ring, gain }
}

/// (chimes, pads, reverb tail seconds)
fn score(cue: Cue) -> (Vec<Chime>, Vec<Pad>, f32) {
  match cue {
    // One bright ping with an octave shimmer.
    Cue::Tier(Tier::Bronze) => (
      vec![chime(A5, 0.06, 1.1, 0.9), chime(A6, 0.06, 0.5, 0.18)],
      vec![Pad { freqs: &[A4, E5], start: 0.0, attack: 0.06, length: 0.5, gain: 0.10 }],
      1.4,
    ),
    // Up a fifth: a little more lift.
    Cue::Tier(Tier::Silver) => (
      vec![chime(D6, 0.06, 1.0, 0.75), chime(A6, 0.16, 1.3, 0.85), chime(D7, 0.16, 0.5, 0.12)],
      vec![Pad { freqs: &[D5, A5], start: 0.0, attack: 0.08, length: 0.7, gain: 0.10 }],
      1.7,
    ),
    // A sparkling arpeggio landing on a bright top note.
    Cue::Tier(Tier::Gold) => (
      vec![
        chime(FS5, 0.05, 0.9, 0.55),
        chime(A5, 0.12, 0.9, 0.6),
        chime(D6, 0.19, 1.0, 0.65),
        chime(FS6, 0.27, 1.6, 0.9),
        chime(D7, 0.27, 0.7, 0.15),
      ],
      vec![Pad { freqs: &[D4, A4, FS5], start: 0.0, attack: 0.12, length: 1.0, gain: 0.12 }],
      2.2,
    ),
    // A slow bloom, a rising cascade and a held chord ringing out.
    Cue::Tier(Tier::Platinum) => (
      vec![
        chime(D6, 0.35, 0.9, 0.45),
        chime(FS6, 0.43, 0.9, 0.45),
        chime(A6, 0.51, 0.9, 0.5),
        chime(D7, 0.59, 1.2, 0.55),
        chime(A5, 0.85, 2.6, 0.7),
        chime(D6, 0.85, 2.6, 0.7),
        chime(FS6, 0.85, 2.6, 0.6),
        chime(A6, 0.85, 2.2, 0.45),
      ],
      vec![
        Pad { freqs: &[D4, A4, E5, FS5], start: 0.0, attack: 0.45, length: 1.4, gain: 0.16 },
        Pad { freqs: &[D5, A5], start: 0.85, attack: 0.05, length: 2.0, gain: 0.10 },
      ],
      3.2,
    ),
    // A quick run up that pops at the top.
    Cue::LevelUp => (
      vec![chime(A5, 0.0, 0.5, 0.5), chime(CS6, 0.06, 0.5, 0.5), chime(E6, 0.12, 0.5, 0.55), chime(A6, 0.18, 1.4, 0.85), chime(B5, 0.18, 0.8, 0.2)],
      vec![Pad { freqs: &[A4, E5], start: 0.0, attack: 0.04, length: 0.45, gain: 0.08 }],
      1.6,
    ),
  }
}

/// Bell partials (ratio, relative gain, how much faster than the fundamental it decays).
const PARTIALS: [(f32, f32, f32); 5] = [(1.0, 1.0, 1.0), (2.0, 0.45, 1.8), (2.76, 0.30, 2.6), (5.40, 0.12, 4.0), (8.93, 0.05, 6.0)];

/// Interleaved stereo samples of a cue, peaking at 0.8.
pub fn samples(cue: Cue) -> Vec<f32> {
  let (chimes, pads, tail) = score(cue);
  let dry_end = chimes.iter().map(|c| c.start + c.ring).chain(pads.iter().map(|p| p.start + p.length)).fold(0.0, f32::max);
  let frames = ((dry_end + tail) * RATE) as usize;
  let mut left = vec![0.0; frames];
  let mut right = vec![0.0; frames];

  for c in &chimes {
    // The two channels sit a few cents apart: a wide, shimmering image.
    for (channel, detune) in [(&mut left, 0.9985_f32), (&mut right, 1.0015)] {
      let first = (c.start * RATE) as usize;
      let count = ((c.ring * RATE) as usize).min(frames.saturating_sub(first));
      for (i, out) in channel[first..first + count].iter_mut().enumerate() {
        let t = i as f32 / RATE;
        let strike = (t / 0.002).min(1.0);
        let mut v = 0.0;
        for (ratio, gain, speed) in PARTIALS {
          let decay = (-t * 6.9 * speed / c.ring).exp();
          v += gain * decay * (TAU * c.freq * ratio * detune * t).sin();
        }
        *out += c.gain * strike * v;
      }
    }
  }

  for pad in &pads {
    let first = (pad.start * RATE) as usize;
    let count = ((pad.length * RATE) as usize).min(frames.saturating_sub(first));
    for i in 0..count {
      let t = i as f32 / RATE;
      // Rises slowly, then fades out over its length.
      let envelope = (t / pad.attack).min(1.0).powi(2) * (1.0 - t / pad.length).max(0.0).powi(2);
      for (k, &freq) in pad.freqs.iter().enumerate() {
        let wobble = 1.0 + 0.002 * (TAU * 0.7 * t + k as f32).sin();
        let l = (TAU * freq * 0.999 * wobble * t).sin();
        let r = (TAU * freq * 1.001 * wobble * t).sin();
        left[first + i] += pad.gain * envelope * l;
        right[first + i] += pad.gain * envelope * r;
      }
    }
  }

  let left = reverb(&left, 0);
  let right = reverb(&right, 23);
  let mut out: Vec<f32> = left.iter().zip(&right).flat_map(|(l, r)| [*l, *r]).collect();
  let peak = out.iter().fold(0.0_f32, |max, s| max.max(s.abs()));
  if peak > 0.0 {
    out.iter_mut().for_each(|s| *s *= 0.8 / peak);
  }
  // Fade the very end so the tail never clicks off.
  let fade = (0.05 * RATE) as usize * 2;
  let len = out.len();
  for (i, s) in out[len.saturating_sub(fade)..].iter_mut().enumerate() {
    *s *= 1.0 - i as f32 / (fade - 1) as f32;
  }
  out
}

/// A small Schroeder reverb (four damped combs into two all-passes), mixed with the dry signal.
/// `spread` offsets the delays so left and right decorrelate.
fn reverb(dry: &[f32], spread: usize) -> Vec<f32> {
  const COMBS: [usize; 4] = [1557, 1617, 1491, 1422];
  const ALLPASSES: [usize; 2] = [225, 556];
  let mut wet = vec![0.0; dry.len()];
  for delay in COMBS.map(|d| d + spread) {
    let mut buffer = vec![0.0; delay];
    let mut damped = 0.0;
    for (i, out) in wet.iter_mut().enumerate() {
      let j = i % delay;
      let delayed = buffer[j];
      damped = delayed * 0.6 + damped * 0.4;
      buffer[j] = dry[i] + damped * 0.86;
      *out += delayed * 0.25;
    }
  }
  for delay in ALLPASSES.map(|d| d + spread) {
    let mut buffer = vec![0.0; delay];
    for (i, sample) in wet.iter_mut().enumerate() {
      let j = i % delay;
      let delayed = buffer[j];
      let input = *sample;
      buffer[j] = input + delayed * 0.5;
      *sample = delayed - input * 0.5;
    }
  }
  dry.iter().zip(wet).map(|(d, w)| d * 0.75 + w * 0.45).collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn cues_are_stereo_audible_and_never_clip() {
    for cue in CUES {
      let samples = samples(cue);
      let peak = samples.iter().fold(0.0_f32, |max, s| max.max(s.abs()));
      assert!(samples.len().is_multiple_of(2), "{cue:?} is interleaved stereo");
      assert!((0.79..=0.81).contains(&peak), "{cue:?} peaks at {peak}");
      assert_eq!(samples.last().copied(), Some(0.0), "{cue:?} fades to silence");
    }
  }

  #[test]
  fn platinum_is_the_longest() {
    let platinum = samples(Cue::Tier(Tier::Platinum)).len();
    for tier in [Tier::Bronze, Tier::Silver, Tier::Gold] {
      assert!(samples(Cue::Tier(tier)).len() < platinum);
    }
  }
}
