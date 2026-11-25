//! Framed card art for mods and arcanes (original: `wikiCardArtUrl` in
//! `services/itemDatabase.ts`, `config/shared/wikiModArt.ts`, `wikiItemArt.ts`).
//! DE's export ships only the flat mod texture; the wiki has the framed card,
//! mirrored on the WFHelper asset host.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::config::constants::ICON_MIRROR_BASE;

struct WikiArt {
    by_unique: HashMap<&'static str, &'static str>,
    by_name: HashMap<&'static str, &'static str>,
    item: HashMap<&'static str, &'static str>,
}

static ART: LazyLock<WikiArt> = LazyLock::new(|| {
    let mut art = WikiArt { by_unique: HashMap::new(), by_name: HashMap::new(), item: HashMap::new() };
    for line in include_str!("../../data/wiki_art.tsv").lines().filter(|l| !l.starts_with('#')) {
        let mut cols = line.splitn(3, '\t');
        let (Some(kind), Some(key), Some(stem)) = (cols.next(), cols.next(), cols.next()) else { continue };
        let map = match kind {
            "u" => &mut art.by_unique,
            "n" => &mut art.by_name,
            "i" => &mut art.item,
            _ => continue,
        };
        map.insert(key, stem);
    }
    art
});

/// Card art URL. Per-item overrides win; otherwise only mods and arcanes have
/// cards, joined by uniqueName and then by display name (the wiki's internal
/// names drifted from DE's for railjack mods and stances).
pub fn card_art_url(unique: &str, name: &str, mod_or_arcane: bool) -> Option<String> {
    if let Some(stem) = ART.item.get(unique) {
        return Some(format!("{ICON_MIRROR_BASE}/item-art/{stem}.webp"));
    }
    if !mod_or_arcane {
        return None;
    }
    let stem = ART.by_unique.get(unique).or_else(|| ART.by_name.get(name))?;
    Some(format!("{ICON_MIRROR_BASE}/mod-art/{}.webp", urlencoding(stem)))
}

/// Stems are wiki file names: ASCII letters, digits and the odd punctuation.
fn urlencoding(stem: &str) -> String {
    let mut out = String::with_capacity(stem.len());
    for b in stem.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_cards() {
        let url = card_art_url("/Lotus/Powersuits/Banshee/PushAugmentCard", "Sonic Fracture", true).unwrap();
        assert_eq!(url, format!("{ICON_MIRROR_BASE}/mod-art/SonicFractureMod.webp"));
        assert!(card_art_url("/Lotus/Powersuits/Banshee/PushAugmentCard", "Sonic Fracture", false).is_none());
        let shard = card_art_url("/Lotus/Types/Gameplay/NarmerSorties/ArchonCrystalAmarMythic", "", false).unwrap();
        assert!(shard.ends_with("/item-art/TauforgedCrimsonArchonShard.webp"));
        assert!(card_art_url("/Lotus/Nope", "Nope", true).is_none());
    }
}
