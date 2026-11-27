//! Number and time formatting (original: `src/lib/format.ts`, `src/lib/timers.ts`).

use chrono::{DateTime, Datelike, Local, Utc};
use crate::i18n::{tkey, tr, trf};
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
        return tr("now").into();
    }
    let (d, h, m, s) = (secs / 86_400, secs % 86_400 / 3600, secs % 3600 / 60, secs % 60);
    if d > 0 {
        trf("{}d {}h", &[&d, &h])
    } else if h > 0 {
        trf("{}h {}m", &[&h, &format!("{m:02}")])
    } else {
        trf("{}m {}s", &[&m, &format!("{s:02}")])
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
        tr("just now").into()
    } else {
        trf("{} ago", &[&countdown_secs(secs)])
    }
}

pub fn local_time(t: DateTime<Utc>) -> String {
    let t = t.with_timezone(&Local);
    format!("{} {}", weekday(t.weekday()), t.format("%H:%M"))
}

/// Short weekday name in the interface language ("Thu").
pub fn weekday(day: chrono::Weekday) -> &'static str {
    const DAYS: [&str; 7] = [tkey("Mon"), tkey("Tue"), tkey("Wed"), tkey("Thu"), tkey("Fri"), tkey("Sat"), tkey("Sun")];
    tr(DAYS[day.num_days_from_monday() as usize])
}

/// Short month name in the interface language ("Nov"), 1-based.
pub fn month(m: u32) -> &'static str {
    const MONTHS: [&str; 12] = [tkey("Jan"), tkey("Feb"), tkey("Mar"), tkey("Apr"), tkey("May"), tkey("Jun"), tkey("Jul"), tkey("Aug"), tkey("Sep"), tkey("Oct"), tkey("Nov"), tkey("Dec")];
    tr(MONTHS[(m.clamp(1, 12) - 1) as usize])
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

/// A node label ("Martialis (Mars)") with its planet in the interface language;
/// node names themselves are proper names and stay.
pub fn node(label: &str) -> String {
    match label.strip_suffix(')').and_then(|l| l.rsplit_once(" (")) {
        Some((name, planet)) => format!("{name} ({})", crate::i18n::tr_any(planet)),
        None => crate::i18n::tr_any(label).to_string(),
    }
}

/// A drop location ("Mercury/Apollodorus (Survival)") with its planet and
/// mission type in the interface language; anything else stays as given.
pub fn drop_location(location: &str) -> String {
    use crate::i18n::tr_any;
    let (place, mode) = match location.strip_suffix(')').and_then(|l| l.rsplit_once(" (")) {
        Some((place, mode)) => (place, Some(mode)),
        None => (location, None),
    };
    let place = match place.split_once('/') {
        Some((planet, node)) => format!("{}/{node}", tr_any(planet)),
        None => place.to_string(),
    };
    match mode {
        Some(mode) => format!("{place} ({})", tr_any(mode)),
        None => place,
    }
}

/// A planet cycle's name or state in the interface language. The core names
/// stay English: they are also alert settings keys.
pub fn cycle_text(text: &'static str) -> &'static str {
    match text {
        "Earth" => tr("Earth"),
        "Cetus" => tr("Cetus"),
        "Cambion Drift" => tr("Cambion Drift"),
        "Orb Vallis" => tr("Orb Vallis"),
        "Duviri" => tr("Duviri"),
        "Day" => tr("Day"),
        "Night" => tr("Night"),
        "Warm" => tr("Warm"),
        "Cold" => tr("Cold"),
        "Fass" => tr("Fass"),
        "Vome" => tr("Vome"),
        "Sorrow" => tr("Sorrow"),
        "Fear" => tr("Fear"),
        "Joy" => tr("Joy"),
        "Anger" => tr("Anger"),
        "Envy" => tr("Envy"),
        other => other,
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
