//! Windows integration: one instance at a time and starting with Windows. Within ADR 0001:
//! named kernel objects scoped to this user's session, and the per-user `Run` key, never
//! anything that needs administrator rights.

/// The flag the `Run` entry passes, so a start with Windows opens straight to the tray.
pub const BACKGROUND_ARG: &str = "--background";

#[cfg(windows)]
mod imp {
  use std::io;
  use std::ptr;

  use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, GetLastError, HANDLE, WAIT_OBJECT_0};
  use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW};
  use windows_sys::Win32::System::Threading::{CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, INFINITE, OpenEventW, SetEvent, WaitForSingleObject};

  fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
  }

  /// Held for the app's lifetime; a second launch finds it and hands over instead.
  pub struct SingleInstance {
    mutex: HANDLE,
    show: HANDLE,
  }

  // SAFETY: kernel handles may be used from any thread.
  unsafe impl Send for SingleInstance {}
  unsafe impl Sync for SingleInstance {}

  impl SingleInstance {
    /// `None` if another instance already runs; it has been asked to show its window.
    #[cfg(test)]
    pub fn acquire(name: &str) -> io::Result<Option<Self>> {
      Self::acquire_quietly(name, true)
    }

    /// Like `acquire`, but only asks the running instance to show itself if `hand_over`.
    pub fn acquire_quietly(name: &str, hand_over: bool) -> io::Result<Option<Self>> {
      let mutex_name = wide(&format!("Local\\{name}.Instance"));
      let show_name = wide(&format!("Local\\{name}.Show"));
      // SAFETY: valid NUL-terminated names; handles are checked and owned by `Self`.
      unsafe {
        let mutex = CreateMutexW(ptr::null(), 0, mutex_name.as_ptr());
        if mutex.is_null() {
          return Err(io::Error::last_os_error());
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
          CloseHandle(mutex);
          let show = if hand_over { OpenEventW(EVENT_MODIFY_STATE, 0, show_name.as_ptr()) } else { ptr::null_mut() };
          if !show.is_null() {
            SetEvent(show);
            CloseHandle(show);
          }
          return Ok(None);
        }
        // Auto-reset: each signal wakes the waiting thread once.
        let show = CreateEventW(ptr::null(), 0, 0, show_name.as_ptr());
        if show.is_null() {
          let error = io::Error::last_os_error();
          CloseHandle(mutex);
          return Err(error);
        }
        Ok(Some(Self { mutex, show }))
      }
    }

    /// Calls `on_show` (on a background thread) each time another launch asks for the window.
    pub fn on_show(&self, on_show: impl Fn() + Send + 'static) {
      let show = self.show as usize;
      std::thread::spawn(move || {
        loop {
          // SAFETY: the event outlives this thread; `Self` lives as long as the app.
          if unsafe { WaitForSingleObject(show as HANDLE, INFINITE) } != WAIT_OBJECT_0 {
            return;
          }
          on_show();
        }
      });
    }
  }

  impl Drop for SingleInstance {
    fn drop(&mut self) {
      // SAFETY: owned handles, closed once.
      unsafe {
        CloseHandle(self.show);
        CloseHandle(self.mutex);
      }
    }
  }

  const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

  pub fn read_value(key: &str, name: &str) -> io::Result<Option<String>> {
    let (key, name) = (wide(key), wide(name));
    let mut buffer = vec![0u16; 1024];
    let mut bytes = (buffer.len() * 2) as u32;
    // SAFETY: the buffer and its size in bytes match.
    let status =
      unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, ptr::null_mut(), buffer.as_mut_ptr().cast(), &mut bytes) };
    match status {
      0 => {
        let len = (bytes as usize / 2).saturating_sub(1);
        Ok(Some(String::from_utf16_lossy(&buffer[..len])))
      }
      ERROR_FILE_NOT_FOUND => Ok(None),
      code => Err(io::Error::from_raw_os_error(code as i32)),
    }
  }

  pub fn write_value(key: &str, name: &str, value: &str) -> io::Result<()> {
    let (key, name, value) = (wide(key), wide(name), wide(value));
    // SAFETY: NUL-terminated strings; the size includes the terminator, in bytes.
    let status = unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), REG_SZ, value.as_ptr().cast(), (value.len() * 2) as u32) };
    if status == 0 { Ok(()) } else { Err(io::Error::from_raw_os_error(status as i32)) }
  }

  pub fn delete_value(key: &str, name: &str) -> io::Result<()> {
    let (key, name) = (wide(key), wide(name));
    // SAFETY: NUL-terminated strings.
    match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) } {
      0 | ERROR_FILE_NOT_FOUND => Ok(()),
      code => Err(io::Error::from_raw_os_error(code as i32)),
    }
  }

  pub fn autostart_enabled(entry: &str) -> bool {
    autostart_entry(entry).is_some()
  }

  pub fn autostart_entry(entry: &str) -> Option<String> {
    read_value(RUN_KEY, entry).ok().flatten()
  }

  pub fn set_autostart(entry: &str, command: Option<&str>) -> io::Result<()> {
    match command {
      Some(command) => write_value(RUN_KEY, entry, command),
      None => delete_value(RUN_KEY, entry),
    }
  }
}

#[cfg(not(windows))]
mod imp {
  //! Linux and macOS come later (roadmap phase 2).
  use std::io;

  pub struct SingleInstance;

  impl SingleInstance {
    pub fn acquire_quietly(_name: &str, _hand_over: bool) -> io::Result<Option<Self>> {
      Ok(Some(Self))
    }

    pub fn on_show(&self, _on_show: impl Fn() + Send + 'static) {}
  }

  pub fn autostart_enabled(_entry: &str) -> bool {
    false
  }

  pub fn autostart_entry(_entry: &str) -> Option<String> {
    None
  }

  pub fn set_autostart(_entry: &str, _command: Option<&str>) -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "autostart is Windows-only for now"))
  }
}

pub use imp::{SingleInstance, autostart_enabled, set_autostart};

/// The `Run` entry's name, as Task Manager's Startup tab shows it.
pub const AUTOSTART_ENTRY: &str = "Archi Games";

/// The MSI runs the installed copy with this while uninstalling (wix/main.wxs).
pub const UNINSTALL_ARG: &str = "--uninstall";

/// The command the `Run` entry holds: this executable, quoted, opening to the tray.
pub fn autostart_command() -> std::io::Result<String> {
  Ok(format!("\"{}\" {BACKGROUND_ARG}", std::env::current_exe()?.display()))
}

/// On uninstall: drops the `Run` entry, but only if it starts this copy (a portable copy
/// may own it instead).
pub fn forget_autostart() -> std::io::Result<()> {
  match imp::autostart_entry(AUTOSTART_ENTRY) {
    Some(entry) if starts_this_copy(&entry, &autostart_command()?) => set_autostart(AUTOSTART_ENTRY, None),
    _ => Ok(()),
  }
}

/// Windows paths ignore case.
fn starts_this_copy(entry: &str, own: &str) -> bool {
  entry.trim().eq_ignore_ascii_case(own)
}

#[cfg(all(test, windows))]
mod tests {
  use super::imp::{delete_value, read_value, write_value};
  use super::*;

  #[test]
  fn a_second_instance_is_turned_away() {
    let name = format!("ArchiGamesTest{}", std::process::id());
    let first = SingleInstance::acquire(&name).unwrap();
    assert!(first.is_some());
    assert!(SingleInstance::acquire(&name).unwrap().is_none());
    drop(first);
    assert!(SingleInstance::acquire(&name).unwrap().is_some(), "free again once the first exits");
  }

  #[test]
  fn registry_values_round_trip() {
    // A throwaway key of our own, never the real Run key.
    let key = format!(r"Software\ArchiGamesTest{}", std::process::id());
    assert_eq!(read_value(&key, "entry").unwrap(), None);
    write_value(&key, "entry", r#""C:\Program Files\Archi Games\archi-games.exe" --background"#).unwrap();
    assert_eq!(read_value(&key, "entry").unwrap().as_deref(), Some(r#""C:\Program Files\Archi Games\archi-games.exe" --background"#));
    delete_value(&key, "entry").unwrap();
    assert_eq!(read_value(&key, "entry").unwrap(), None);
    let wide: Vec<u16> = key.encode_utf16().chain([0]).collect();
    // SAFETY: NUL-terminated key name; removes the empty test key.
    unsafe { windows_sys::Win32::System::Registry::RegDeleteKeyW(windows_sys::Win32::System::Registry::HKEY_CURRENT_USER, wide.as_ptr()) };
  }

  #[test]
  fn autostart_command_quotes_the_path() {
    let command = autostart_command().unwrap();
    assert!(command.starts_with('"') && command.ends_with("\" --background"));
  }

  #[test]
  fn uninstall_only_forgets_its_own_entry() {
    let own = r#""C:\Users\a\AppData\Local\Programs\Archi Games\archi-games.exe" --background"#;
    assert!(starts_this_copy(r#""c:\users\a\appdata\local\programs\archi games\archi-games.exe" --background "#, own));
    assert!(!starts_this_copy(r#""D:\Tools\ArchiGames-0.3.0-x64-portable.exe" --background"#, own));
  }
}
