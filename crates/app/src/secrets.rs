//! The fortnite-api.com key lives in the OS credential store (Windows
//! Credential Manager), not in the database.

use archi_fortnite::API_KEY_SETTING;
use archi_storage::{Storage, StorageError};
use keyring::Entry;
use thiserror::Error;

const SERVICE: &str = "archi-games";

#[derive(Debug, Error)]
pub enum SecretError {
  #[error(transparent)]
  Keyring(#[from] keyring::Error),
  #[error(transparent)]
  Storage(#[from] StorageError),
}

pub fn api_key(storage: &Storage) -> Result<Option<String>, SecretError> {
  let entry = entry()?;
  match entry.get_password() {
    Ok(key) => return Ok(Some(key)),
    Err(keyring::Error::NoEntry) => {}
    Err(error) => return Err(error.into()),
  }
  // Earlier versions kept it in the settings table: move it out.
  let Some(key) = storage.setting(API_KEY_SETTING)? else { return Ok(None) };
  entry.set_password(&key)?;
  storage.delete_setting(API_KEY_SETTING)?;
  Ok(Some(key))
}

pub fn set_api_key(key: &str) -> Result<(), SecretError> {
  Ok(entry()?.set_password(key)?)
}

fn entry() -> keyring::Result<Entry> {
  Entry::new(SERVICE, API_KEY_SETTING)
}
