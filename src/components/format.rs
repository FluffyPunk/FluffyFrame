//! Number and time formatting (original: `src/lib/format.ts`, `src/lib/timers.ts`).

use chrono::{DateTime, Local, Utc};
pub use fluffyframe_core::config::game_tables::title_case;

/// 1234567 → "1,234,567"
pub fn int(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// 1_250_000 → "1.25M"
pub fn compact(n: u64) -> String {
    match n {
        0..=9_999 => int(n),
        10_000..=999_999 => format!("{:.1}k", n as f64 / 1e3),
        1_000_000..=999_999_999 => format!("{:.2}M", n as f64 / 1e6),
        _ => format!("{:.2}B", n as f64 / 1e9),
    }
}

pub fn plat(p: Option<f64>) -> String {
    match p {
        Some(p) if p >= 10.0 || p.fract() == 0.0 => format!("{p:.0}p"),
        Some(p) => format!("{p:.1}p"),
        None => "—".into(),
    }
}

/// Time left as "2d 3h", "3h 12m", "4m 05s".
pub fn countdown_secs(secs: i64) -> String {
    if secs <= 0 {
        return "now".into();
    }
    let (d, h, m, s) = (secs / 86_400, secs % 86_400 / 3600, secs % 3600 / 60, secs % 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m:02}m")
    } else {
        format!("{m}m {s:02}s")
    }
}

pub fn until(t: Option<DateTime<Utc>>) -> String {
    match t {
        Some(t) => countdown_secs((t - Utc::now()).num_seconds()),
        None => "—".into(),
    }
}

pub fn ago(t: DateTime<Utc>) -> String {
    let secs = (Utc::now() - t).num_seconds();
    if secs < 60 {
        "just now".into()
    } else {
        format!("{} ago", countdown_secs(secs))
    }
}

pub fn local_time(t: DateTime<Utc>) -> String {
    t.with_timezone(&Local).format("%a %H:%M").to_string()
}

pub fn local_datetime(t: DateTime<Utc>) -> String {
    t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string()
}

pub fn duration_secs(secs: u64) -> String {
    let (h, m) = (secs / 3600, secs % 3600 / 60);
    match (h, m) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn formats() {
        assert_eq!(super::int(1_234_567), "1,234,567");
        assert_eq!(super::int(12), "12");
        assert_eq!(super::compact(1_250_000), "1.25M");
        assert_eq!(super::countdown_secs(3 * 3600 + 120), "3h 02m");
        assert_eq!(super::plat(Some(12.4)), "12p");
        assert_eq!(super::duration_secs(43_200), "12h");
    }
}
