//! Shows a test panel, for checking the overlay on a desktop:
//! `cargo run -p fluffyframe-overlay --example show [--edit] [monitor]`.
//! `--edit` unlocks it for 30 s and prints the layout after each drag.
//! `WAYLAND_DISPLAY= cargo run …` tries the X11 path (XWayland) instead.

use fluffyframe_overlay::overlay::{Layout, Overlay, Panel};

/// Prints the overlay's log to stderr.
struct Stderr;
impl log::Log for Stderr {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }
    fn log(&self, r: &log::Record) {
        eprintln!("{} {}", r.level(), r.args());
    }
    fn flush(&self) {}
}

fn main() {
    let _ = log::set_logger(&Stderr).map(|()| log::set_max_level(log::LevelFilter::Info));
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let edit = args.iter().position(|a| a == "--edit").map(|i| args.remove(i)).is_some();
    let monitor = args.first().cloned();

    let font: &'static [u8] = include_bytes!("../../../assets/fonts/barlow-latin-600-normal.ttf");
    let overlay = Overlay::start(vec![font], Layout::default(), |l| println!("layout: {l:?}"));
    let panel = Panel {
        title: "FluffyFrame overlay test".into(),
        lines: vec!["Current · B · 58%".into(), "   +120.5% Critical Chance  x1.07  A *".into(), "New roll · A · 81%".into()],
    };
    if edit {
        // On a named monitor, like the app: shown there first, then unlocked.
        if monitor.is_some() {
            overlay.show(monitor.as_deref(), panel.clone());
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        overlay.set_editing(Some(panel));
        std::thread::sleep(std::time::Duration::from_secs(30));
        overlay.set_editing(None);
        std::thread::sleep(std::time::Duration::from_millis(200));
    } else {
        overlay.show(monitor.as_deref(), panel);
        std::thread::sleep(std::time::Duration::from_secs(4));
    }
}
