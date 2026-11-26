//! Static game naming tables (original: `config/shared/missionTypes.ts`,
//! `rivenStatDisplayNames.ts`, `relayNames.ts`, parts of `worldStateParser.ts`).

/// MT_* enum → label, matching DE's own dictionary.
pub fn mission_type_label(mt: &str) -> String {
    let label = match mt {
        "MT_ALCHEMY" => "Alchemy",
        "MT_ARENA" => "Arena",
        "MT_ARMAGEDDON" => "Void Armageddon",
        "MT_ARTIFACT" => "Disruption",
        "MT_ASSASSINATION" => "Assassination",
        "MT_ASSAULT" => "Assault",
        "MT_CAPTURE" => "Capture",
        "MT_CORRUPTION" => "Void Flood",
        "MT_COUNTER_INTEL" => "Deception",
        "MT_DEFENSE" => "Defense",
        "MT_ENDLESS_CAPTURE" => "Legacyte Harvest",
        "MT_ENDLESS_EXTERMINATION" => "Sanctuary Onslaught",
        "MT_EVACUATION" => "Defection",
        "MT_EXCAVATE" => "Excavation",
        "MT_EXTERMINATION" => "Exterminate",
        "MT_HIVE" => "Hive",
        "MT_INTEL" => "Spy",
        "MT_JUNCTION" => "Solar Rail Junction",
        "MT_LANDSCAPE" => "Free Roam",
        "MT_MOBILE_DEFENSE" => "Mobile Defense",
        "MT_PURIFY" => "Infested Salvage",
        "MT_PURSUIT" => "Pursuit",
        "MT_PVP" => "Conclave",
        "MT_RACE" => "Rush",
        "MT_RAILJACK" => "Railjack",
        "MT_RESCUE" => "Rescue",
        "MT_RETRIEVAL" => "Hijack",
        "MT_SABOTAGE" => "Sabotage",
        "MT_SURVIVAL" => "Survival",
        "MT_TERRITORY" => "Interception",
        "MT_VAULTS" => "Netracell",
        "MT_VOID_CASCADE" => "Void Cascade",
        other => return title_case(other.trim_start_matches("MT_").replace('_', " ").as_str()),
    };
    label.to_string()
}

/// ExportRegions `missionIndex` → MT_* enum. Verified against live world state;
/// unknown indices return `None` instead of a guess.
pub fn mission_index_to_mt(index: i64) -> Option<&'static str> {
    Some(match index {
        0 => "MT_ASSASSINATION",
        1 => "MT_EXTERMINATION",
        2 => "MT_SURVIVAL",
        3 => "MT_RESCUE",
        4 => "MT_SABOTAGE",
        5 => "MT_CAPTURE",
        7 => "MT_INTEL",
        8 => "MT_DEFENSE",
        9 => "MT_MOBILE_DEFENSE",
        13 => "MT_TERRITORY",
        14 => "MT_RETRIEVAL",
        15 => "MT_HIVE",
        17 => "MT_EXCAVATE",
        21 => "MT_PURIFY",
        22 => "MT_ARENA",
        24 => "MT_PURSUIT",
        25 => "MT_RACE",
        26 => "MT_ASSAULT",
        27 => "MT_EVACUATION",
        28 => "MT_LANDSCAPE",
        33 => "MT_ARTIFACT",
        34 => "MT_CORRUPTION",
        35 => "MT_VOID_CASCADE",
        36 => "MT_ARMAGEDDON",
        38 => "MT_ALCHEMY",
        _ => return None,
    })
}

pub fn faction_index_label(index: i64) -> &'static str {
    match index {
        0 => "Grineer",
        1 => "Corpus",
        2 => "Infested",
        3 => "Corrupted",
        _ => "",
    }
}

pub fn faction_label(fc: &str) -> String {
    match fc {
        "FC_GRINEER" => "Grineer".into(),
        "FC_CORPUS" => "Corpus".into(),
        "FC_INFESTATION" => "Infested".into(),
        "FC_OROKIN" => "Corrupted".into(),
        "FC_SENTIENT" => "Sentient".into(),
        "FC_NARMER" => "Narmer".into(),
        other => title_case(&other.trim_start_matches("FC_").replace('_', " ")),
    }
}

/// Void fissure tier from the world-state `Modifier`.
pub fn void_tier(modifier: &str) -> Option<&'static str> {
    Some(match modifier.trim_end_matches("Hard") {
        "VoidT1" => "Lith",
        "VoidT2" => "Meso",
        "VoidT3" => "Neo",
        "VoidT4" => "Axi",
        "VoidT5" => "Requiem",
        "VoidT6" => "Omnia",
        _ => return None,
    })
}

/// Sort order for fissure tiers.
pub fn void_tier_order(tier: &str) -> u8 {
    match tier {
        "Lith" => 1,
        "Meso" => 2,
        "Neo" => 3,
        "Axi" => 4,
        "Requiem" => 5,
        "Omnia" => 6,
        _ => 9,
    }
}

/// Relay hub nodes Baro and Varzia visit.
pub fn hub_label(node: &str) -> String {
    match node {
        "EarthHUB" => "Strata Relay (Earth)".into(),
        "MercuryHUB" => "Larunda Relay (Mercury)".into(),
        "SaturnHUB" => "Kronia Relay (Saturn)".into(),
        "PlutoHUB" => "Orcus Relay (Pluto)".into(),
        "ErisHUB" => "Kuiper Relay (Eris)".into(),
        "EuropaHUB" => "Leonov Relay (Europa)".into(),
        "VenusHUB" => "Vesper Relay (Venus)".into(),
        "TennoConHUB2" | "TennoConHUB" => "TennoCon Relay".into(),
        other => other.into(),
    }
}

pub fn syndicate_label(tag: &str) -> String {
    let label = match tag {
        "ArbitersSyndicate" => "Arbiters of Hexis",
        "CephalonSudaSyndicate" => "Cephalon Suda",
        "NewLokaSyndicate" => "New Loka",
        "PerrinSyndicate" => "The Perrin Sequence",
        "RedVeilSyndicate" => "Red Veil",
        "SteelMeridianSyndicate" => "Steel Meridian",
        "CetusSyndicate" => "Ostrons",
        "SolarisSyndicate" => "Solaris United",
        "EntratiSyndicate" => "Entrati",
        "NecraloidSyndicate" => "Necraloid",
        "QuillsSyndicate" => "The Quills",
        "VentKidsSyndicate" => "Ventkids",
        "VoxSyndicate" => "Vox Solaris",
        "ZarimanSyndicate" => "The Holdfasts",
        "KahlSyndicate" => "Kahl's Garrison",
        "EntratiLabSyndicate" => "Cavia",
        "HexSyndicate" => "The Hex",
        "ConclaveSyndicate" => "Conclave",
        "CephalonSimarisSyndicate" | "LibrarySyndicate" => "Cephalon Simaris",
        "RadioLegionSyndicate" => "Nightwave",
        other if other.starts_with("RadioLegion") => "Nightwave",
        other => return title_case(&split_camel(other.trim_end_matches("Syndicate"))),
    };
    label.to_string()
}

/// The syndicate whose own daily standing cap is the inventory field
/// `DailyAffiliation<suffix>`. (The bare `DailyAffiliation` is the cap the six
/// aligned syndicates share.)
pub fn daily_cap_syndicate(suffix: &str) -> Option<&'static str> {
    Some(match suffix {
        "Pvp" => "ConclaveSyndicate",
        "Library" => "LibrarySyndicate",
        "Cetus" => "CetusSyndicate",
        "Quills" => "QuillsSyndicate",
        "Solaris" => "SolarisSyndicate",
        "Ventkids" => "VentKidsSyndicate",
        "Vox" => "VoxSyndicate",
        "Entrati" => "EntratiSyndicate",
        "Necraloid" => "NecraloidSyndicate",
        "Kahl" => "KahlSyndicate",
        "Zariman" => "ZarimanSyndicate",
        "Cavia" => "EntratiLabSyndicate",
        "Hex" => "HexSyndicate",
        _ => return None,
    })
}

/// The sigil (DE uniqueName) with a syndicate's base (rank 0) emblem, for the
/// syndicates DE's export has one for: the aligned six, Conclave and Simaris.
/// Open-world syndicates only have rank sigils there; their logos are bundled
/// with the app instead.
pub fn syndicate_emblem(tag: &str) -> Option<String> {
    let sigil = match tag {
        "ArbitersSyndicate" => "SyndicateSigilArbitersOfHexisA",
        "CephalonSudaSyndicate" => "SyndicateSigilCephalonSudaA",
        "NewLokaSyndicate" => "SyndicateSigilNewLokaA",
        "PerrinSyndicate" => "SyndicateSigilPerrinSequenceA",
        "RedVeilSyndicate" => "SyndicateSigilRedVeilA",
        "SteelMeridianSyndicate" => "SyndicateSigilSteelMeridianA",
        "ConclaveSyndicate" => "SyndicateSigilConclaveA",
        // "Cephalon Simaris Sigil" (the wiki's name), the Sanctuary Warden.
        "LibrarySyndicate" | "CephalonSimarisSyndicate" => "CollectorSigil",
        _ => return None,
    };
    Some(format!("/Lotus/Upgrades/Skins/Sigils/{sigil}"))
}

/// Every stat DE's riven tables can roll (ExportUpgrades, 2026-11), including
    /// the combined elements, newer factions and newer gun/melee stats.
pub const RIVEN_STAT_TAGS: [&str; 54] = [
    "ComboDurationMod", "SlideAttackCritChanceMod", "WeaponAmmoEfficiency", "WeaponAmmoMaxMod",
    "WeaponArmorPiercingDamageMod", "WeaponBlastDamageMod", "WeaponClipMaxMod", "WeaponCorrosiveDamageMod",
    "WeaponCritChanceMod", "WeaponCritDamageMod", "WeaponDamageAmountMod", "WeaponElectricityDamageMod",
    "WeaponFactionDamageCorpus", "WeaponFactionDamageGrineer", "WeaponFactionDamageInfested",
    "WeaponFactionDamageOrokin", "WeaponFactionDamageScaldra", "WeaponFactionDamageTechrot",
    "WeaponFireDamageMod", "WeaponFireIterationsMod", "WeaponFireRateMod", "WeaponFreezeDamageMod",
    "WeaponGasDamageMod", "WeaponImpactDamageMod", "WeaponMagazineReloadHolstered", "WeaponMagneticDamageMod",
    "WeaponMeleeComboBonusOnHitMod", "WeaponMeleeComboEfficiencyMod", "WeaponMeleeComboInitialBonusMod",
    "WeaponMeleeComboPointsOnHitMod", "WeaponMeleeDamageMod", "WeaponMeleeFactionDamageCorpus",
    "WeaponMeleeFactionDamageGrineer", "WeaponMeleeFactionDamageInfested", "WeaponMeleeFinisherDamageMod",
    "WeaponMeleeHeavyAttackChargeMod", "WeaponMeleeHeavyAttackDamageMod", "WeaponMeleeParryAngleMod",
    "WeaponMeleeRangeIncMod", "WeaponMeleeSlamDamageMod", "WeaponProcTimeMod", "WeaponProjectileSpeedMod",
    "WeaponPunctureDepthMod", "WeaponRadiationDamageMod", "WeaponRecoilReductionMod", "WeaponReloadSpeedMod",
    "WeaponSlashDamageMod", "WeaponStatusDamage", "WeaponStunChanceMod", "WeaponToxinDamageMod",
    "WeaponViralDamageMod", "WeaponWeakpointCriticalChance", "WeaponWeakpointDamage", "WeaponZoomFovMod",
];

/// Riven buff tag → display name.
pub fn riven_stat_label(tag: &str, melee: bool) -> String {
    if melee && tag == "WeaponFireRateMod" {
        return "Attack Speed".into();
    }
    let label = match tag {
        "WeaponCritChanceMod" => "Critical Chance",
        "WeaponCritDamageMod" => "Critical Damage",
        "WeaponFireIterationsMod" => "Multishot",
        "WeaponFireRateMod" => "Fire Rate",
        "WeaponDamageAmountMod" => "Damage",
        "WeaponMeleeDamageMod" => "Melee Damage",
        "WeaponReloadSpeedMod" => "Reload Speed",
        "WeaponStunChanceMod" => "Status Chance",
        "WeaponProcTimeMod" => "Status Duration",
        "WeaponPunctureDepthMod" => "Punch Through",
        "WeaponClipMaxMod" => "Magazine Capacity",
        "WeaponAmmoMaxMod" => "Ammo Maximum",
        "WeaponRecoilReductionMod" => "Weapon Recoil",
        "WeaponZoomFovMod" => "Zoom",
        "WeaponProjectileSpeedMod" => "Projectile Speed",
        "WeaponImpactDamageMod" => "Impact",
        "WeaponArmorPiercingDamageMod" => "Puncture",
        "WeaponSlashDamageMod" => "Slash",
        "WeaponFreezeDamageMod" => "Cold",
        "WeaponFireDamageMod" => "Heat",
        "WeaponElectricityDamageMod" => "Electricity",
        "WeaponToxinDamageMod" => "Toxin",
        "WeaponFactionDamageGrineer" => "Damage to Grineer",
        "WeaponFactionDamageCorpus" => "Damage to Corpus",
        "WeaponFactionDamageInfested" => "Damage to Infested",
        "WeaponMeleeRangeIncMod" => "Range",
        "ComboDurationMod" => "Combo Duration",
        "SlideAttackCritChanceMod" => "Slide Attack",
        "WeaponMeleeFinisherDamageMod" => "Finisher Damage",
        "WeaponMeleeComboEfficiencyMod" => "Heavy Attack Efficiency",
        "WeaponMeleeComboInitialBonusMod" => "Initial Combo",
        "WeaponMeleeComboPointsOnHitMod" => "Chance to Gain Combo Count",
        "WeaponMeleeComboBonusOnHitMod" => "Additional Combo Count Chance",
        "WeaponMeleeFactionDamageCorpus" => "Damage to Corpus",
        "WeaponMeleeFactionDamageGrineer" => "Damage to Grineer",
        "WeaponMeleeFactionDamageInfested" => "Damage to Infested",
        // Combined elements.
        "WeaponBlastDamageMod" => "Blast",
        "WeaponCorrosiveDamageMod" => "Corrosive",
        "WeaponGasDamageMod" => "Gas",
        "WeaponMagneticDamageMod" => "Magnetic",
        "WeaponRadiationDamageMod" => "Radiation",
        "WeaponViralDamageMod" => "Viral",
        // Newer factions.
        "WeaponFactionDamageOrokin" => "Damage to Orokin",
        "WeaponFactionDamageScaldra" => "Damage to Scaldra",
        "WeaponFactionDamageTechrot" => "Damage to Techrot",
        // Newer gun stats.
        "WeaponAmmoEfficiency" => "Ammo Efficiency",
        "WeaponStatusDamage" => "Status Damage",
        "WeaponWeakpointDamage" => "Weak Point Damage",
        "WeaponWeakpointCriticalChance" => "Weak Point Critical Chance",
        "WeaponMagazineReloadHolstered" => "Magazine Reloaded/s when Holstered",
        // Newer melee stats.
        "WeaponMeleeHeavyAttackChargeMod" => "Heavy Attack Wind Up Speed",
        "WeaponMeleeHeavyAttackDamageMod" => "Melee Damage on Heavy Attack",
        "WeaponMeleeSlamDamageMod" => "Slam Attack Damage",
        "WeaponMeleeParryAngleMod" => "Parry Angle",
        other => other,
    };
    label.to_string()
}

pub fn polarity_label(pol: &str) -> &'static str {
    match pol {
        "AP_ATTACK" => "Madurai",
        "AP_DEFENSE" => "Vazarin",
        "AP_TACTIC" => "Naramon",
        "AP_POWER" => "Zenurik",
        "AP_WARD" => "Unairu",
        "AP_PRECEPT" => "Penjaga",
        "AP_UMBRA" => "Umbra",
        "AP_UNIVERSAL" => "Universal",
        "AP_ANY" => "Aura",
        _ => "",
    }
}

/// "HELLO world" → "Hello World".
pub fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

/// "AshPrimeChassis" → "Ash Prime Chassis".
pub fn split_camel(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 {
            let prev = chars[i - 1];
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            let boundary = (c.is_uppercase() && (prev.is_lowercase() || (prev.is_uppercase() && next_lower)))
                || (c.is_ascii_digit() && prev.is_alphabetic())
                || (c.is_alphabetic() && prev.is_ascii_digit());
            if boundary {
                out.push(' ');
            }
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {

    #[test]
    fn daily_caps() {
        assert_eq!(daily_cap_syndicate("Library"), Some("LibrarySyndicate"));
        assert_eq!(daily_cap_syndicate("Cavia"), Some("EntratiLabSyndicate"));
        assert_eq!(daily_cap_syndicate(""), None);
    }

    #[test]
    fn syndicate_emblems() {
        assert_eq!(syndicate_emblem("RedVeilSyndicate").as_deref(), Some("/Lotus/Upgrades/Skins/Sigils/SyndicateSigilRedVeilA"));
        assert_eq!(syndicate_emblem("ConclaveSyndicate").as_deref(), Some("/Lotus/Upgrades/Skins/Sigils/SyndicateSigilConclaveA"));
        assert_eq!(syndicate_emblem("CetusSyndicate"), None);
        assert_eq!(syndicate_emblem("LibrarySyndicate").as_deref(), Some("/Lotus/Upgrades/Skins/Sigils/CollectorSigil"));
        assert_eq!(syndicate_emblem("EventSyndicate"), None);
    }

    use super::*;

    #[test]
    fn camel_split() {
        assert_eq!(split_camel("AshPrimeChassis"), "Ash Prime Chassis");
        assert_eq!(split_camel("Booster3Day"), "Booster 3 Day");
        assert_eq!(split_camel("RJWeapon"), "RJ Weapon");
    }

    #[test]
    fn labels() {
        assert_eq!(mission_type_label("MT_TERRITORY"), "Interception");
        assert_eq!(mission_type_label("MT_NEW_THING"), "New Thing");
        assert_eq!(void_tier("VoidT4Hard"), Some("Axi"));
        assert_eq!(syndicate_label("ArbitersSyndicate"), "Arbiters of Hexis");
    }
}
