//! Unlock sounds, synthesized at runtime (bell-like tones), so the app ships
//! no audio files. One cue per tier, richer the higher the tier, plus one
//! for leveling up.

use std::f32::consts::TAU;
use std::num::NonZero;

use archi_core::Tier;
use rodio::buffer::SamplesBuffer;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Source};

const SAMPLE_RATE: u32 = 44_100;

/// Settings key for the volume, 0–100.
pub const VOLUME_SETTING: &str = "sound.volume";
pub const DEFAULT_VOLUME: u8 = 70;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
  Tier(Tier),
  LevelUp,
}

/// The default output device, if there is one. Without it sounds are
/// silently skipped.
pub struct Sounds {
  sink: Option<MixerDeviceSink>,
}

impl Sounds {
  pub fn new() -> Self {
    let sink = DeviceSinkBuilder::open_default_sink().ok().map(|mut sink| {
      sink.log_on_drop(false);
      sink
    });
    Self { sink }
  }

  /// `volume` in 0–100.
  pub fn play(&self, cue: Cue, volume: u8) {
    let Some(sink) = &self.sink else { return };
    if volume == 0 {
      return;
    }
    let buffer = SamplesBuffer::new(NonZero::<u16>::MIN, NonZero::new(SAMPLE_RATE).expect("non-zero rate"), samples(cue));
    sink.mixer().add(buffer.amplify(f32::from(volume.min(100)) / 100.0));
  }
}

/// (frequency Hz, start s, length s)
type Note = (f32, f32, f32);

const C5: f32 = 523.25;
const E5: f32 = 659.25;
const G5: f32 = 783.99;
const C6: f32 = 1046.5;
const E6: f32 = 1318.51;
const G6: f32 = 1567.98;

fn notes(cue: Cue) -> &'static [Note] {
  match cue {
    Cue::Tier(Tier::Bronze) => &[(G5, 0.0, 0.5)],
    Cue::Tier(Tier::Silver) => &[(E5, 0.0, 0.4), (G5, 0.12, 0.6)],
    Cue::Tier(Tier::Gold) => &[(C5, 0.0, 0.4), (E5, 0.1, 0.4), (G5, 0.2, 0.5), (C6, 0.3, 0.8)],
    Cue::Tier(Tier::Platinum) => &[(C5, 0.0, 0.4), (E5, 0.1, 0.4), (G5, 0.2, 0.4), (C6, 0.3, 0.5), (C6, 0.5, 1.6), (E6, 0.5, 1.6), (G6, 0.5, 1.6)],
    Cue::LevelUp => &[(G5, 0.0, 0.25), (C6, 0.08, 0.25), (E6, 0.16, 0.3), (G6, 0.24, 0.7)],
  }
}

/// Mono samples of a cue, peaking at 0.8.
pub fn samples(cue: Cue) -> Vec<f32> {
  let notes = notes(cue);
  let end = notes.iter().map(|&(_, start, length)| start + length).fold(0.0, f32::max);
  let rate = SAMPLE_RATE as f32;
  let mut out = vec![0.0; (end * rate) as usize];
  for &(freq, start, length) in notes {
    let first = (start * rate) as usize;
    let count = (length * rate) as usize;
    for (i, sample) in out.iter_mut().skip(first).take(count).enumerate() {
      let t = i as f32 / rate;
      // 5 ms attack, then exponential decay.
      let envelope = (t / 0.005).min(1.0) * (-t * 4.0 / length).exp();
      let phase = TAU * freq * t;
      *sample += envelope * (phase.sin() + 0.35 * (2.0 * phase).sin() + 0.12 * (3.0 * phase).sin());
    }
  }
  let peak = out.iter().fold(0.0_f32, |max, s| max.max(s.abs()));
  if peak > 0.0 {
    out.iter_mut().for_each(|s| *s *= 0.8 / peak);
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn cues_are_audible_without_clipping() {
    for cue in [Cue::Tier(Tier::Bronze), Cue::Tier(Tier::Silver), Cue::Tier(Tier::Gold), Cue::Tier(Tier::Platinum), Cue::LevelUp] {
      let samples = samples(cue);
      let peak = samples.iter().fold(0.0_f32, |max, s| max.max(s.abs()));
      assert!(!samples.is_empty(), "{cue:?}");
      assert!((0.79..=0.81).contains(&peak), "{cue:?} peaks at {peak}");
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
