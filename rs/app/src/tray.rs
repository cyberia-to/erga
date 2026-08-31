//! The menu-bar item.
//!
//! erga is a thing you start and then stop looking at, so the one number
//! worth carrying into the menu bar is how close the payout is. Idle, it
//! offers the only action that matters instead.
//!
//! macOS wants the status item built and touched from the main thread; every
//! call here is made from `update`, which is the main thread under eframe.

use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

pub struct Tray {
    icon: TrayIcon,
    toggle: MenuItem,
    quit: MenuItem,
    title: String,
    label: String,
}

/// What the menu bar was asked to do this frame.
pub enum Ask {
    Nothing,
    ToggleMining,
    Quit,
}

impl Tray {
    pub fn new() -> Option<Self> {
        let toggle = MenuItem::new("start mining", true, None);
        let quit = MenuItem::new("quit erga", true, None);
        let menu = Menu::new();
        menu.append(&toggle).ok()?;
        menu.append(&tray_icon::menu::PredefinedMenuItem::separator()).ok()?;
        menu.append(&quit).ok()?;
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_title("⬡")
            .with_tooltip("erga")
            .build()
            .ok()?;
        Some(Tray { icon, toggle, quit, title: "⬡".into(), label: "start mining".into() })
    }

    /// Show the payout while mining, the mark while idle. Only touches the
    /// status item when the text actually changed — the menu bar redraws on
    /// every set, and this runs several times a second.
    pub fn update(&mut self, running: bool, mhs: f64, toward: Option<f32>) {
        let title = if !running {
            "⬡".to_string()
        } else if let Some(t) = toward {
            format!("⬡ {:.0}%", (t * 100.0).min(100.0))
        } else {
            format!("⬡ {mhs:.0}")
        };
        if title != self.title {
            let _ = self.icon.set_title(Some(&title));
            self.title = title;
        }
        let label = if running { "stop mining" } else { "start mining" };
        if label != self.label {
            self.toggle.set_text(label);
            self.label = label.to_string();
        }
        let tip = if running {
            match toward {
                Some(t) => format!("erga · {mhs:.1} MH/s · {:.1}% toward payout", t * 100.0),
                None => format!("erga · {mhs:.1} MH/s"),
            }
        } else {
            "erga · idle".to_string()
        };
        let _ = self.icon.set_tooltip(Some(tip));
    }

    /// Drain the menu's events. Returns what was asked, if anything.
    pub fn poll(&self) -> Ask {
        let mut ask = Ask::Nothing;
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            if ev.id == self.toggle.id() {
                ask = Ask::ToggleMining;
            } else if ev.id == self.quit.id() {
                ask = Ask::Quit;
            }
        }
        ask
    }
}
