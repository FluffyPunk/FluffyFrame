//! The in-game panel for a riven mid-cycle: its current roll and the new one
//! on offer, shown over the game as soon as a read finds the cycle (hotkey or
//! EE.log), hidden once the roll is chosen.

use std::time::Duration;

use fluffyframe_core::services::riven_good_rolls::{GoodRollData, sheet_tag};
use fluffyframe_core::services::rivens::{self, DecodedRiven, DecodedStat, OwnedRiven};
use fluffyframe_overlay::overlay::Panel;

use crate::i18n::{tr, trf};
use crate::ipc::Backend;
use crate::stores::AppState;

/// Hidden after this even if no choice was seen (the game closed, the log
/// missed it).
const SHOW_FOR: Duration = Duration::from_secs(5 * 60);

/// Shows the panel for a new roll on offer, or hides it once none is.
/// Called whenever the inventory changes. A roll is shown once: it stays in
/// the inventory until chosen, and later reads (every few minutes) mustn't
/// bring the panel back over the game.
pub fn update(state: &mut AppState, backend: &Backend) {
    let Some(riven) = state.inventory.as_ref().and_then(|inv| inv.rivens.iter().find(|r| r.pending.is_some()).cloned()) else {
        if state.riven_overlay.take().is_some() {
            backend.overlay.hide();
        }
        return;
    };
    let roll = serde_json::to_string(&riven).unwrap_or_default();
    if state.riven_overlay.as_ref() == Some(&roll) {
        return;
    }
    let (enabled, monitor) = {
        let s = backend.shared.settings.read().unwrap();
        (s.overlay_enabled, s.overlay_monitor.clone())
    };
    // Game data not loaded yet: shown by the next read instead.
    let Some(panel) = panel(state, &riven).filter(|_| enabled) else { return };
    backend.overlay.show_for(monitor.as_deref(), panel, SHOW_FOR);
    state.riven_overlay = Some(roll);
}

/// `riven`'s current roll and the one on offer, as a panel (None: the game
/// data to decode them isn't loaded yet).
fn panel(state: &AppState, riven: &OwnedRiven) -> Option<Panel> {
    let db = state.db.as_ref()?;
    let decode = |r: &OwnedRiven| rivens::decode(r, &db.rivens, state.good_rolls.as_deref());
    let (current, new) = (decode(riven)?, decode(riven.pending.as_deref()?)?);
    // The community sheet's row for the weapon: what it wants and tolerates.
    let sheet = state.good_rolls.as_deref().and_then(|g| g.get(&current.weapon_name, &db.rivens));
    let label = |tag: &str| db.rivens.entry(&riven.item_type, tag).map_or_else(|| tag.to_string(), |e| e.name.clone());

    let title = format!("{} {}", current.weapon_name, current.suffix).trim().to_string();
    let mut lines = roll_lines(tr("Current"), &current, sheet);
    lines.push(String::new());
    lines.extend(roll_lines(tr("New roll"), &new, sheet));
    lines.push(String::new());
    match sheet {
        Some(data) => {
            let wanted: Vec<String> = data.wanted().into_iter().map(label).collect();
            let curses: Vec<String> = data.accepted_bad.iter().map(|t| label(t)).collect();
            lines.extend(wrap(tr("Good buffs:"), &wanted, tr("any")));
            lines.extend(wrap(tr("Good curses:"), &curses, tr("none")));
        }
        None => lines.push(tr("No good-roll data for this weapon.").to_string()),
    }
    lines.push(trf(tr("Rerolls: {}"), &[&riven.rerolls]));
    Some(Panel { title, lines })
}

/// "Current · A- · 71% · Good", then a line per stat:
/// "+184.8% Critical Chance  x1.07  A *", where `*` marks a buff the sheet
/// wants (or a curse it tolerates) on this weapon.
fn roll_lines(label: &str, d: &DecodedRiven, sheet: Option<&GoodRollData>) -> Vec<String> {
    let mut head = format!("{label} · {} · {}%", d.grade, (d.perfect * 100.0).round());
    if let Some(g) = d.attr_grade {
        head.push_str(" · ");
        head.push_str(tr(g.label()));
    }
    let good = |s: &DecodedStat| {
        let tag = sheet_tag(&s.tag);
        sheet.is_some_and(|data| if s.positive { data.wanted().contains(&tag.as_str()) } else { data.accepted_bad.contains(&tag) })
    };
    let stats = d.stats.iter().map(|s| {
        let mark = if good(s) { " *" } else { "" };
        format!("   {} {}  x{:.2}  {}{mark}", s.value_text(), s.name, ratio(s.roll), s.grade)
    });
    std::iter::once(head).chain(stats).collect()
}

/// A stat's multiplier on its base value: rolls span 0.9x to 1.1x.
fn ratio(roll: f64) -> f64 {
    0.9 + 0.2 * roll.clamp(0.0, 1.0)
}

/// Characters per line before a list wraps (the panel is ~640 px wide).
const WRAP_AT: usize = 52;

/// "Good buffs: Critical Chance, Multishot, …", wrapped onto indented lines;
/// `empty` when there are none.
fn wrap(label: &str, items: &[String], empty: &str) -> Vec<String> {
    let mut lines = vec![label.to_string()];
    if items.is_empty() {
        lines[0] = format!("{label} {empty}");
        return lines;
    }
    for (i, item) in items.iter().enumerate() {
        let piece = if i + 1 < items.len() { format!("{item},") } else { item.clone() };
        let last = lines.last_mut().expect("starts with the label");
        if last.chars().count() + 1 + piece.chars().count() > WRAP_AT && last.trim() != label {
            lines.push(format!("   {piece}"));
        } else {
            last.push(' ');
            last.push_str(&piece);
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_spans_the_roll_range() {
        assert_eq!(format!("x{:.2}", ratio(0.0)), "x0.90");
        assert_eq!(format!("x{:.2}", ratio(0.5)), "x1.00");
        assert_eq!(format!("x{:.2}", ratio(1.0)), "x1.10");
    }

    #[test]
    fn lists_wrap_on_indented_lines() {
        let items: Vec<String> = ["Critical Chance", "Critical Damage", "Multishot", "Damage", "Fire Rate"].map(String::from).into();
        let lines = wrap("Good buffs:", &items, "any");
        assert_eq!(lines[0], "Good buffs: Critical Chance, Critical Damage,");
        assert_eq!(lines[1], "   Multishot, Damage, Fire Rate");
        assert!(lines.iter().all(|l| l.chars().count() <= WRAP_AT));
        assert_eq!(wrap("Good curses:", &[], "none"), ["Good curses: none"]);
    }
}
