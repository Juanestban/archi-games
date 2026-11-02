//! Is a game running? Answered from the system's process list by executable
//! name only. ADR 0001: no handle to any process is ever opened, so nothing
//! here can look like tampering to an anti-cheat.

use std::io;

/// Whether a process with any of these executable names (case-insensitive)
/// is running.
#[cfg(windows)]
pub fn any_running(names: &[&str]) -> io::Result<bool> {
  use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
  use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
  };

  // A ToolHelp snapshot lists processes (pid, exe name) without opening them.
  // SAFETY: the snapshot handle is checked and closed, and `entry` is a
  // zeroed POD with `dwSize` set as the API requires.
  unsafe {
    let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if snapshot == INVALID_HANDLE_VALUE {
      return Err(io::Error::last_os_error());
    }
    let mut entry: PROCESSENTRY32W = std::mem::zeroed();
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;

    let mut found = false;
    let mut more = Process32FirstW(snapshot, &mut entry) != 0;
    while more && !found {
      let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
      let exe = String::from_utf16_lossy(&entry.szExeFile[..len]);
      found = names.iter().any(|name| exe.eq_ignore_ascii_case(name));
      more = Process32NextW(snapshot, &mut entry) != 0;
    }
    CloseHandle(snapshot);
    Ok(found)
  }
}

/// Linux and macOS come later (roadmap phase 2); until then the game counts
/// as closed and polling stays on the idle interval.
#[cfg(not(windows))]
pub fn any_running(_names: &[&str]) -> io::Result<bool> {
  Ok(false)
}

#[cfg(all(test, windows))]
mod tests {
  use super::*;

  #[test]
  fn finds_this_test_process_by_name() {
    let exe = std::env::current_exe().unwrap();
    let name = exe.file_name().unwrap().to_str().unwrap().to_uppercase();
    assert!(any_running(&[&name]).unwrap());
    assert!(!any_running(&["definitely-not-running-archi.exe"]).unwrap());
  }
}
