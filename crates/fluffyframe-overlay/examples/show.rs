//! Shows a test panel for a few seconds, for checking the overlay on a
//! desktop: `cargo run -p fluffyframe-overlay --example show [monitor]`.
//! `WAYLAND_DISPLAY= cargo run …` tries the X11 path (XWayland) instead.

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
    let font: &'static [u8] = include_bytes!("../../../assets/fonts/barlow-latin-600-normal.ttf");
    let overlay = fluffyframe_overlay::overlay::Overlay::start(vec![font]);
    let monitor = std::env::args().nth(1);
    let panel = fluffyframe_overlay::overlay::Panel {
        title: "FluffyFrame overlay test".into(),
        lines: vec!["Current · B · 58%".into(), "   +120.5% Critical Chance  A".into(), "New roll · A · 81%".into()],
    };
    overlay.show(monitor.as_deref(), panel);
    std::thread::sleep(std::time::Duration::from_secs(4));
}
