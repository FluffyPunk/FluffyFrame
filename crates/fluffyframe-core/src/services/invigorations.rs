//! Helminth invigorations (after browse.wf's oracle, `invigorations.pluto`).
//!
//! Each week the game offers three warframes, each with an offensive and a
//! utility invigoration. The picks are seeded from the week number and the
//! player's name, and the previous offerings' warframes are left out of the
//! draw. The game does part of the maths in 32-bit floats; that rounding is
//! kept, or the picks drift.

use chrono::{DateTime, TimeZone, Utc};

/// Week 0 starts here (Monday 00:00 UTC).
const EPOCH: i64 = 1_391_990_400;
const WEEK: i64 = 604_800;
const PER_WEEK: usize = 3;

const OFFENSIVE: [&str; 9] = [
    "PowerStrength",
    "PowerRange",
    "PowerDuration",
    "MeleeDamage",
    "PrimaryDamage",
    "SecondaryDamage",
    "PrimaryCritChance",
    "SecondaryCritChance",
    "MeleeCritChance",
];
const UTILITY: [&str; 11] = [
    "PowerEfficiency",
    "MovementSpeed",
    "ParkourSpeed",
    "Health",
    "Energy",
    "StatusResistance",
    "ReloadSpeed",
    "HealthRegen",
    "Armor",
    "Jumps",
    "EnergyRegen",
];
/// Hildryn and Lavos use no energy: no energy invigorations in their weeks.
const NO_ENERGY: [&str; 2] = ["/Lotus/Powersuits/IronFrame/IronFrameBaseSuit", "/Lotus/Powersuits/Alchemist/AlchemistBaseSuit"];

/// The invigoration week at `t`.
pub fn week_index(t: DateTime<Utc>) -> i64 {
    (t.timestamp() - EPOCH).div_euclid(WEEK)
}

pub fn week_start(index: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(EPOCH + index * WEEK, 0).single().unwrap_or_default()
}

/// The game's generator: a 64-bit LCG; draws use its high 32 bits.
struct Rng(u64);

impl Rng {
    fn step(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(0x1405_7B7E_F767_814F);
        (self.0 >> 32) as u32
    }

    /// A float in `lo..hi`, computed in f32 as the game does.
    fn float(&mut self, lo: f32, hi: f32) -> f32 {
        let high = self.step();
        let (lo, hi) = if lo > hi { (hi, lo) } else { (lo, hi) };
        ((high >> 6) & 0xFF_FFFF) as f32 * 0.000_000_059_604_645_f32 * (hi - lo) + lo
    }

    fn int(&mut self, lo: u64, hi: u64) -> u64 {
        if hi == lo {
            return lo;
        }
        lo + u64::from(self.step() & 0x3FFF_FFFF) % (hi - lo + 1)
    }
}

/// Standard CRC-32 (the oracle's `crypto.crc32`).
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// `n` distinct entries by weight; every draw consumes one random number.
fn pick<T: Copy>(mut entries: Vec<(T, f64)>, n: usize, rng: &mut Rng) -> Vec<T> {
    let mut out = Vec::with_capacity(n);
    let mut total: f64 = entries.iter().map(|e| e.1).sum();
    for _ in 0..n {
        let mut r = f64::from(rng.float(0.0, 1.0)) * total;
        if let Some(i) = entries.iter().position(|&(_, w)| {
            let hit = r < w;
            r -= w;
            hit
        }) {
            let (t, w) = entries.remove(i);
            out.push(t);
            total -= w;
        }
    }
    out
}

/// One week's offerings: base suit uniqueNames and invigoration names
/// (`OFFENSIVE`/`UTILITY` entries), in offering order.
#[derive(Debug, Clone, PartialEq)]
pub struct Offerings {
    pub suits: Vec<String>,
    pub offensive: Vec<&'static str>,
    pub utility: Vec<&'static str>,
}

/// The week's draw for `name` over `suits` (base suit uniqueNames in the game's
/// order), leaving out `exclude` (the previous offerings). With `known_suits`,
/// those are used instead of the drawn ones: the invigoration draws come after
/// the suit draws and don't depend on them, so they are exact either way.
fn draw(name: &str, index: i64, suits: &[String], exclude: &[String], known_suits: Option<&[String]>) -> Offerings {
    let mut rng = Rng(index as u64);
    let v = rng.int(1, 1_000_000);
    // The oracle adds the name's CRC in f32: the seed keeps that rounding.
    let seed = (crc32(name.as_bytes()) as f32 + v as f32) as u64;
    let mut rng = Rng(seed);
    let weighted = suits.iter().map(|s| (s, if exclude.contains(s) { 0.0 } else { 1.0 })).collect();
    let drawn: Vec<String> = pick(weighted, PER_WEEK, &mut rng).into_iter().cloned().collect();
    let suits = known_suits.map_or(drawn, <[String]>::to_vec);
    let offensive = pick(OFFENSIVE.iter().map(|&o| (o, 1.0)).collect(), PER_WEEK, &mut rng);
    let no_energy = suits.iter().any(|s| NO_ENERGY.contains(&s.as_str()));
    let utility = UTILITY.iter().filter(|u| !(no_energy && matches!(**u, "Energy" | "EnergyRegen"))).map(|&u| (u, 1.0)).collect();
    let utility = pick(utility, PER_WEEK, &mut rng);
    Offerings { suits, offensive, utility }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Week {
    pub index: i64,
    pub offerings: Offerings,
    /// Taken from the account (the current week the game already rolled).
    pub from_inventory: bool,
}

/// This week and the `count - 1` after it for `name`. `last_seen` is the
/// account's stored offerings (week index and suits): the game leaves those out
/// of the next draw, so later weeks are exact only if each week is opened in
/// game; without it the first week's warframes are a guess.
pub fn plan(name: &str, suits: &[String], last_seen: Option<(i64, &[String])>, now: DateTime<Utc>, count: usize) -> Vec<Week> {
    let current = week_index(now);
    let mut out = Vec::with_capacity(count);
    let mut previous: Vec<String> = Vec::new();
    for index in current..current + count as i64 {
        let week = match last_seen {
            Some((seen, offered)) if seen == index => {
                Week { index, offerings: draw(name, index, suits, &[], Some(offered)), from_inventory: true }
            }
            _ => {
                // The week after the stored offerings excludes them; later ones
                // exclude the week before.
                let exclude = match last_seen {
                    Some((seen, offered)) if index == current && seen < index => offered.to_vec(),
                    _ => previous.clone(),
                };
                Week { index, offerings: draw(name, index, suits, &exclude, None), from_inventory: false }
            }
        };
        previous = week.offerings.suits.clone();
        out.push(week);
    }
    out
}

/// Base suit uniqueNames in the game's draw order, from ExportWarframes entries
/// (uniqueName, productCategory, parentName) in export order: Excalibur first,
/// then each other base suit, latest first (the oracle prepends them).
pub fn suit_order<'a>(entries: impl Iterator<Item = (&'a str, &'a str)>) -> Vec<String> {
    const EXCALIBUR: &str = "/Lotus/Powersuits/Excalibur/ExcaliburBaseSuit";
    let mut order: Vec<String> = Vec::new();
    for (category, parent) in entries {
        if category == "Suits" && parent != "/Lotus/Powersuits/Excalibur/DarkExcalibur" && !order.iter().any(|p| p == parent) {
            order.insert(0, parent.to_string());
        }
    }
    order.retain(|p| p != EXCALIBUR);
    order.insert(0, EXCALIBUR.to_string());
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small export, checked against browse.wf's oracle algorithm.
    fn suits() -> Vec<String> {
        let names = ["Ash", "Banshee", "Ember", "Frost", "Excalibur", "Mag", "Nova", "Rhino", "Trinity", "Volt", "IronFrame", "Alchemist"];
        let parents: Vec<String> = names.iter().map(|n| format!("/Lotus/Powersuits/{n}/{n}BaseSuit")).collect();
        suit_order(parents.iter().map(|p| ("Suits", p.as_str())))
    }

    fn short(o: &Offerings) -> (Vec<&str>, Vec<&str>, Vec<&str>) {
        (o.suits.iter().map(|s| s.split('/').nth(3).unwrap()).collect(), o.offensive.clone(), o.utility.clone())
    }

    #[test]
    fn matches_the_oracle() {
        let suits = suits();
        let a = draw("FluffyTestTenno", 667, &suits, &[], None);
        assert_eq!(
            short(&a),
            (vec!["Excalibur", "Nova", "Ember"], vec!["SecondaryCritChance", "PowerRange", "MeleeDamage"], vec!["Jumps", "EnergyRegen", "PowerEfficiency"])
        );
        // Excluding a suit reshuffles every draw; Lavos drops the energy picks.
        let b = draw("FluffyTestTenno", 668, &suits, &["/Lotus/Powersuits/Frost/FrostBaseSuit".to_string()], None);
        assert_eq!(
            short(&b),
            (vec!["Alchemist", "Mag", "Ember"], vec!["SecondaryDamage", "PowerStrength", "PowerRange"], vec!["Armor", "HealthRegen", "PowerEfficiency"])
        );
        let c = draw("Tenno", 700, &suits, &[], None);
        assert_eq!(
            short(&c),
            (vec!["Volt", "Alchemist", "Trinity"], vec!["PowerDuration", "SecondaryCritChance", "MeleeCritChance"], vec!["Jumps", "ReloadSpeed", "Armor"])
        );
    }

    #[test]
    fn plan_uses_the_stored_week_then_chains() {
        let suits = suits();
        let now = week_start(667) + chrono::Duration::days(2);
        let seen = vec!["/Lotus/Powersuits/Ash/AshBaseSuit".to_string(), "/Lotus/Powersuits/Mag/MagBaseSuit".to_string(), "/Lotus/Powersuits/Volt/VoltBaseSuit".to_string()];
        let weeks = plan("FluffyTestTenno", &suits, Some((667, &seen)), now, 3);
        assert!(weeks[0].from_inventory && weeks[0].offerings.suits == seen);
        // Invigorations don't depend on the suit draws.
        assert_eq!(weeks[0].offerings.offensive, vec!["SecondaryCritChance", "PowerRange", "MeleeDamage"]);
        assert_eq!(weeks[1].offerings, draw("FluffyTestTenno", 668, &suits, &seen, None));
        assert_eq!(weeks[2].offerings, draw("FluffyTestTenno", 669, &suits, &weeks[1].offerings.suits, None));
        // Stale stored offerings are left out of the current week.
        let stale = plan("FluffyTestTenno", &suits, Some((660, &seen)), now, 1);
        assert_eq!(stale[0].offerings, draw("FluffyTestTenno", 667, &suits, &seen, None));
        assert_eq!(week_index(now), 667);
    }

    #[test]
    fn crc32_is_standard() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }
}
