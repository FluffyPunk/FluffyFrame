//! Owned rivens from inventory fingerprints (original: `services/rivenFingerprint.ts`,
//! `services/rivenData.ts`). The OCR riven scanner is intentionally not ported.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::game_tables;

/// Fingerprint values are fractions of this maximum.
const FINGERPRINT_MAX: f64 = 1_073_741_823.0; // 0x3FFFFFFF

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RivenStat {
    pub tag: String,
    /// 0..1 position inside the stat's roll range.
    pub roll: f64,
}

impl RivenStat {
    pub fn label(&self, melee: bool) -> String {
        game_tables::riven_stat_label(&self.tag, melee)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnedRiven {
    pub item_id: Option<String>,
    pub item_type: String,
    pub riven_type: String,
    /// Weapon unique name the riven is locked to.
    pub weapon: Option<String>,
    pub mastery_req: u32,
    pub rank: u32,
    pub rerolls: u32,
    pub polarity: Option<String>,
    pub buffs: Vec<RivenStat>,
    pub curses: Vec<RivenStat>,
    /// Veiled rivens carry their unlock challenge instead of stats.
    pub challenge: Option<RivenChallenge>,
}

/// A veiled riven's unlock challenge (original: `describeChallengeType`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RivenChallenge {
    /// "Kill 25 Enemies with Headshots without taking damage"
    pub description: String,
    /// The description without its count or complication, for grouping.
    pub group: String,
    pub progress: Option<u32>,
    pub required: Option<u32>,
}

impl OwnedRiven {
    pub fn is_melee(&self) -> bool {
        self.riven_type == "Melee" || self.riven_type == "Zaw"
    }

    pub fn is_veiled(&self) -> bool {
        self.challenge.is_some() || self.weapon.is_none()
    }
}

const CHALLENGES: [(&str, &str); 33] = [
    ("RandomizedKill", "Kill {n} Enemies"),
    ("RandomizedKillPassengers", "Kill {n} Enemies that are on a Dropship"),
    ("RandomizedKillFallingPilots", "Kill {n} Enemies with Headshots"),
    ("RandomizedFinisherKill", "Kill {n} Enemies with Finishers"),
    ("RandomizedHeadshot", "Kill {n} Enemies with Headshots"),
    ("RandomizedHeadshotGlide", "Get {n} Headshot kills in a single Aim Glide"),
    ("RandomizedStyleKill", "Kill {n} Enemies while Sliding"),
    ("RandomizedWallClingKillstreak", "Get {n} kills in a row while Wall Dashing or Wall Latching without touching the floor"),
    ("RandomizedHeadshotUnawareBallistas", "Kill {n} unalerted Tusk Ballistas with a Headshot"),
    ("RandomizedKillSentients", "Get the killing blow on {n} Sentients"),
    ("RandomizedLongRangeSniper", "Kill {n} Enemies with Headshots from at least 75m away"),
    ("RandomizedSkiffArcher", "Destroy {n} Dargyns in flight using a bow"),
    ("RandomizedAntiAntiAir", "Destroy {n} Vruush Turrets while in Archwing without dying or becoming downed"),
    ("RandomizedFlyingHeadshotSeries", "Land {n} consecutive headshots while in Archwing in the Plains of Eidolon"),
    ("RandomizedFindCaches", "Find {n} Caches"),
    ("RandomizedFindRareMedallions", "Pick up {n} Syndicate Medallions"),
    ("RandomizedFisherman", "Catch {n} fish without missing a throw"),
    ("SustainMeleeComboThree", "Sustain a 6x Melee Combo Multiplier for 30s"),
    ("HighSurvivalPacifist", "Complete a Survival mission with level 30 or higher enemies without killing anyone"),
    ("HighExterminationUndetected", "Complete an Extermination mission with level 30 or higher enemies without being detected"),
    ("HighPerfectDefense", "Complete a Defense mission with level 30 or higher enemies with the defense objective taking no damage"),
    ("HighSoloInterceptionHobbled", "Complete a Solo Interception mission with level 30 or higher enemies and a Hobbled Dragon Key equipped"),
    ("LimitedSynthesis", "Synthesize a Simaris target without using Traps or Abilities while having a Hobbled Dragon Key equipped"),
    ("PlainsTimedVariety", "Catch one fish, mine one gem or metal, and kill one enemy in 30 seconds"),
    ("KahlMissions", "Complete {n} Kahl missions"),
    ("DJRandomizedKill", "Kill {n} Enemies"),
    ("DJRandomizedFinisherKill", "Kill {n} Enemies with Finishers"),
    ("DJRandomizedHeadshot", "Kill {n} Enemies with Headshots"),
    ("DJRandomizedStyleKill", "Kill {n} Enemies while Sliding"),
    ("DJRandomizedWallClingKillstreak", "Get {n} kills in a row while Wall Dashing or Wall Latching without touching the floor"),
    ("DJRandomizedKillPassengers", "Kill {n} Enemies that are on a Dropship"),
    ("DJRandomizedHeadshotUnawareBallistas", "Kill {n} unalerted Tusk Ballistas with a Headshot"),
    ("DJRandomizedFindCaches", "Find {n} Caches"),
];

const COMPLICATIONS: [(&str, &str); 25] = [
    ("ResetOnDamageTaken", "without taking damage"),
    ("ResetOnDowned", "without dying or becoming downed"),
    ("ResetOnMissionFailure", "without failing a mission"),
    ("ResetOnAlarmRaised", "without raising any alarms"),
    ("ResetOnAllyDowned", "without an ally becoming downed"),
    ("ResetOnDisrupt", "without being disrupted by a Magnetic Status Effect"),
    ("ResetOnGearAirSupport", "without using air support"),
    ("ResetOnGearAmmoRestores", "without using ammo consumables"),
    ("ResetOnGearCipher", "without using ciphers"),
    ("ResetOnGearEnergyRestores", "without using energy consumables"),
    ("ResetOnGearHealthRestores", "without using health consumables"),
    ("ResetOnGearShieldRestores", "without using shield-restoring consumables"),
    ("ResetOnProc", "without getting afflicted by a Status Effect"),
    ("ResetOnNewDay", "in one day"),
    ("EquippedDamageDebuffKey", "with an Extinguished Dragon Key equipped"),
    ("EquippedHealthDebuffKey", "with a Bleeding Dragon Key equipped"),
    ("EquippedShieldDebuffKey", "with a Decaying Dragon Key equipped"),
    ("EquippedSpeedDebuffKey", "with a Hobbled Dragon Key equipped"),
    ("SoloPlayer", ", while alone or in Solo Mode"),
    ("PetPresent", "with an active pet present"),
    ("SentinelPresent", "with an active sentinel present"),
    ("Invisible", "while invisible"),
    ("AimGliding", "during Aim Glide"),
    ("Sliding", "while sliding"),
    ("Undetected", "while undetected"),
];

fn leaf(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// "RandomizedFooBar" → "Randomized Foo Bar" for challenges the table lacks.
fn split_camel(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_uppercase() && !out.is_empty() {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

fn parse_challenge(ch: &Value) -> RivenChallenge {
    let progress = ch.get("Progress").and_then(Value::as_u64).map(|v| v as u32);
    let required = ch.get("Required").and_then(Value::as_u64).map(|v| v as u32);
    let kind = ch.get("Type").and_then(Value::as_str).map(leaf).unwrap_or("");
    let template = CHALLENGES.iter().find(|(k, _)| *k == kind).map(|(_, t)| *t);
    let n = required.map(|r| r.to_string()).unwrap_or_else(|| "?".into());
    let mut description = match template {
        Some(t) => t.replace("{n}", &n),
        None => split_camel(kind),
    };
    if let Some(text) = ch
        .get("Complication")
        .and_then(Value::as_str)
        .and_then(|c| COMPLICATIONS.iter().find(|(k, _)| *k == leaf(c)))
        .map(|(_, t)| *t)
    {
        description = format!("{description} {text}");
    }
    let group = match template {
        Some(t) => t.replace(" {n}", "").replace("{n} ", "").trim().to_string(),
        None => split_camel(kind),
    };
    RivenChallenge { description, group, progress, required }
}

/// Unveiled rivens live in `Upgrades` under `/Upgrades/Mods/Randomized/`.
pub fn is_riven_type(item_type: &str) -> bool {
    item_type.contains("/Upgrades/Mods/Randomized/")
        && !item_type.rsplit('/').next().unwrap_or("").starts_with("Raw")
}

pub fn riven_type_label(item_type: &str) -> &'static str {
    let tail = item_type.rsplit('/').next().unwrap_or("");
    match () {
        _ if tail.contains("ModularPistol") => "Kitgun",
        _ if tail.contains("ModularMelee") => "Zaw",
        _ if tail.contains("Rifle") => "Rifle",
        _ if tail.contains("Shotgun") => "Shotgun",
        _ if tail.contains("Pistol") => "Pistol",
        _ if tail.contains("Melee") => "Melee",
        _ if tail.contains("Archgun") || tail.contains("SpaceGun") => "Archgun",
        _ if tail.contains("Pet") || tail.contains("Sentinel") => "Companion",
        _ => "Riven",
    }
}

fn stats(list: Option<&Value>) -> Vec<RivenStat> {
    list.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|b| {
                    Some(RivenStat {
                        tag: b.get("Tag")?.as_str()?.to_string(),
                        roll: (b.get("Value")?.as_f64()? / FINGERPRINT_MAX).clamp(0.0, 1.0),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_owned(entry: &Value) -> Option<OwnedRiven> {
    let item_type = entry.get("ItemType")?.as_str()?.to_string();
    let fp: Value = match entry.get("UpgradeFingerprint")? {
        Value::String(s) => serde_json::from_str(s).ok()?,
        other => other.clone(),
    };
    let u = |k: &str| fp.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
    Some(OwnedRiven {
        item_id: entry.get("ItemId").and_then(|v| v.get("$oid")).and_then(Value::as_str).map(str::to_string),
        riven_type: riven_type_label(&item_type).to_string(),
        weapon: fp.get("compat").and_then(Value::as_str).map(str::to_string),
        mastery_req: u("lvlReq"),
        rank: u("lvl"),
        rerolls: u("rerolls"),
        polarity: fp.get("pol").and_then(Value::as_str).map(str::to_string),
        buffs: stats(fp.get("buffs")),
        curses: stats(fp.get("curses")),
        challenge: fp.get("challenge").filter(|c| c.is_object()).map(parse_challenge),
        item_type,
    })
}

// Stat math (original: `services/rivenFingerprint.ts`, `rivenConstants.ts`,
// from browse.wf's RivenParser).
const NUM_BUFFS_ATTEN: [f64; 6] = [0.0, 1.0, 0.66000003, 0.5, 0.40000001, 0.34999999];
const NUM_BUFFS_CURSE_ATTEN: [f64; 6] = [0.0, 1.0, 0.33000001, 0.5, 1.25, 1.5];
const SPECIFIC_FIT_ATTEN: f64 = 1.5;
const BASE_DRAIN: f64 = 10.0;
pub const MAX_RANK: u32 = 8;

/// One stat as the card prints it.
#[derive(Debug, Clone)]
pub struct DecodedStat {
    pub tag: String,
    pub name: String,
    /// Card value: a percentage or plain number, or a factor for multipliers.
    pub value: f64,
    /// Printed as "x1.08" (faction damage, Initial Combo).
    pub multiplier: bool,
    /// "%", "s" or "".
    pub unit: &'static str,
    pub positive: bool,
    /// 0..1 inside the stat's roll range (curses: lower is better).
    pub roll: f64,
    pub grade: &'static str,
}

impl DecodedStat {
    /// "+184.8%", "-10%", "x1.08", "+3.7".
    pub fn value_text(&self) -> String {
        card_text(self.value, self.multiplier, self.unit)
    }
}

/// A card value as printed: "+184.8%", "-10%", "x1.08", "+3.7".
fn card_text(value: f64, multiplier: bool, unit: &str) -> String {
    if multiplier {
        return format!("x{}", trim_float(value));
    }
    let sign = if value >= 0.0 { "+" } else { "" };
    format!("{sign}{}{unit}", trim_float(value))
}

fn trim_float(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// An unveiled riven with real card values and grades.
#[derive(Debug, Clone)]
pub struct DecodedRiven {
    pub weapon_name: String,
    /// Generated name syllables ("Visi-cronicron").
    pub suffix: String,
    pub disposition: f64,
    pub stats: Vec<DecodedStat>,
    /// Letter grade of the average roll (S, A+ … C-, F).
    pub grade: &'static str,
    /// Average roll quality 0..1 (curses inverted): "N% perfect".
    pub perfect: f64,
    /// Against the community good-rolls sheet; None when the weapon has no row.
    pub attr_grade: Option<super::riven_good_rolls::AttrGrade>,
    /// Endo it dissolves for.
    pub endo: u32,
}

impl DecodedRiven {
    /// Endo worth showing: weak attributes plus enough rerolls or rank to matter
    /// (original: `rivenDissolveHint`).
    pub fn dissolve_hint(&self, riven: &OwnedRiven) -> Option<u32> {
        use super::riven_good_rolls::AttrGrade;
        matches!(self.attr_grade, Some(AttrGrade::Bad | AttrGrade::Ok))
            .then_some(self.endo)
            .filter(|e| riven.rerolls >= 5 || *e >= 3000)
    }
}

/// Share of stats two rivens have in common, as a percentage of all distinct
/// stats (original: `computeRivenStatSimilarity`), compared on warframe.market
/// attribute names. Also returns which listing attributes matched.
pub fn similarity(my_tags: &[&str], listing_attrs: &[String]) -> (u32, Vec<bool>) {
    let mine: Vec<&str> = my_tags.iter().filter_map(|t| super::wfm_riven_search::wfm_attribute(t)).collect();
    if mine.is_empty() || listing_attrs.is_empty() {
        return (0, vec![false; listing_attrs.len()]);
    }
    let matched: Vec<bool> = listing_attrs.iter().map(|a| mine.contains(&a.as_str())).collect();
    let hits = matched.iter().filter(|m| **m).count();
    let union = mine.len() + listing_attrs.len() - hits;
    (((hits as f64 / union as f64) * 100.0).round() as u32, matched)
}

/// Letter grade of a roll (original: `floatToGrade`).
pub fn roll_grade(roll: f64, curse: bool) -> &'static str {
    let f = if curse { 1.0 - roll } else { roll };
    let score = -10.0 + 20.0 * f;
    const GRADES: [(f64, &str); 10] = [
        (9.5, "S"),
        (7.5, "A+"),
        (5.5, "A"),
        (3.5, "A-"),
        (1.5, "B+"),
        (-1.5, "B"),
        (-3.5, "B-"),
        (-5.5, "C+"),
        (-7.5, "C"),
        (-9.5, "C-"),
    ];
    GRADES.iter().find(|(min, _)| score >= *min).map(|(_, g)| *g).unwrap_or("F")
}

/// Endo from dissolving (original: `rivenDissolveEndo`).
pub fn dissolve_endo(mastery: u32, rank: u32, rerolls: u32) -> u32 {
    let mr = mastery.clamp(8, 16) as i64;
    let rank = rank.min(8);
    let total = 100 * (mr - 8) + (22.5 * 2f64.powi(rank as i32)).floor() as i64 + 200 * rerolls as i64 - 7;
    total.max(0) as u32
}

fn is_multiplier(tag: &str) -> bool {
    tag.contains("FactionDamage") || tag == "WeaponMeleeComboInitialBonusMod"
}

fn round_stat(raw: f64, percent: bool) -> f64 {
    if percent { (raw * 1000.0).round() / 10.0 } else { (raw * 100.0).round() / 100.0 }
}

/// Where a stat sits on a riven: buff or curse, how many of each the riven
/// has, its rank and its weapon's disposition.
#[derive(Debug, Clone, Copy)]
pub struct StatContext {
    pub positive: bool,
    pub buffs: usize,
    pub curses: usize,
    pub rank: u32,
    pub disposition: f64,
}

/// The card value of one stat (`roll` 0..1 inside its range), as `decode` and
/// the calculator print it.
pub fn card_value(entry: Option<&super::riven_data::RivenEntry>, tag: &str, roll: f64, cx: StatContext) -> f64 {
    let base = entry.map(|e| e.base).unwrap_or(0.0);
    let percent = entry.map(|e| e.unit).unwrap_or("%") == "%";
    let multiplier = is_multiplier(tag);
    let atten = SPECIFIC_FIT_ATTEN * cx.disposition * BASE_DRAIN;
    let lvl = (cx.rank + 1) as f64;
    let roll_mul = 0.9 + 0.2 * roll;
    let (nb, nc) = (cx.buffs.min(5), cx.curses.min(5));
    if cx.positive {
        let raw = base * atten * 1.25f64.powi(nc as i32) * roll_mul * NUM_BUFFS_ATTEN[nb] * lvl;
        let v = round_stat(raw, percent && !multiplier);
        if multiplier { ((1.0 + v) * 100.0).round() / 100.0 } else { v }
    } else {
        let raw = base.abs() * atten * roll_mul * NUM_BUFFS_CURSE_ATTEN[nb] * NUM_BUFFS_ATTEN[nc] * lvl;
        let m = round_stat(raw, percent && !multiplier);
        if multiplier {
            ((1.0 - m) * 100.0).round() / 100.0
        } else if base > 0.0 {
            -m
        } else {
            m
        }
    }
}

/// Riven disposition: 1.2 for every weapon that takes one since DE flattened
/// dispositions (checked against the live export).
pub const DISPOSITION: f64 = 1.2;

/// The stat counts a riven can roll.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Setup {
    #[default]
    TwoBuffs,
    TwoBuffsCurse,
    ThreeBuffs,
    ThreeBuffsCurse,
}

impl Setup {
    pub const ALL: [Setup; 4] = [Setup::TwoBuffs, Setup::TwoBuffsCurse, Setup::ThreeBuffs, Setup::ThreeBuffsCurse];

    pub fn label(self) -> &'static str {
        match self {
            Setup::TwoBuffs => "2 buffs",
            Setup::TwoBuffsCurse => "2 buffs + curse",
            Setup::ThreeBuffs => "3 buffs",
            Setup::ThreeBuffsCurse => "3 buffs + curse",
        }
    }

    pub fn buffs(self) -> usize {
        if matches!(self, Setup::ThreeBuffs | Setup::ThreeBuffsCurse) { 3 } else { 2 }
    }

    pub fn curses(self) -> usize {
        usize::from(matches!(self, Setup::TwoBuffsCurse | Setup::ThreeBuffsCurse))
    }

    /// (buff, curse) strength at max rank, as a multiple of base × disposition × 90
    /// (the usual way to quote them): buffs ×0.99, ×1.2375, ×0.75, ×0.9375 and
    /// the curse ×0.495, ×0.75, in [`Setup::ALL`] order.
    pub fn factors(self) -> (f64, Option<f64>) {
        let (nb, nc) = (self.buffs(), self.curses());
        let max_rank = (MAX_RANK + 1) as f64;
        let scale = SPECIFIC_FIT_ATTEN * BASE_DRAIN * max_rank / 90.0;
        let buff = scale * 1.25f64.powi(nc as i32) * NUM_BUFFS_ATTEN[nb];
        let curse = (nc > 0).then(|| scale * NUM_BUFFS_CURSE_ATTEN[nb] * NUM_BUFFS_ATTEN[nc]);
        (buff, curse)
    }
}

/// A stat's card values at the worst and best roll.
#[derive(Debug, Clone)]
pub struct StatRange {
    pub tag: String,
    pub name: String,
    /// Card values at roll 0 and roll 1; for a curse the second is the harsher one.
    pub low: f64,
    pub high: f64,
    multiplier: bool,
    unit: &'static str,
}

impl StatRange {
    /// "+90.1% to +110.2%".
    pub fn text(&self) -> String {
        format!("{} to {}", card_text(self.low, self.multiplier, self.unit), card_text(self.high, self.multiplier, self.unit))
    }
}

/// Every stat a riven type can roll, as a buff and (if the setup has one) as a
/// curse, with its range at `rank` (original: browse.wf's riven calculator).
pub fn stat_ranges(tables: &super::riven_data::RivenTables, riven_type: &str, setup: Setup, rank: u32) -> (Vec<StatRange>, Vec<StatRange>) {
    let range = |positive: bool| -> Vec<StatRange> {
        let cx = StatContext { positive, buffs: setup.buffs(), curses: setup.curses(), rank, disposition: DISPOSITION };
        tables
            .entries(riven_type)
            .iter()
            .map(|e| {
                let multiplier = is_multiplier(&e.tag);
                StatRange {
                    tag: e.tag.clone(),
                    name: e.name.clone(),
                    low: card_value(Some(e), &e.tag, 0.0, cx),
                    high: card_value(Some(e), &e.tag, 1.0, cx),
                    multiplier,
                    unit: if multiplier { "" } else { e.unit },
                }
            })
            .collect()
    };
    (range(true), if setup.curses() > 0 { range(false) } else { Vec::new() })
}

/// Card values, grades and name for an unveiled riven; None when its weapon or
/// its disposition is missing from the game data.
pub fn decode(
    riven: &OwnedRiven,
    tables: &super::riven_data::RivenTables,
    good_rolls: Option<&super::riven_good_rolls::GoodRolls>,
) -> Option<DecodedRiven> {
    let compat = riven.weapon.as_deref()?;
    let disposition = tables.disposition(compat)?;
    let weapon_name = tables.weapon_name(compat)?.to_string();
    let (nb, nc) = (riven.buffs.len(), riven.curses.len());
    let mut stats = Vec::new();
    for (stat, positive) in riven.buffs.iter().map(|s| (s, true)).chain(riven.curses.iter().map(|s| (s, false))) {
        let entry = tables.entry(&riven.item_type, &stat.tag);
        let unit = entry.map(|e| e.unit).unwrap_or("%");
        let multiplier = is_multiplier(&stat.tag);
        let cx = StatContext { positive, buffs: nb, curses: nc, rank: riven.rank, disposition };
        let value = card_value(entry, &stat.tag, stat.roll, cx);
        stats.push(DecodedStat {
            name: entry.map(|e| e.name.clone()).unwrap_or_else(|| stat.label(riven.is_melee())),
            tag: stat.tag.clone(),
            value,
            multiplier,
            unit: if multiplier { "" } else { unit },
            positive,
            roll: stat.roll,
            grade: roll_grade(stat.roll, !positive),
        });
    }
    let scored: Vec<f64> = riven.buffs.iter().map(|b| b.roll).chain(riven.curses.iter().map(|c| 1.0 - c.roll)).collect();
    let avg = if scored.is_empty() { 0.5 } else { scored.iter().sum::<f64>() / scored.len() as f64 };
    let attr_grade = good_rolls.and_then(|g| g.get(&weapon_name, tables)).map(|data| {
        let buffs: Vec<&str> = riven.buffs.iter().map(|b| b.tag.as_str()).collect();
        let curses: Vec<&str> = riven.curses.iter().map(|c| c.tag.as_str()).collect();
        super::riven_good_rolls::grade(data, &buffs, &curses)
    });
    let buffs: Vec<(String, f64)> = riven.buffs.iter().map(|b| (b.tag.clone(), b.roll)).collect();
    Some(DecodedRiven {
        weapon_name,
        suffix: tables.suffix(&riven.item_type, &buffs),
        disposition,
        stats,
        grade: roll_grade(avg, false),
        perfect: avg,
        attr_grade,
        endo: dissolve_endo(riven.mastery_req, riven.rank, riven.rerolls),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_factors() {
        let f: Vec<(f64, Option<f64>)> = Setup::ALL.iter().map(|s| s.factors()).collect();
        let near = |a: f64, b: f64| (a - b).abs() < 1e-4;
        let expected = [(0.99, None), (1.2375, Some(0.495)), (0.75, None), (0.9375, Some(0.75))];
        for ((buff, curse), (eb, ec)) in f.iter().zip(expected) {
            assert!(near(*buff, eb), "{buff} vs {eb}");
            assert_eq!(curse.is_some(), ec.is_some());
            if let (Some(c), Some(e)) = (curse, ec) {
                assert!(near(*c, e), "{c} vs {e}");
            }
        }
    }

    #[test]
    fn card_value_matches_setup_factor() {
        let entry = super::super::riven_data::RivenEntry {
            tag: "WeaponCritChanceMod".into(),
            base: 0.016666,
            name: "Critical Chance".into(),
            unit: "%",
            prefix: String::new(),
            suffix: String::new(),
        };
        // Rank 8 rifle crit chance, 2 buffs: 0.016666 × 1.2 × 90 × 0.99 = 178.2%, ±10%.
        let cx = StatContext { positive: true, buffs: 2, curses: 0, rank: MAX_RANK, disposition: DISPOSITION };
        assert_eq!(card_value(Some(&entry), &entry.tag, 0.0, cx), 160.4);
        assert_eq!(card_value(Some(&entry), &entry.tag, 1.0, cx), 196.0);
        // As the curse of 2 buffs + curse: 0.495 of the same, negative.
        let curse = StatContext { positive: false, curses: 1, ..cx };
        assert_eq!(card_value(Some(&entry), &entry.tag, 1.0, curse), -98.0);
    }

    #[test]
    fn every_stat_has_a_label_and_market_name() {
        for tag in crate::config::game_tables::RIVEN_STAT_TAGS {
            assert_ne!(crate::config::game_tables::riven_stat_label(tag, false), tag, "no label for {tag}");
            assert!(crate::services::wfm_riven_search::wfm_attribute(tag).is_some(), "no warframe.market name for {tag}");
        }
        assert!(super::is_multiplier("WeaponFactionDamageTechrot"), "new factions print as x1.05");
    }

    #[test]
    fn ammo_efficiency_reads_like_heavy_attack_efficiency() {
        use super::{OwnedRiven, RivenStat, decode};
        use crate::services::public_export::ExportSet;
        use serde_json::json;
        let rifle = "/Lotus/Upgrades/Mods/Randomized/LotusRifleRandomModRare";
        let entry = |tag: &str, value: f64, loc: &str| json!({"tag": tag, "upgradeValues": [{"value": value, "locTag": loc}]});
        let exports = ExportSet {
            files: [
                ("ExportWeapons".to_string(), json!({"ExportWeapons": [{"uniqueName": "/W/Gun", "name": "Gun", "omegaAttenuation": 1.2}]})),
                ("ExportUpgrades".to_string(), json!({"ExportUpgrades": [{"uniqueName": rifle, "upgradeEntries": [
                    entry("WeaponAmmoEfficiency", -0.005, "|val|% Ammo Efficiency"),
                    entry("WeaponRecoilReductionMod", -0.0099, "|val|% Weapon Recoil"),
                ]}]})),
            ]
            .into(),
        };
        let tables = crate::services::riven_data::RivenTables::build(&exports);
        let stat = |tag: &str| RivenStat { tag: tag.into(), roll: 0.5 };
        let riven = |buffs: Vec<RivenStat>, curses: Vec<RivenStat>| OwnedRiven {
            item_id: None,
            item_type: rifle.into(),
            riven_type: "Rifle".into(),
            weapon: Some("/W/Gun".into()),
            mastery_req: 8,
            rank: 8,
            rerolls: 0,
            polarity: None,
            buffs,
            curses,
            challenge: None,
        };
        let buff = decode(&riven(vec![stat("WeaponAmmoEfficiency")], vec![]), &tables, None).unwrap();
        assert!(buff.stats[0].value > 0.0, "buff reads +X% Ammo Efficiency");
        assert!(buff.stats[0].value_text().starts_with('+'));
        let curse = decode(&riven(vec![stat("WeaponRecoilReductionMod")], vec![stat("WeaponAmmoEfficiency")]), &tables, None).unwrap();
        assert!(curse.stats[1].value < 0.0, "curse reads -X% Ammo Efficiency");
        assert!(curse.stats[0].value < 0.0, "recoil buff still reads -X% Weapon Recoil");
    }

    #[test]
    fn similarity() {
        let attrs = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let (pct, matched) = super::similarity(
            &["WeaponCritChanceMod", "WeaponCritDamageMod", "WeaponZoomFovMod"],
            &attrs(&["critical_chance", "critical_damage", "multishot"]),
        );
        assert_eq!(pct, 50, "2 shared of 4 distinct");
        assert_eq!(matched, vec![true, true, false]);
        assert_eq!(super::similarity(&["WeaponMeleeDamageMod"], &attrs(&["base_damage_/_melee_damage"])).0, 100);
    }

    #[test]
    fn riven_types() {
        assert!(super::is_riven_type("/Lotus/Upgrades/Mods/Randomized/LotusRifleRandomModRare"));
        assert!(!super::is_riven_type("/Lotus/Upgrades/Mods/Randomized/RawRifleRandomMod"));
        assert_eq!(super::riven_type_label("/Lotus/Upgrades/Mods/Randomized/LotusModularPistolRandomModRare"), "Kitgun");
        assert_eq!(super::riven_type_label("/Lotus/Upgrades/Mods/Randomized/PlayerMeleeWeaponRandomModRare"), "Melee");
    }
}
