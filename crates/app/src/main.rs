//! Archi Games: the egui window on the main thread, the tracker on its own
//! thread (see [`worker`]).

// Release builds are a GUI app: no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod catalog;
mod desktop;
mod i18n;
mod icon;
mod process;
mod replays;
mod secrets;
mod session;
mod sound;
mod tray;
mod ui;
mod update;
mod worker;

use std::error::Error;
use std::process::ExitCode;
use std::sync::{Arc, mpsc};

use archi_storage::Storage;
use catalog::Catalog;
use eframe::egui;

fn main() -> ExitCode {
  match run() {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("error: {error}");
      ExitCode::FAILURE
    }
  }
}

fn run() -> Result<(), Box<dyn Error>> {
  // Run by the MSI while uninstalling: no window, and a running copy doesn't matter.
  if std::env::args().any(|arg| arg == desktop::UNINSTALL_ARG) {
    return Ok(desktop::forget_autostart()?);
  }
  // A second launch (e.g. the Start menu while it sits in the tray) shows the
  // running window instead.
  update::clean_up();
  let after_update = std::env::args().any(|arg| arg == update::AFTER_UPDATE_ARG);
  let Some(instance) = acquire_instance(after_update)? else { return Ok(()) };
  // Started with Windows: straight to the tray.
  let background = std::env::args().any(|arg| arg == desktop::BACKGROUND_ARG);
  let catalog = Arc::new(Catalog::bundled()?);
  let db_path = Storage::default_path()?;
  // Opened (and migrated) before the worker opens its own connection.
  let storage = Storage::open(&db_path)?;

  let options = eframe::NativeOptions {
    viewport: egui::ViewportBuilder::default()
      .with_title("Archi Games")
      .with_inner_size([1120.0, 740.0])
      .with_min_inner_size([880.0, 580.0])
      // The title bar is drawn by the app (ui/chrome.rs).
      .with_decorations(false)
      .with_icon(Arc::new(egui::IconData { rgba: icon::rgba(64), width: 64, height: 64 }))
      .with_visible(!background),
    ..Default::default()
  };
  eframe::run_native(
    "Archi Games",
    options,
    Box::new(move |cc| {
      let worker = worker::spawn(cc.egui_ctx.clone(), Arc::clone(&catalog), db_path)?;
      let (shell, shell_events) = mpsc::channel();
      let (ctx, show) = (cc.egui_ctx.clone(), shell.clone());
      instance.on_show(move || {
        if show.send(tray::ShellEvent::Show).is_ok() {
          ctx.request_repaint();
        }
      });
      Ok(Box::new(ui::ArchiApp::new(cc, ui::Parts { catalog, storage, worker, shell, shell_events, instance, start_hidden: background })))
    }),
  )?;
  Ok(())
}

/// Right after an update the old version may still be closing: wait for it rather than hand
/// over to it.
fn acquire_instance(after_update: bool) -> std::io::Result<Option<desktop::SingleInstance>> {
  let attempts = if after_update { 60 } else { 1 };
  for attempt in 1..=attempts {
    if let Some(instance) = desktop::SingleInstance::acquire_quietly("ArchiGames", attempt == attempts)? {
      return Ok(Some(instance));
    }
    std::thread::sleep(std::time::Duration::from_millis(250));
  }
  Ok(None)
}
