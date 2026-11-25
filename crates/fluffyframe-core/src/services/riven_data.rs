//! Riven stat tables from DE's export (original: `services/rivenData.ts`): each
//! riven mod type's upgrade entries (base value, card label, name syllables) and
//! each weapon's name and disposition (`omegaAttenuation`, currently 1.2 for every
//! riven-capable weapon).

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::item_database::sanitize_name;
use super::public_export::ExportSet;

/// One stat a riven type can roll.
#[derive(Debug, Clone)]
pub struct RivenEntry {
    pub tag: String,
    /// Value per rank before disposition and roll scaling.
    pub base: f64,
    /// Card label ("Critical Chance", "Heat").
    pub name: String,
    /// How the card prints the value: "%", "s" or nothing (Punch Through, Range,
    /// faction damage), read from the label's `|val|` unit.
    pub unit: &'static str,
    /// Name syllables ("crita", "cron").
    pub prefix: String,
    pub suffix: String,
}

#[derive(Debug, Default)]
pub struct RivenTables {
    /// Riven mod uniqueName → its stat entries.
    mods: HashMap<String, Vec<RivenEntry>>,
    /// Weapon uniqueName (the fingerprint's `compat`) → display name and disposition.
    weapons: HashMap<String, (String, Option<f64>)>,
    /// Lowercase weapon names, to tell a variant from a standalone weapon.
    names_lc: HashSet<String>,
}

/// Variant affixes folded into the base weapon's family (original: `weaponVariants.ts`).
const VARIANT_SUFFIXES: [&str; 3] = [" Prime", " Wraith", " Vandal"];
const VARIANT_PREFIXES: [&str; 10] =
    ["MK1-", "Kuva ", "Tenet ", "Prisma ", "Vaykor ", "Rakta ", "Secura ", "Sancti ", "Synoid ", "Telos "];

/// "|val|% <DT_FIRE_COLOR>Heat" → "Heat"; "|val|% Fire Rate (x2 for Bows)" → "Fire Rate".
fn clean_loc(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '|' => {
                // |val| plus an optional unit right after it.
                for c in chars.by_ref() {
                    if c == '|' {
                        break;
                    }
                }
                if matches!(chars.peek(), Some('%' | 's')) {
                    chars.next();
                }
            }
            '<' => {
                for c in chars.by_ref() {
                    if c == '>' {
                        break;
                    }
                }
            }
            '(' => {
                for c in chars.by_ref() {
                    if c == ')' {
                        break;
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl RivenTables {
    pub fn build(exports: &ExportSet) -> Self {
        let mut t = RivenTables::default();
        for w in exports.array("ExportWeapons", "ExportWeapons") {
            let (Some(unique), Some(name)) =
                (w.get("uniqueName").and_then(Value::as_str), w.get("name").and_then(Value::as_str))
            else {
                continue;
            };
            let name = sanitize_name(name);
            t.names_lc.insert(name.to_lowercase());
            let dispo = w.get("omegaAttenuation").and_then(Value::as_f64);
            t.weapons.insert(unique.to_string(), (name, dispo));
        }
        for m in exports.array("ExportUpgrades", "ExportUpgrades") {
            let Some(unique) = m.get("uniqueName").and_then(Value::as_str).filter(|u| u.contains("/Randomized/")) else {
                continue;
            };
            let Some(entries) = m.get("upgradeEntries").and_then(Value::as_array) else { continue };
            let entries: Vec<RivenEntry> = entries
                .iter()
                .filter_map(|e| {
                    let tag = e.get("tag")?.as_str()?.to_string();
                    let value = e.get("upgradeValues").and_then(|v| v.get(0));
                    let s = |k: &str| e.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                    let raw_loc = value.and_then(|v| v.get("locTag")).and_then(Value::as_str).unwrap_or("");
                    let loc = clean_loc(raw_loc);
                    let unit = if raw_loc.contains("|%") {
                        "%"
                    } else if raw_loc.contains("|s ") {
                        "s"
                    } else {
                        ""
                    };
                    Some(RivenEntry {
                        base: display_base(&tag, value.and_then(|v| v.get("value")).and_then(Value::as_f64).unwrap_or(0.0)),
                        name: if loc.is_empty() { tag.clone() } else { loc },
                        unit,
                        prefix: s("prefixTag"),
                        suffix: s("suffixTag"),
                        tag,
                    })
                })
                .collect();
            if !entries.is_empty() {
                t.mods.insert(unique.to_string(), entries);
            }
        }
        t
    }

    pub fn weapon_name(&self, unique: &str) -> Option<&str> {
        self.weapons.get(unique).map(|(n, _)| n.as_str())
    }

    pub fn disposition(&self, unique: &str) -> Option<f64> {
        self.weapons.get(unique).and_then(|(_, d)| *d)
    }

    /// The riven type's entry for `tag`, with the original's melee fallbacks.
    pub fn entry(&self, riven_type: &str, tag: &str) -> Option<&RivenEntry> {
        let entries = self.mods.get(riven_type)?;
        let find = |t: &str| entries.iter().find(|e| e.tag == t);
        find(tag)
            .or_else(|| find(&tag.replace("WeaponFaction", "WeaponMeleeFaction")))
            .or_else(|| if tag == "WeaponDamageAmountMod" { find("WeaponMeleeDamageMod") } else { None })
    }

    /// Generated riven name ("Crita-visicron"): buffs by descending roll, prefixes
    /// then the last buff's suffix; curses do not count.
    pub fn suffix(&self, riven_type: &str, buffs: &[(String, f64)]) -> String {
        let mut sorted: Vec<(&RivenEntry, f64)> =
            buffs.iter().filter_map(|(tag, roll)| Some((self.entry(riven_type, tag)?, *roll))).collect();
        sorted.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.base.total_cmp(&b.0.base)));
        let title = |s: &str| -> String {
            let lower = s.to_lowercase();
            let mut c = lower.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
        };
        match sorted.as_slice() {
            [] => String::new(),
            [(e, _)] if e.prefix.is_empty() => String::new(),
            [(e, _)] => title(&e.prefix) + &e.suffix.to_lowercase(),
            many => {
                let mut name = String::new();
                for (i, (e, _)) in many.iter().enumerate() {
                    if i == many.len() - 1 {
                        name += &e.suffix.to_lowercase();
                    } else if name.is_empty() {
                        name += &title(&e.prefix);
                    } else if !e.prefix.is_empty() {
                        name += "-";
                        name += &e.prefix.to_lowercase();
                    }
                }
                name
            }
        }
    }

    /// Riven family slug: variant affixes fold into the base weapon when it exists
    /// ("Kuva Bramma" has no base form and stays itself); original `getRivenFamilySlug`.
    pub fn family_slug(&self, weapon_name: &str) -> String {
        let base_if_known = |candidate: &str, fallback: &str| {
            if self.names_lc.contains(&candidate.to_lowercase()) { candidate.to_string() } else { fallback.to_string() }
        };
        let mut name = weapon_name.trim().to_string();
        for suffix in VARIANT_SUFFIXES {
            if name.to_lowercase().ends_with(&suffix.to_lowercase()) {
                name = base_if_known(&name[..name.len() - suffix.len()], &name);
                break;
            }
        }
        for prefix in VARIANT_PREFIXES {
            if name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                name = base_if_known(&name[prefix.len()..], &name);
                break;
            }
        }
        slug(&name.replace('&', " and "))
    }
}

/// DE stores Ammo Efficiency with a negative base, but the card reads it like
/// Heavy Attack Efficiency: a buff is "+X%", a curse "-X%". (Recoil keeps its
/// negative base on purpose: a recoil buff reads "-X% Weapon Recoil".)
fn display_base(tag: &str, base: f64) -> f64 {
    if tag == "WeaponAmmoEfficiency" { base.abs() } else { base }
}

/// "Silva and Aegis" → "silva_and_aegis".
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().replace(['\'', '\u{2019}', '\u{2018}'], "").chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_card_labels() {
        assert_eq!(clean_loc("|val|% <DT_FIRE_COLOR>Heat"), "Heat");
        assert_eq!(clean_loc("|val|% Fire Rate (x2 for Bows)"), "Fire Rate");
        assert_eq!(clean_loc("|val|s Combo Duration"), "Combo Duration");
        assert_eq!(clean_loc("|val| Punch Through"), "Punch Through");
        assert_eq!(clean_loc("|STAT1|% Critical Chance for Slide Attack"), "Critical Chance for Slide Attack");
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Silva and Aegis"), "silva_and_aegis");
        assert_eq!(slug("MK1-Braton"), "mk1_braton");
    }
}
