//! Archi Games: the egui window on the main thread, the tracker on its own
//! thread (see [`worker`]).

// Release builds are a GUI app: no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod catalog;
mod i18n;
mod process;
mod secrets;
mod session;
mod sound;
mod ui;
mod worker;

use std::error::Error;
use std::process::ExitCode;
use std::sync::Arc;

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
  let catalog = Arc::new(Catalog::bundled()?);
  let db_path = Storage::default_path()?;
  // Opened (and migrated) before the worker opens its own connection.
  let storage = Storage::open(&db_path)?;

  let options = eframe::NativeOptions {
    viewport: egui::ViewportBuilder::default().with_title("Archi Games").with_inner_size([1040.0, 700.0]).with_min_inner_size([820.0, 560.0]),
    ..Default::default()
  };
  eframe::run_native(
    "Archi Games",
    options,
    Box::new(move |cc| {
      let worker = worker::spawn(cc.egui_ctx.clone(), Arc::clone(&catalog), db_path)?;
      Ok(Box::new(ui::ArchiApp::new(cc, catalog, storage, worker)))
    }),
  )?;
  Ok(())
}
