//! Archi Games: the egui window on the main thread, the tracker on its own
//! thread (see [`worker`]).

// Release builds are a GUI app: no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod catalog;
mod desktop;
mod i18n;
mod icon;
mod process;
mod secrets;
mod session;
mod sound;
mod tray;
mod ui;
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
  // A second launch (e.g. the Start menu while it sits in the tray) shows the
  // running window instead.
  let Some(instance) = desktop::SingleInstance::acquire("ArchiGames")? else { return Ok(()) };
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
