//! The app icon (a white trophy on a brand-blue rounded square), drawn in code so there is no
//! image file to keep in sync. Used by the window, the tray and, through `build.rs`, the .exe.

/// RGBA pixels, `size`×`size`, anti-aliased by 4×4 supersampling.
pub fn rgba(size: u32) -> Vec<u8> {
  const SAMPLES: u32 = 4;
  let mut pixels = Vec::with_capacity((size * size * 4) as usize);
  for y in 0..size {
    for x in 0..size {
      let (mut background, mut trophy) = (0.0, 0.0);
      for sy in 0..SAMPLES {
        for sx in 0..SAMPLES {
          // Sample position in 0..24 icon units.
          let u = (x as f32 + (sx as f32 + 0.5) / SAMPLES as f32) / size as f32 * 24.0;
          let v = (y as f32 + (sy as f32 + 0.5) / SAMPLES as f32) / size as f32 * 24.0;
          if in_rounded_square(u, v) {
            background += 1.0;
            if in_trophy(u, v) {
              trophy += 1.0;
            }
          }
        }
      }
      let n = (SAMPLES * SAMPLES) as f32;
      let (background, trophy) = (background / n, trophy / n);
      // Blue (#2d81ff) under white, both over transparency.
      let blue = [0x2d as f32, 0x81 as f32, 0xff as f32];
      let mix = |c: f32| if background > 0.0 { (c * (background - trophy) + 255.0 * trophy) / background } else { 0.0 };
      pixels.extend([mix(blue[0]) as u8, mix(blue[1]) as u8, mix(blue[2]) as u8, (background * 255.0).round() as u8]);
    }
  }
  pixels
}

fn in_rounded_square(u: f32, v: f32) -> bool {
  const R: f32 = 5.0;
  let dx = (u - 12.0).abs() - (12.0 - R);
  let dy = (v - 12.0).abs() - (12.0 - R);
  dx.max(0.0).hypot(dy.max(0.0)) <= R
}

fn in_trophy(u: f32, v: f32) -> bool {
  // Bowl: a rim at the top narrowing into a half circle.
  let bowl = ((5.0..=10.0).contains(&v) && (u - 12.0).abs() <= 5.0) || (v > 10.0 && (u - 12.0).hypot(v - 10.0) <= 5.0);
  // Handles: rings either side of the bowl's top.
  let ring = |cx: f32| (1.6..=2.6).contains(&(u - cx).hypot(v - 8.0));
  let handles = (ring(7.0) && u < 7.0) || (ring(17.0) && u > 17.0);
  let stem = (u - 12.0).abs() <= 1.0 && (14.5..=17.5).contains(&v);
  let base = (u - 12.0).abs() <= 4.0 && (17.5..=19.5).contains(&v);
  bowl || handles || stem || base
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn corners_are_transparent_and_the_cup_is_white() {
    let size = 32;
    let pixels = rgba(size);
    assert_eq!(pixels.len(), (size * size * 4) as usize);
    let at = |x: u32, y: u32| &pixels[((y * size + x) * 4) as usize..][..4];
    assert_eq!(at(0, 0)[3], 0, "rounded corner");
    assert_eq!(at(16, 10), [255, 255, 255, 255], "inside the bowl");
    assert_eq!(at(3, 28), [0x2d, 0x81, 0xff, 255], "background");
  }
}
