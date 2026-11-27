//! Text from `fluffyframe-core` the interface shows: enum labels. The core stays
//! English (it has no translations); listing its labels here puts them in the
//! locale files, and views show them with `tr(x.label())`.

use crate::i18n::tkey;

#[allow(dead_code)]
const CORE_TEXT: &[&str] = &[
    // ItemKind
    tkey("Warframe"), tkey("Archwing"), tkey("Necramech"), tkey("Primary"), tkey("Secondary"), tkey("Melee"),
    tkey("Arch-Gun"), tkey("Arch-Melee"), tkey("Amp"), tkey("Companion"), tkey("Companion Weapon"), tkey("K-Drive"),
    tkey("Mod"), tkey("Arcane"), tkey("Relic"), tkey("Resource"), tkey("Component"), tkey("Blueprint"), tkey("Gear"),
    tkey("Key"), tkey("Extractor"), tkey("Endo"), tkey("Other"),
    // mastery_helper::Source
    tkey("Owned: level it"), tkey("Ready to claim"), tkey("Blueprint: craft it"), tkey("Blueprint: craft the parts"),
    tkey("Blueprint from Market"), tkey("Blueprint from clan lab"),
    // rivens::Setup
    tkey("2 buffs"), tkey("2 buffs + curse"), tkey("3 buffs"), tkey("3 buffs + curse"),
    // InventorySource
    tkey("Read from the running game"), tkey("Imported inventory JSON"), tkey("Imported AlecaFrame cache"), tkey("No inventory"),
    // AttrGrade
    tkey("Great"), tkey("Good"), tkey("OK"), tkey("Bad"),
    // Rarity, Refinement
    tkey("Common"), tkey("Uncommon"), tkey("Rare"), tkey("Intact"), tkey("Exceptional"), tkey("Flawless"), tkey("Radiant"),
    // WfmStatus
    tkey("Online"), tkey("In game"), tkey("Invisible"),
    // InventoryGroup
    tkey("Everything"), tkey("All Parts"), tkey("Full Sets"), tkey("Relics"), tkey("Mods"), tkey("Arcanes"),
    tkey("Equipment"), tkey("Resources"), tkey("Misc"),
    // ShardColor
    tkey("Crimson"), tkey("Amber"), tkey("Azure"), tkey("Emerald"), tkey("Topaz"), tkey("Violet"),
    // Mission types (game_tables::mission_type_label)
    tkey("Alchemy"), tkey("Arena"), tkey("Void Armageddon"), tkey("Disruption"), tkey("Assassination"),
    tkey("Assault"), tkey("Capture"), tkey("Void Flood"), tkey("Deception"), tkey("Defense"),
    tkey("Legacyte Harvest"), tkey("Sanctuary Onslaught"), tkey("Defection"), tkey("Excavation"),
    tkey("Exterminate"), tkey("Hive"), tkey("Spy"), tkey("Solar Rail Junction"), tkey("Free Roam"),
    tkey("Mobile Defense"), tkey("Infested Salvage"), tkey("Pursuit"), tkey("Conclave"), tkey("Rush"),
    tkey("Railjack"), tkey("Rescue"), tkey("Hijack"), tkey("Sabotage"), tkey("Survival"), tkey("Interception"),
    tkey("Netracell"), tkey("Void Cascade"),
    // Syndicates (game_tables::syndicate_label)
    tkey("Arbiters of Hexis"), tkey("Cephalon Suda"), tkey("New Loka"), tkey("The Perrin Sequence"),
    tkey("Red Veil"), tkey("Steel Meridian"), tkey("Ostrons"), tkey("Solaris United"), tkey("Entrati"),
    tkey("Necraloid"), tkey("The Quills"), tkey("Ventkids"), tkey("Vox Solaris"), tkey("The Holdfasts"),
    tkey("Kahl's Garrison"), tkey("Cavia"), tkey("The Hex"), tkey("Conclave"), tkey("Cephalon Simaris"),
    tkey("Nightwave"),
    // Planets and Railjack regions (node labels' "(…)" part)
    tkey("Ceres"), tkey("Dark Refractory, Deimos"), tkey("Deimos"), tkey("Duviri"), tkey("Earth"), tkey("Eris"),
    tkey("Europa"), tkey("Höllvania"), tkey("Jupiter"), tkey("Kuva Fortress"), tkey("Lua"), tkey("Mars"),
    tkey("Mercury"), tkey("Neptune"), tkey("Phobos"), tkey("Pluto"), tkey("Saturn"), tkey("Sedna"), tkey("Uranus"),
    tkey("Venus"), tkey("Void"), tkey("Zariman"), tkey("Earth Proxima"), tkey("Venus Proxima"),
    tkey("Saturn Proxima"), tkey("Neptune Proxima"), tkey("Pluto Proxima"), tkey("Veil Proxima"),
    tkey("Uranus Proxima"),
    // Mission types (game_tables::mission_type_label)
    tkey("Alchemy"), tkey("Arena"), tkey("Void Armageddon"), tkey("Disruption"), tkey("Assassination"),
    tkey("Assault"), tkey("Capture"), tkey("Void Flood"), tkey("Deception"), tkey("Defense"),
    tkey("Legacyte Harvest"), tkey("Sanctuary Onslaught"), tkey("Defection"), tkey("Excavation"),
    tkey("Exterminate"), tkey("Hive"), tkey("Spy"), tkey("Solar Rail Junction"), tkey("Free Roam"),
    tkey("Mobile Defense"), tkey("Infested Salvage"), tkey("Pursuit"), tkey("Conclave"), tkey("Rush"),
    tkey("Railjack"), tkey("Rescue"), tkey("Hijack"), tkey("Sabotage"), tkey("Survival"), tkey("Interception"),
    tkey("Netracell"), tkey("Void Cascade"),
    // Syndicates (game_tables::syndicate_label)
    tkey("Arbiters of Hexis"), tkey("Cephalon Suda"), tkey("New Loka"), tkey("The Perrin Sequence"),
    tkey("Red Veil"), tkey("Steel Meridian"), tkey("Ostrons"), tkey("Solaris United"), tkey("Entrati"),
    tkey("Necraloid"), tkey("The Quills"), tkey("Ventkids"), tkey("Vox Solaris"), tkey("The Holdfasts"),
    tkey("Kahl's Garrison"), tkey("Cavia"), tkey("The Hex"), tkey("Conclave"), tkey("Cephalon Simaris"),
    tkey("Nightwave"),
    // Planets and Railjack regions (node labels' "(…)" part)
    tkey("Ceres"), tkey("Dark Refractory, Deimos"), tkey("Deimos"), tkey("Duviri"), tkey("Earth"), tkey("Eris"),
    tkey("Europa"), tkey("Höllvania"), tkey("Jupiter"), tkey("Kuva Fortress"), tkey("Lua"), tkey("Mars"),
    tkey("Mercury"), tkey("Neptune"), tkey("Phobos"), tkey("Pluto"), tkey("Saturn"), tkey("Sedna"), tkey("Uranus"),
    tkey("Venus"), tkey("Void"), tkey("Zariman"), tkey("Earth Proxima"), tkey("Venus Proxima"),
    tkey("Saturn Proxima"), tkey("Neptune Proxima"), tkey("Pluto Proxima"), tkey("Veil Proxima"),
    tkey("Uranus Proxima"),
];
