//! The tray icon: the app lives here while the window is closed. Clicking it opens the window;
//! its menu opens it, checks now or quits. Events reach the UI through a channel and wake it,
//! even while the window is hidden.

use std::sync::mpsc::Sender;

use eframe::egui;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellEvent {
  Show,
  PollNow,
  Quit,
}

const SHOW: &str = "show";
const POLL: &str = "poll";
const QUIT: &str = "quit";

/// Menu texts, already translated: open, check now, quit.
pub struct Labels {
  pub show: String,
  pub poll: String,
  pub quit: String,
}

pub struct Tray {
  _icon: TrayIcon,
  show: MenuItem,
  poll: MenuItem,
  quit: MenuItem,
}

impl Tray {
  /// Must run on the thread that owns the event loop (the main thread).
  pub fn new(ctx: &egui::Context, events: Sender<ShellEvent>, labels: &Labels) -> Result<Self, Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(SHOW, &labels.show, true, None);
    let poll = MenuItem::with_id(POLL, &labels.poll, true, None);
    let quit = MenuItem::with_id(QUIT, &labels.quit, true, None);
    let menu = Menu::with_items(&[&show, &poll, &PredefinedMenuItem::separator(), &quit])?;

    let size = 32;
    let icon = tray_icon::Icon::from_rgba(crate::icon::rgba(size), size, size).expect("icon pixels match its size");
    let tray = TrayIconBuilder::new().with_icon(icon).with_tooltip("Archi Games").with_menu(Box::new(menu)).with_menu_on_left_click(false).build()?;

    let wake = |ctx: &egui::Context, events: &Sender<ShellEvent>, event| {
      if events.send(event).is_ok() {
        ctx.request_repaint();
      }
    };
    let (menu_ctx, menu_events) = (ctx.clone(), events.clone());
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
      let shell = match event.id.as_ref() {
        SHOW => ShellEvent::Show,
        POLL => ShellEvent::PollNow,
        QUIT => ShellEvent::Quit,
        _ => return,
      };
      wake(&menu_ctx, &menu_events, shell);
    }));
    let click_ctx = ctx.clone();
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
      let open = matches!(
        event,
        TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }
          | TrayIconEvent::DoubleClick { button: MouseButton::Left, .. }
      );
      if open {
        wake(&click_ctx, &events, ShellEvent::Show);
      }
    }));

    Ok(Self { _icon: tray, show, poll, quit })
  }

  /// After a language change.
  pub fn set_labels(&self, labels: &Labels) {
    self.show.set_text(&labels.show);
    self.poll.set_text(&labels.poll);
    self.quit.set_text(&labels.quit);
  }
}
