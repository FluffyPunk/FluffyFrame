//! 44bananas' community "good rolls" sheet and the attribute grade it gives a
//! riven (original: `services/rivenBestAttributes.ts`, `config/shared/rivenGoodRolls.ts`,
//! `gradeFromGoodRolls` in `services/rivenGrading.ts`).

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::riven_data::{RivenTables, slug};
use super::{http, json_cache};

const SHEET_ID: &str = "1LJ83e4x_xIVgjZg049PKrDgTex2MIuaiX0FFZi9mH-Q";
const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Primary,
    Secondary,
    Melee,
    Archgun,
    Robotic,
}

const TABS: [(u64, Class); 5] = [
    (0, Class::Primary),
    (1505239276, Class::Secondary),
    (1413904270, Class::Melee),
    (289737427, Class::Archgun),
    (965095749, Class::Robotic),
];

/// One acceptable roll: every mandatory stat, plus any of the optional ones.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GoodRoll {
    pub mandatory: Vec<String>,
    pub optional: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GoodRollData {
    pub good: Vec<GoodRoll>,
    /// Curses the sheet tolerates on this weapon.
    pub accepted_bad: Vec<String>,
}

impl GoodRollData {
    /// Wanted buffs, mandatory ones first, without repeats (original: `getBestAttributes`).
    pub fn wanted(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for list in [self.good.iter().flat_map(|g| &g.mandatory).collect::<Vec<_>>(), self.good.iter().flat_map(|g| &g.optional).collect()] {
            for tag in list {
                if !out.contains(&tag.as_str()) {
                    out.push(tag);
                }
            }
        }
        out
    }
}

/// The sheet, keyed by lowercase weapon (family) name.
#[derive(Debug, Default)]
pub struct GoodRolls {
    by_name: HashMap<String, GoodRollData>,
    by_slug: HashMap<String, String>,
}

/// GOOD/BAD badge on a riven card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttrGrade {
    Bad,
    Ok,
    Good,
    Great,
}

impl AttrGrade {
    pub const ALL: [AttrGrade; 4] = [AttrGrade::Great, AttrGrade::Good, AttrGrade::Ok, AttrGrade::Bad];

    pub fn label(self) -> &'static str {
        match self {
            AttrGrade::Great => "Great",
            AttrGrade::Good => "Good",
            AttrGrade::Ok => "OK",
            AttrGrade::Bad => "Bad",
        }
    }
}

fn tags_for(abbrev: &str, class: Class) -> Vec<&'static str> {
    let tag = match abbrev.trim().to_uppercase().as_str() {
        "" => return vec![],
        "DMG" if class == Class::Melee => "WeaponMeleeDamageMod",
        "DMG" => "WeaponDamageAmountMod",
        "ELEMENT" => {
            return vec!["WeaponFireDamageMod", "WeaponFreezeDamageMod", "WeaponElectricityDamageMod", "WeaponToxinDamageMod"];
        }
        "CC" => "WeaponCritChanceMod",
        "CD" => "WeaponCritDamageMod",
        "MS" => "WeaponFireIterationsMod",
        "FR" | "AS" => "WeaponFireRateMod",
        "RLS" => "WeaponReloadSpeedMod",
        "SC" => "WeaponStunChanceMod",
        "SD" => "WeaponProcTimeMod",
        "PT" => "WeaponPunctureDepthMod",
        "MAG" => "WeaponClipMaxMod",
        "AMMO" => "WeaponAmmoMaxMod",
        // The sheet spells it out once.
        "REC" | "RECOIL" => "WeaponRecoilReductionMod",
        "ZOOM" => "WeaponZoomFovMod",
        "PFS" => "WeaponProjectileSpeedMod",
        "IMP" => "WeaponImpactDamageMod",
        "PUNC" => "WeaponArmorPiercingDamageMod",
        "SLASH" => "WeaponSlashDamageMod",
        "COLD" => "WeaponFreezeDamageMod",
        "HEAT" => "WeaponFireDamageMod",
        "ELEC" => "WeaponElectricityDamageMod",
        "TOX" => "WeaponToxinDamageMod",
        "DTG" => "WeaponFactionDamageGrineer",
        "DTC" => "WeaponFactionDamageCorpus",
        "DTI" => "WeaponFactionDamageInfested",
        "RANGE" => "WeaponMeleeRangeIncMod",
        "IC" => "WeaponMeleeComboInitialBonusMod",
        "EFF" => "WeaponMeleeComboEfficiencyMod",
        "SLIDE" => "SlideAttackCritChanceMod",
        "FIN" => "WeaponMeleeFinisherDamageMod",
        _ => return vec![],
    };
    vec![tag]
}

/// The sheet names melee faction damage like the gun stat.
pub fn sheet_tag(tag: &str) -> String {
    tag.replace("WeaponMeleeFaction", "WeaponFaction")
}

fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = vec![vec![]];
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => quoted = false,
                _ => field.push(c),
            }
        } else {
            match c {
                '"' => quoted = true,
                ',' => rows.last_mut().unwrap().push(std::mem::take(&mut field)),
                '\n' => {
                    rows.last_mut().unwrap().push(std::mem::take(&mut field));
                    rows.push(vec![]);
                }
                '\r' => {}
                _ => field.push(c),
            }
        }
    }
    if !field.is_empty() || !rows.last().unwrap().is_empty() {
        rows.last_mut().unwrap().push(field);
    }
    rows.retain(|r| r.len() > 1 || r.first().is_some_and(|f| !f.is_empty()));
    rows
}

/// "CC CD/MS or DMG MS" → alternatives; "/" marks optional stats.
fn parse_positives(cell: &str, class: Class) -> Vec<GoodRoll> {
    let tokens: Vec<&str> = cell.split_whitespace().collect();
    let mut rolls = Vec::new();
    for alt in tokens.split(|t| t.eq_ignore_ascii_case("or")) {
        let mut mandatory: Vec<String> = Vec::new();
        let mut optional: Vec<String> = Vec::new();
        for token in alt {
            let target = if token.contains('/') { &mut optional } else { &mut mandatory };
            for part in token.split('/') {
                for tag in tags_for(part, class) {
                    if !target.iter().any(|t| t == tag) {
                        target.push(tag.to_string());
                    }
                }
            }
        }
        optional.retain(|t| !mandatory.contains(t));
        if !mandatory.is_empty() || !optional.is_empty() {
            rolls.push(GoodRoll { mandatory, optional });
        }
    }
    rolls
}

fn parse_negatives(cell: &str, class: Class) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for token in cell.split(|c: char| c == '/' || c.is_whitespace()) {
        for tag in tags_for(token, class) {
            if !out.iter().any(|t| t == tag) {
                out.push(tag.to_string());
            }
        }
    }
    out
}

fn parse_sheet(csv: &str, class: Class) -> crate::Result<Vec<(String, GoodRollData)>> {
    let rows = parse_csv(csv.trim_start_matches('\u{feff}'));
    let header: Vec<String> = rows.first().map(|r| r.iter().map(|h| h.trim().to_lowercase()).collect()).unwrap_or_default();
    let col = |p: &str| header.iter().position(|h| h.starts_with(p));
    let (Some(w), Some(pos), Some(neg)) = (col("weapon"), col("positive"), col("negative")) else {
        anyhow::bail!("unexpected good-rolls header for {class:?}: {header:?}");
    };
    fn cell(row: &[String], i: usize) -> &str {
        row.get(i).map(String::as_str).unwrap_or("")
    }
    Ok(rows
        .iter()
        .skip(1)
        .filter_map(|row| {
            let name = cell(row, w).trim().to_lowercase();
            let data = GoodRollData { good: parse_positives(cell(row, pos), class), accepted_bad: parse_negatives(cell(row, neg), class) };
            (!name.is_empty() && (!data.good.is_empty() || !data.accepted_bad.is_empty())).then_some((name, data))
        })
        .collect())
}

/// Affixes the sheet folds into the base row (warframe.market lists them apart).
const SHEET_ONLY_PREFIXES: [&str; 6] = ["coda ", "dex ", "mara ", "carmine ", "ceti ", "prime "];

impl GoodRolls {
    /// Downloads the five class tabs (cached a week; a stale copy beats nothing).
    pub fn load() -> crate::Result<Self> {
        let mut by_name = HashMap::new();
        for (gid, class) in TABS {
            let url = format!("https://docs.google.com/spreadsheets/d/{SHEET_ID}/export?format=csv&gid={gid}");
            let csv = json_cache::fetch_cached(&format!("riven-good-rolls-{gid}.csv"), MAX_AGE, || http::get_text(&url))?;
            for (name, data) in parse_sheet(&csv, class)? {
                by_name.entry(name).or_insert(data);
            }
        }
        let by_slug = by_name.keys().map(|k| (slug(&k.replace('&', " and ")), k.clone())).collect();
        Ok(GoodRolls { by_name, by_slug })
    }

    /// The sheet row for a weapon: exact name, then without a sheet-only prefix,
    /// then by riven family (variants share their base weapon's row).
    pub fn get(&self, weapon_name: &str, tables: &RivenTables) -> Option<&GoodRollData> {
        let lc = weapon_name.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
        if let Some(d) = self.by_name.get(&lc) {
            return Some(d);
        }
        let folded = SHEET_ONLY_PREFIXES.iter().find_map(|p| lc.strip_prefix(p)).unwrap_or(&lc).trim();
        if let Some(d) = self.by_name.get(folded) {
            return Some(d);
        }
        let key = self.by_slug.get(&tables.family_slug(folded))?;
        self.by_name.get(key)
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

/// Overall attribute grade from the buffs and curses a riven rolled.
pub fn grade(data: &GoodRollData, buffs: &[&str], curses: &[&str]) -> AttrGrade {
    let buffs: Vec<String> = buffs.iter().map(|t| sheet_tag(t)).collect();
    let curses: Vec<String> = curses.iter().map(|t| sheet_tag(t)).collect();
    let in_any = |tag: &str, optional_too: bool| {
        data.good.iter().any(|g| g.mandatory.iter().any(|m| m == tag) || (optional_too && g.optional.iter().any(|o| o == tag)))
    };
    // Per curse: tolerated, hurts a wanted stat ("Bad"), or neither.
    let bad_curse = curses.iter().any(|c| !data.accepted_bad.contains(c) && in_any(c, true));
    let neutral_curse = curses.iter().any(|c| !data.accepted_bad.contains(c) && !in_any(c, true));
    let helping = buffs.iter().filter(|b| in_any(b, true)).count();
    let matches_roll = data.good.iter().any(|g| {
        g.mandatory.iter().all(|m| buffs.contains(m))
            && buffs.iter().all(|b| g.mandatory.contains(b) || g.optional.contains(b))
    });
    if bad_curse {
        if (matches_roll && helping >= 2) || helping >= 3 { AttrGrade::Ok } else { AttrGrade::Bad }
    } else if neutral_curse {
        if matches_roll || helping >= 2 {
            AttrGrade::Good
        } else if helping >= 1 {
            AttrGrade::Ok
        } else {
            AttrGrade::Bad
        }
    } else if matches_roll {
        if helping >= 2 && !curses.is_empty() { AttrGrade::Great } else { AttrGrade::Good }
    } else if helping >= 2 {
        AttrGrade::Good
    } else if helping >= 1 {
        AttrGrade::Ok
    } else {
        AttrGrade::Bad
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: &str = "Weapon,Positive Stats,Negative Stats,Notes\n\
        Rubico,\"CC CD/MS or DMG MS\",ZOOM/REC,x\n\
        Nikana,CC CD/DMG/ELEMENT,SLIDE IMP,\n";

    #[test]
    fn parses_sheet_rows() {
        let rows = parse_sheet(CSV, Class::Primary).unwrap();
        assert_eq!(rows.len(), 2);
        let (name, rubico) = &rows[0];
        assert_eq!(name, "rubico");
        assert_eq!(rubico.good.len(), 2);
        assert_eq!(rubico.good[0].mandatory, vec!["WeaponCritChanceMod"]);
        assert_eq!(rubico.good[0].optional, vec!["WeaponCritDamageMod", "WeaponFireIterationsMod"]);
        assert_eq!(rubico.accepted_bad, vec!["WeaponZoomFovMod", "WeaponRecoilReductionMod"]);
        let melee = parse_sheet(CSV, Class::Melee).unwrap();
        assert!(melee[1].1.good[0].optional.contains(&"WeaponMeleeDamageMod".to_string()));
        assert_eq!(melee[1].1.good[0].optional.len(), 6, "DMG + four elements + CD");
    }

    #[test]
    fn grades_like_the_original() {
        let rubico = &parse_sheet(CSV, Class::Primary).unwrap()[0].1;
        let g = |b: &[&str], c: &[&str]| grade(rubico, b, c);
        let (cc, cd, ms, zoom, slash) =
            ("WeaponCritChanceMod", "WeaponCritDamageMod", "WeaponFireIterationsMod", "WeaponZoomFovMod", "WeaponSlashDamageMod");
        assert_eq!(g(&[cc, cd, ms], &[zoom]), AttrGrade::Great);
        assert_eq!(g(&[cc, cd], &[]), AttrGrade::Good);
        assert_eq!(g(&[cc, cd], &[ms]), AttrGrade::Ok, "curse on a wanted stat");
        assert_eq!(g(&[slash], &[]), AttrGrade::Bad);
        assert_eq!(g(&[cc, slash], &[slash]), AttrGrade::Ok);
    }
}
