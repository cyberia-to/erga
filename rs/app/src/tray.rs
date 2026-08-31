//! The menu-bar item.
//!
//! erga is a thing you start and then stop looking at, so the one number
//! worth carrying into the menu bar is how close the payout is. Idle, it
//! offers the only action that matters instead.
//!
//! macOS wants the status item built and touched from the main thread; every
//! call here is made from `update`, which is the main thread under eframe.

use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

/// The app's own mark, shrunk for the menu bar and cut out of its plate.
///
/// The bundled artwork is a mint heptagon on a near-black squircle; that
/// square would read as a black tile up there, so the plate is keyed out by
/// alpha and only the mark survives. Same bytes as the Dock icon, so the two
/// cannot drift apart.
fn mark(size: u32) -> Option<Icon> {
    let src = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).ok()?;
    let (sw, sh) = (src.width as usize, src.height as usize);
    let n = size as usize;
    let mut out = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            // box-average the source cell this pixel covers
            let (x0, x1) = (x * sw / n, ((x + 1) * sw / n).max(x * sw / n + 1));
            let (y0, y1) = (y * sh / n, ((y + 1) * sh / n).max(y * sh / n + 1));
            let (mut r, mut g, mut b, mut count) = (0u32, 0u32, 0u32, 0u32);
            for sy in y0..y1.min(sh) {
                for sx in x0..x1.min(sw) {
                    let o = (sy * sw + sx) * 4;
                    r += src.rgba[o] as u32;
                    g += src.rgba[o + 1] as u32;
                    b += src.rgba[o + 2] as u32;
                    count += 1;
                }
            }
            let c = count.max(1);
            let (r, g, b) = ((r / c) as u8, (g / c) as u8, (b / c) as u8);
            // the plate is near-black: fade it out rather than paint a tile
            let lum = r.max(g).max(b) as f32 / 255.0;
            let a = ((lum - 0.10) / 0.35).clamp(0.0, 1.0);
            out.extend_from_slice(&[r, g, b, (a * 255.0) as u8]);
        }
    }
    Icon::from_rgba(out, size, size).ok()
}

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
            .with_icon(mark(22)?)
            .with_icon_as_template(false)
            .with_title("")
            .with_tooltip("erga")
            .build()
            .ok()?;
        Some(Tray { icon, toggle, quit, title: String::new(), label: "start mining".into() })
    }

    /// Show the payout while mining, the mark while idle. Only touches the
    /// status item when the text actually changed — the menu bar redraws on
    /// every set, and this runs several times a second.
    pub fn update(&mut self, running: bool, mhs: f64, toward: Option<f32>) {
        let title = if !running {
            String::new()
        } else if let Some(t) = toward {
            format!(" {:.0}%", (t * 100.0).min(100.0))
        } else {
            format!(" {mhs:.0}")
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
