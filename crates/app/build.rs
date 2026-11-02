//! Embeds the app icon and version info in the Windows executable. The icon comes from the
//! same drawing the window and tray use (`src/icon.rs`). Named, versioned executables with an
//! icon also look less suspicious to antivirus heuristics (ADR 0001).

#[path = "src/icon.rs"]
#[allow(dead_code)]
mod icon;

fn main() {
  println!("cargo::rerun-if-changed=src/icon.rs");
  if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
    return;
  }
  let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
  let ico = out.join("archi-games.ico");
  std::fs::write(&ico, ico_bytes(&[16, 24, 32, 48, 64, 256])).expect("write the icon");

  let mut resource = winresource::WindowsResource::new();
  resource.set_icon(ico.to_str().expect("UTF-8 path"));
  resource.set("ProductName", "Archi Games");
  resource.set("FileDescription", "Archi Games");
  resource.set("CompanyName", "Archi Games");
  resource.set("LegalCopyright", "Archi Games");
  // Without the Windows SDK's resource compiler the exe just has no icon; still builds.
  if let Err(error) = resource.compile() {
    println!("cargo::warning=executable icon not embedded: {error}");
  }
}

/// An .ico holding one 32-bit BMP image per size.
fn ico_bytes(sizes: &[u32]) -> Vec<u8> {
  let images: Vec<Vec<u8>> = sizes.iter().map(|&size| bmp_image(size)).collect();
  let mut out = Vec::new();
  out.extend([0, 0, 1, 0]);
  out.extend((sizes.len() as u16).to_le_bytes());
  let mut offset = 6 + 16 * sizes.len() as u32;
  for (&size, image) in sizes.iter().zip(&images) {
    let side = if size >= 256 { 0 } else { size as u8 };
    out.extend([side, side, 0, 0]);
    out.extend(1u16.to_le_bytes()); // planes
    out.extend(32u16.to_le_bytes()); // bits per pixel
    out.extend((image.len() as u32).to_le_bytes());
    out.extend(offset.to_le_bytes());
    offset += image.len() as u32;
  }
  images.into_iter().for_each(|image| out.extend(image));
  out
}

/// BITMAPINFOHEADER, bottom-up BGRA pixels, then an all-zero AND mask (alpha does the work).
fn bmp_image(size: u32) -> Vec<u8> {
  let rgba = icon::rgba(size);
  let mask_row = size.div_ceil(32) * 4;
  let mut out = Vec::new();
  out.extend(40u32.to_le_bytes());
  out.extend((size as i32).to_le_bytes());
  out.extend((size as i32 * 2).to_le_bytes()); // image + mask
  out.extend(1u16.to_le_bytes());
  out.extend(32u16.to_le_bytes());
  out.extend([0; 24]); // no compression; sizes and palette left at 0
  for y in (0..size).rev() {
    for x in 0..size {
      let p = &rgba[((y * size + x) * 4) as usize..][..4];
      out.extend([p[2], p[1], p[0], p[3]]);
    }
  }
  out.extend(std::iter::repeat_n(0, (mask_row * size) as usize));
  out
}
