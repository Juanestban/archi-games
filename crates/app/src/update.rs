//! Self-update from the project's GitHub Releases, like ArtCraft's launcher. Within ADR 0002:
//! only this repository's releases over HTTPS, only when the player clicks "Update", and only
//! after the download matches the release's `SHA256SUMS.txt`.
//!
//! Release assets (built by `.github/workflows/release.yml`):
//! `ArchiGames-<version>-x64.msi`, `ArchiGames-<version>-x64-portable.exe`, `SHA256SUMS.txt`.

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const REPO: &str = "Juanestban/archi-games";
pub const AUTO_CHECK_SETTING: &str = "update.auto_check";
/// Passed to the new version after updating: it waits for the old one to exit.
pub const AFTER_UPDATE_ARG: &str = "--after-update";

const SUMS: &str = "SHA256SUMS.txt";
const MSI_SUFFIX: &str = "-x64.msi";
const PORTABLE_SUFFIX: &str = "-x64-portable.exe";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(u64, u64, u64);

impl Version {
  pub fn current() -> Self {
    Self::parse(env!("CARGO_PKG_VERSION")).expect("the crate version is x.y.z")
  }

  /// `x.y.z` or `vx.y.z`. Pre-releases (`-beta`…) are not offered as updates.
  pub fn parse(text: &str) -> Option<Self> {
    let mut parts = text.strip_prefix('v').unwrap_or(text).split('.').map(|part| part.parse::<u64>().ok());
    let version = Self(parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
  }
}

impl fmt::Display for Version {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}.{}.{}", self.0, self.1, self.2)
  }
}

/// How this copy was installed, which decides the asset to fetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Install {
  /// Per-user MSI in `%LOCALAPPDATA%\Programs\Archi Games`: updated by a newer MSI.
  Msi,
  /// A standalone .exe anywhere else: replaced in place.
  Portable,
}

impl Install {
  pub fn detect() -> Self {
    let installed = std::env::var_os("LOCALAPPDATA").map(|dir| Path::new(&dir).join("Programs").join("Archi Games"));
    match (std::env::current_exe(), installed) {
      (Ok(exe), Some(dir)) if exe.to_string_lossy().to_lowercase().starts_with(&dir.to_string_lossy().to_lowercase()) => Self::Msi,
      _ => Self::Portable,
    }
  }

  fn suffix(self) -> &'static str {
    match self {
      Self::Msi => MSI_SUFFIX,
      Self::Portable => PORTABLE_SUFFIX,
    }
  }
}

#[derive(Debug, Error)]
pub enum UpdateError {
  #[error("network error: {0}")]
  Network(String),
  #[error("unexpected response from GitHub: {0}")]
  BadResponse(String),
  #[error("the download doesn't match SHA256SUMS.txt")]
  Checksum,
  #[error(transparent)]
  Io(#[from] std::io::Error),
}

fn network(error: reqwest::Error) -> UpdateError {
  UpdateError::Network(error.to_string())
}

#[derive(Deserialize)]
struct GhRelease {
  tag_name: String,
  html_url: String,
  #[serde(default)]
  body: Option<String>,
  #[serde(default)]
  assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
  name: String,
  browser_download_url: String,
}

/// A newer release with an installable asset for this copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
  pub version: Version,
  pub notes: String,
  /// The release's web page.
  pub page: String,
  install: Install,
  asset_name: String,
  asset_url: String,
  sums_url: String,
}

/// The newer release worth offering, if `body` (GitHub's "latest release") is one.
fn pick(body: &[u8], current: Version, install: Install) -> Result<Option<Update>, UpdateError> {
  let release: GhRelease = serde_json::from_slice(body).map_err(|e| UpdateError::BadResponse(e.to_string()))?;
  let Some(version) = Version::parse(&release.tag_name) else { return Ok(None) };
  if version <= current {
    return Ok(None);
  }
  let find = |pred: &dyn Fn(&str) -> bool| release.assets.iter().find(|a| pred(&a.name));
  // Without its installer or checksums a release can't be installed safely: not offered.
  let (Some(asset), Some(sums)) = (find(&|name| name.ends_with(install.suffix())), find(&|name| name == SUMS)) else { return Ok(None) };
  Ok(Some(Update {
    version,
    notes: release.body.unwrap_or_default(),
    page: release.html_url,
    install,
    asset_name: asset.name.clone(),
    asset_url: asset.browser_download_url.clone(),
    sums_url: sums.browser_download_url.clone(),
  }))
}

/// The hex SHA-256 listed for `name` in a `sha256sum`-style file (`<hex>  <name>`).
fn expected_sum(sums: &str, name: &str) -> Option<String> {
  sums.lines().find_map(|line| {
    let (hash, file) = line.trim().split_once(char::is_whitespace)?;
    (file.trim().trim_start_matches('*') == name && hash.len() == 64).then(|| hash.to_ascii_lowercase())
  })
}

fn hex(bytes: &[u8]) -> String {
  bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub struct Updater {
  http: Client,
}

impl Updater {
  pub fn new() -> Result<Self, UpdateError> {
    let http =
      Client::builder().user_agent(concat!("archi-games/", env!("CARGO_PKG_VERSION"))).timeout(Duration::from_secs(60)).build().map_err(network)?;
    Ok(Self { http })
  }

  /// The newest release, if newer than this copy.
  pub async fn check(&self, install: Install) -> Result<Option<Update>, UpdateError> {
    let response = self
      .http
      .get(format!("https://api.github.com/repos/{REPO}/releases/latest"))
      .header("Accept", "application/vnd.github+json")
      .send()
      .await
      .map_err(network)?;
    match response.status() {
      // No release published yet.
      StatusCode::NOT_FOUND => Ok(None),
      status if status.is_success() => pick(&response.bytes().await.map_err(network)?, Version::current(), install),
      status => Err(UpdateError::BadResponse(format!("HTTP {status}"))),
    }
  }

  /// Downloads the installer, reporting progress (0–1), and checks it against the release's
  /// checksums. A mismatching file is deleted.
  pub async fn download(&self, update: &Update, progress: impl Fn(f32)) -> Result<PathBuf, UpdateError> {
    let sums = self.http.get(&update.sums_url).send().await.and_then(|r| r.error_for_status()).map_err(network)?.text().await.map_err(network)?;
    let expected = expected_sum(&sums, &update.asset_name).ok_or(UpdateError::Checksum)?;

    let path = match update.install {
      Install::Msi => std::env::temp_dir().join(&update.asset_name),
      // Next to the running .exe, so the swap is a rename on the same volume.
      Install::Portable => std::env::current_exe()?.with_extension("exe.new"),
    };
    let mut response = self.http.get(&update.asset_url).send().await.and_then(|r| r.error_for_status()).map_err(network)?;
    let total = response.content_length().unwrap_or(0);
    let mut file = std::fs::File::create(&path)?;
    let mut hasher = Sha256::new();
    let mut done = 0u64;
    while let Some(chunk) = response.chunk().await.map_err(network)? {
      file.write_all(&chunk)?;
      hasher.update(&chunk);
      done += chunk.len() as u64;
      if total > 0 {
        progress(done as f32 / total as f32);
      }
    }
    file.flush()?;
    drop(file);
    if hex(&hasher.finalize()) != expected {
      let _ = std::fs::remove_file(&path);
      return Err(UpdateError::Checksum);
    }
    Ok(path)
  }
}

/// Starts the new version. The caller must quit right away so its files can be replaced.
pub fn apply(update: &Update, downloaded: &Path) -> Result<(), UpdateError> {
  match update.install {
    // Per-user MSI: no administrator. LAUNCHAPP makes it start the new version when done.
    Install::Msi => {
      Command::new("msiexec").arg("/i").arg(downloaded).args(["/passive", "LAUNCHAPP=1"]).spawn()?;
    }
    // Windows lets a running .exe be renamed, not overwritten: move it aside, put the new one
    // in its place, start it. The old file is deleted on the next start.
    Install::Portable => {
      let exe = std::env::current_exe()?;
      let old = exe.with_extension("exe.old");
      let _ = std::fs::remove_file(&old);
      std::fs::rename(&exe, &old)?;
      if let Err(error) = std::fs::rename(downloaded, &exe) {
        let _ = std::fs::rename(&old, &exe);
        return Err(error.into());
      }
      Command::new(&exe).arg(AFTER_UPDATE_ARG).spawn()?;
    }
  }
  Ok(())
}

/// Removes what a portable update left behind.
pub fn clean_up() {
  if let Ok(exe) = std::env::current_exe() {
    let _ = std::fs::remove_file(exe.with_extension("exe.old"));
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn release(tag: &str, assets: &[&str]) -> Vec<u8> {
    let assets: Vec<_> =
      assets.iter().map(|name| serde_json::json!({ "name": name, "browser_download_url": format!("https://example.com/{name}") })).collect();
    serde_json::to_vec(
      &serde_json::json!({ "tag_name": tag, "html_url": "https://github.com/x/y/releases/tag/v", "body": "notes", "assets": assets }),
    )
    .unwrap()
  }

  const FULL: [&str; 3] = ["ArchiGames-0.2.0-x64.msi", "ArchiGames-0.2.0-x64-portable.exe", "SHA256SUMS.txt"];

  #[test]
  fn versions_compare_numerically() {
    assert!(Version::parse("v0.10.0").unwrap() > Version::parse("0.9.9").unwrap());
    assert_eq!(Version::parse("1.2.3").unwrap().to_string(), "1.2.3");
    assert_eq!(Version::parse("v1.2.3-beta.1"), None, "pre-releases aren't offered");
    assert_eq!(Version::parse("1.2"), None);
    assert_eq!(Version::parse("1.2.3.4"), None);
  }

  #[test]
  fn offers_only_newer_releases_with_the_right_asset() {
    let current = Version::parse("0.1.0").unwrap();
    let update = pick(&release("v0.2.0", &FULL), current, Install::Msi).unwrap().unwrap();
    assert_eq!(update.asset_name, "ArchiGames-0.2.0-x64.msi");
    let portable = pick(&release("v0.2.0", &FULL), current, Install::Portable).unwrap().unwrap();
    assert_eq!(portable.asset_name, "ArchiGames-0.2.0-x64-portable.exe");

    assert_eq!(pick(&release("v0.1.0", &FULL), current, Install::Msi).unwrap(), None, "same version");
    assert_eq!(pick(&release("v0.0.9", &FULL), current, Install::Msi).unwrap(), None, "older");
    assert_eq!(pick(&release("v0.2.0", &FULL[..2]), current, Install::Msi).unwrap(), None, "no checksums");
    assert_eq!(pick(&release("v0.2.0", &FULL[1..]), current, Install::Msi).unwrap(), None, "no MSI");
  }

  #[test]
  fn reads_sha256sum_files() {
    let hash = "a".repeat(64);
    let sums = format!("{hash}  ArchiGames-0.2.0-x64.msi\r\n{}  *ArchiGames-0.2.0-x64-portable.exe\n", "B".repeat(64));
    assert_eq!(expected_sum(&sums, "ArchiGames-0.2.0-x64.msi"), Some(hash));
    assert_eq!(expected_sum(&sums, "ArchiGames-0.2.0-x64-portable.exe"), Some("b".repeat(64)));
    assert_eq!(expected_sum(&sums, "other.exe"), None);
  }

  /// Talks to the real GitHub API: `cargo test -p archi-games -- --ignored`.
  #[tokio::test]
  #[ignore = "needs the network"]
  async fn checks_the_real_repository() {
    let update = Updater::new().unwrap().check(Install::Portable).await.unwrap();
    if let Some(update) = update {
      assert!(update.version > Version::current());
    }
  }

  #[test]
  fn hashes_as_lowercase_hex() {
    assert_eq!(hex(&Sha256::digest(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
  }
}
