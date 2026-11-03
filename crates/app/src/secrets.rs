//! Each game's API key lives in the OS credential store (Windows Credential
//! Manager), not in the database, under the game's credential name
//! (e.g. `fortnite.api_key`).

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

pub fn api_key(storage: &Storage, name: &str) -> Result<Option<String>, SecretError> {
  let entry = Entry::new(SERVICE, name)?;
  match entry.get_password() {
    Ok(key) => return Ok(Some(key)),
    Err(keyring::Error::NoEntry) => {}
    Err(error) => return Err(error.into()),
  }
  // Earlier versions kept Fortnite's in the settings table under the same name: move it out.
  let Some(key) = storage.setting(name)? else { return Ok(None) };
  entry.set_password(&key)?;
  storage.delete_setting(name)?;
  Ok(Some(key))
}

pub fn set_api_key(name: &str, key: &str) -> Result<(), SecretError> {
  Ok(Entry::new(SERVICE, name)?.set_password(key)?)
}
