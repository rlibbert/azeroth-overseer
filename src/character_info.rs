//! Static WotLK 3.3.5a playable race/class ID -> name lookup. Same table
//! already verified live against real character data in wow-gm-console
//! (e.g. race=8/class=4 correctly matches the known Troll Rogue "Bazzul").

pub fn race_name(race: u8) -> &'static str {
    match race {
        1 => "Human",
        2 => "Orc",
        3 => "Dwarf",
        4 => "Night Elf",
        5 => "Undead",
        6 => "Tauren",
        7 => "Gnome",
        8 => "Troll",
        10 => "Blood Elf",
        11 => "Draenei",
        _ => "Unknown",
    }
}

pub fn class_name(class: u8) -> &'static str {
    match class {
        1 => "Warrior",
        2 => "Paladin",
        3 => "Hunter",
        4 => "Rogue",
        5 => "Priest",
        6 => "Death Knight",
        7 => "Shaman",
        8 => "Mage",
        9 => "Warlock",
        11 => "Druid",
        _ => "Unknown",
    }
}
