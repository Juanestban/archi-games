//! Finding CS2's `cfg` folder and writing (or removing) the GSI config file there, the only file
//! the app writes in a game's folder, and only when the player turns the integration on.

use std::io;
use std::path::{Path, PathBuf};

/// CS2 reads every `gamestate_integration_*.cfg` in its `cfg` folder at startup.
pub const CFG_FILE: &str = "gamestate_integration_archigames.cfg";

/// CS2 kept CS:GO's Steam app id.
const APP_ID: &str = "730";

/// `<library>\steamapps\common\Counter-Strike Global Offensive\game\csgo\cfg` for the Steam
/// library holding CS2, from Steam's install folder (`SteamPath` in the registry).
pub fn cfg_dir(steam: &Path) -> Option<PathBuf> {
  let folders = std::fs::read_to_string(steam.join("steamapps").join("libraryfolders.vdf")).ok();
  let library = folders.as_deref().and_then(library_with_app).map(PathBuf::from).unwrap_or_else(|| steam.to_owned());
  let dir = library.join("steamapps").join("common").join("Counter-Strike Global Offensive").join("game").join("csgo").join("cfg");
  dir.is_dir().then_some(dir)
}

/// The `"path"` of the library listing app 730 in `libraryfolders.vdf`.
fn library_with_app(vdf: &str) -> Option<String> {
  let mut path = None;
  for line in vdf.lines() {
    let fields: Vec<&str> = line.split('"').skip(1).step_by(2).collect();
    match fields.as_slice() {
      ["path", value] => path = Some(value.replace("\\\\", "\\")),
      [APP_ID, _] => return path,
      _ => {}
    }
  }
  None
}

/// What the game reads: where to post, the token it must send back, and the data wanted.
pub fn cfg_contents(port: u16, token: &str) -> String {
  format!(
    r#""Archi Games"
{{
  "uri"       "http://127.0.0.1:{port}/cs2"
  "timeout"   "5.0"
  "buffer"    "0.5"
  "throttle"  "0.5"
  "heartbeat" "60.0"
  "auth"
  {{
    "token" "{token}"
  }}
  "data"
  {{
    "provider"           "1"
    "map"                "1"
    "player_id"          "1"
    "player_state"       "1"
    "player_weapons"     "1"
    "player_match_stats" "1"
  }}
}}
"#
  )
}

pub fn install(dir: &Path, port: u16, token: &str) -> io::Result<()> {
  std::fs::write(dir.join(CFG_FILE), cfg_contents(port, token))
}

/// Removes the file; fine if it was already gone.
pub fn uninstall(dir: &Path) -> io::Result<()> {
  match std::fs::remove_file(dir.join(CFG_FILE)) {
    Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
    result => result,
  }
}

pub fn installed(dir: &Path) -> bool {
  dir.join(CFG_FILE).is_file()
}

#[cfg(test)]
mod tests {
  use super::*;

  const FOLDERS: &str = r#""libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"apps"
		{
			"228980"		"361634936"
		}
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
		"apps"
		{
			"1422450"		"9000"
			"730"		"38720421883"
		}
	}
}"#;

  #[test]
  fn finds_the_library_that_has_cs2() {
    assert_eq!(library_with_app(FOLDERS).as_deref(), Some(r"D:\SteamLibrary"));
    assert_eq!(library_with_app(r#""path" "C:\\Steam" "apps" { "570" "1" }"#), None);
  }

  #[test]
  fn writes_and_removes_the_config() {
    let dir = std::env::temp_dir().join(format!("archi-cs2-cfg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    assert!(!installed(&dir));
    install(&dir, 43117, "abc").unwrap();
    let written = std::fs::read_to_string(dir.join(CFG_FILE)).unwrap();
    assert!(written.contains(r#""uri"       "http://127.0.0.1:43117/cs2""#));
    assert!(written.contains(r#""token" "abc""#));
    uninstall(&dir).unwrap();
    uninstall(&dir).unwrap();
    assert!(!installed(&dir));
    std::fs::remove_dir_all(&dir).unwrap();
  }
}
