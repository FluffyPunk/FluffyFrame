//! Embedded images (original: `src/lib/assetUrls.ts`). All from the WFHelper repo (MIT).

use egui::{ImageSource, include_image};

use crate::views::ViewName;

pub const LOGO: ImageSource<'static> = include_image!("../assets/icons/logo.png");
pub const PLATINUM: ImageSource<'static> = include_image!("../assets/icons/Platinum.png");
pub const DUCATS: ImageSource<'static> = include_image!("../assets/icons/OrokinDucats.png");
pub const DUCATS_SMALL: ImageSource<'static> = include_image!("../assets/icons/ducats.png");
pub const AYA: ImageSource<'static> = include_image!("../assets/icons/aya.webp");
pub const VITUS: ImageSource<'static> = include_image!("../assets/icons/vitus.png");
pub const RIVEN: ImageSource<'static> = include_image!("../assets/icons/riven.png");
pub const TRADE: ImageSource<'static> = include_image!("../assets/icons/trade.png");
pub const RELIC: ImageSource<'static> = include_image!("../assets/icons/IconRelic256.png");
pub const MASTERY: ImageSource<'static> = include_image!("../assets/icons/Mastery.png");
pub const ENDO: ImageSource<'static> = include_image!("../assets/icons/endo.png");
/// Blank riven card the Rivens view draws stats onto (316×400).
pub const RIVEN_TEMPLATE: ImageSource<'static> = include_image!("../assets/riven/RivenTemplate.png");

/// Damage-type icon for a riven stat tag (original: `ELEMENT_ICON_URLS`).
pub fn element_icon(tag: &str) -> Option<ImageSource<'static>> {
    Some(match tag {
        "WeaponFireDamageMod" => include_image!("../assets/elements/Heat.png"),
        "WeaponFreezeDamageMod" => include_image!("../assets/elements/Cold.png"),
        "WeaponElectricityDamageMod" => include_image!("../assets/elements/Electricity.png"),
        "WeaponToxinDamageMod" => include_image!("../assets/elements/Toxin.png"),
        "WeaponImpactDamageMod" => include_image!("../assets/elements/Impact.png"),
        "WeaponArmorPiercingDamageMod" => include_image!("../assets/elements/Puncture.png"),
        "WeaponSlashDamageMod" => include_image!("../assets/elements/Slash.png"),
        // Combined elements (wiki.warframe.com damage icons, CC BY-SA).
        "WeaponBlastDamageMod" => include_image!("../assets/elements/Blast.png"),
        "WeaponCorrosiveDamageMod" => include_image!("../assets/elements/Corrosive.png"),
        "WeaponGasDamageMod" => include_image!("../assets/elements/Gas.png"),
        "WeaponMagneticDamageMod" => include_image!("../assets/elements/Magnetic.png"),
        "WeaponRadiationDamageMod" => include_image!("../assets/elements/Radiation.png"),
        "WeaponViralDamageMod" => include_image!("../assets/elements/Viral.png"),
        _ => return None,
    })
}

pub fn view_icon(view: ViewName) -> ImageSource<'static> {
    match view {
        ViewName::Dashboard | ViewName::Setup => include_image!("../assets/icons/Dashboard.svg"),
        ViewName::Inventory => include_image!("../assets/icons/IconWarframe_256.png"),
        ViewName::Foundry => include_image!("../assets/icons/Foundry.png"),
        ViewName::Mastery => include_image!("../assets/icons/Mastery_bw2.png"),
        ViewName::Stats => include_image!("../assets/icons/Stats.png"),
        ViewName::World => include_image!("../assets/icons/Navigation.png"),
        ViewName::Syndicates => include_image!("../assets/icons/Syndicates.svg"),
        ViewName::Market => include_image!("../assets/icons/Market.png"),
        ViewName::Relics => include_image!("../assets/icons/IconRelic256.png"),
        ViewName::Wiki => include_image!("../assets/icons/Wiki.svg"),
        ViewName::Rivens => include_image!("../assets/icons/Rivens.png"),
        ViewName::Settings => include_image!("../assets/icons/Settings.png"),
    }
}

/// Planet art for open-world cycles.
pub fn cycle_icon(name: &str) -> ImageSource<'static> {
    match name {
        "Orb Vallis" => include_image!("../assets/world/vallis.webp"),
        "Cambion Drift" => include_image!("../assets/world/cambion.webp"),
        "Zariman" | "Duviri" => include_image!("../assets/world/zariman.webp"),
        _ => include_image!("../assets/world/earth.webp"),
    }
}

pub fn relic_icon(era: &str) -> ImageSource<'static> {
    match era {
        "Meso" => include_image!("../assets/world/relic-meso.png"),
        "Neo" => include_image!("../assets/world/relic-neo.png"),
        "Axi" => include_image!("../assets/world/relic-axi.png"),
        "Requiem" | "Omnia" => include_image!("../assets/world/relic-requiem.png"),
        _ => include_image!("../assets/world/relic-lith.png"),
    }
}

pub fn polarity_icon(pol: &str) -> Option<ImageSource<'static>> {
    Some(match pol {
        "AP_ATTACK" => include_image!("../assets/polarities/madurai.png"),
        "AP_DEFENSE" => include_image!("../assets/polarities/vazarin.png"),
        "AP_TACTIC" => include_image!("../assets/polarities/naramon.png"),
        "AP_POWER" => include_image!("../assets/polarities/zenurik.png"),
        "AP_WARD" => include_image!("../assets/polarities/unairu.png"),
        "AP_PRECEPT" => include_image!("../assets/polarities/penjaga.png"),
        "AP_UMBRA" => include_image!("../assets/polarities/umbra.png"),
        "AP_ANY" => include_image!("../assets/polarities/aura.png"),
        _ => return None,
    })
}
