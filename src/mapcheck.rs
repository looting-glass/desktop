//! Map check (PoE 1 maps): a map's mods, coloured by how the player marked them (deadly /
//! annoying / neutral). Marks are mod templates stored in the config's `[map_check]`.

use crate::config::MapCheckConfig;
use crate::item::Item;
use serde::Serialize;

pub const DEADLY: &str = "deadly";
pub const ANNOYING: &str = "annoying";

#[derive(Debug, Serialize, PartialEq)]
pub struct MapMod {
    pub text: String,
    /// What a mark is stored as: the text with numbers replaced by `#`.
    pub template: String,
    /// "deadly", "annoying" or "".
    pub level: &'static str,
}

/// What the overlay shows for a map check.
#[derive(Debug, Serialize, PartialEq)]
pub struct MapView {
    /// Item class for the overlay header ("Maps").
    pub class: String,
    pub tier: Option<u32>,
    /// Area bonuses as ("Atlas Drops", 46).
    pub bonuses: Vec<(String, u32)>,
    pub corrupted: bool,
    pub mods: Vec<MapMod>,
    pub deadly: usize,
    pub annoying: usize,
}

pub fn level_of(template: &str, marks: &MapCheckConfig) -> &'static str {
    if marks.deadly.iter().any(|t| t == template) {
        DEADLY
    } else if marks.annoying.iter().any(|t| t == template) {
        ANNOYING
    } else {
        ""
    }
}

pub fn view(item: &Item, marks: &MapCheckConfig) -> MapView {
    let info = item.map.clone().unwrap_or_default();
    let mods: Vec<MapMod> = item
        .mods
        .iter()
        .map(|m| MapMod { text: m.text.clone(), template: m.template.clone(), level: level_of(&m.template, marks) })
        .collect();
    MapView {
        class: item.class.clone(),
        tier: info.tier,
        bonuses: info.bonuses,
        corrupted: item.corrupted,
        deadly: mods.iter().filter(|m| m.level == DEADLY).count(),
        annoying: mods.iter().filter(|m| m.level == ANNOYING).count(),
        mods,
    }
}

/// Set a mod's mark: "deadly", "annoying", or anything else to clear it.
pub fn mark(marks: &mut MapCheckConfig, template: &str, level: &str) {
    marks.deadly.retain(|t| t != template);
    marks.annoying.retain(|t| t != template);
    match level {
        DEADLY => marks.deadly.push(template.to_string()),
        ANNOYING => marks.annoying.push(template.to_string()),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::parse;

    #[test]
    fn marks_colour_every_roll_of_a_mod() {
        let map = parse(include_str!("../dev/map.txt")).unwrap();
        let mut marks = MapCheckConfig::default();
        mark(&mut marks, "#% more Monster Life", DEADLY);
        mark(&mut marks, "Area contains a Mirage", ANNOYING);
        let v = view(&map, &marks);
        assert_eq!((v.tier, v.bonuses.len()), (Some(12), 3));
        let levels: Vec<&str> = v.mods.iter().map(|m| m.level).collect();
        assert_eq!(levels, vec!["", DEADLY, ANNOYING, ""]);
        assert_eq!((v.deadly, v.annoying), (1, 1));
        assert_eq!(v.mods[1].text, "44% more Monster Life");
    }

    #[test]
    fn re_marking_moves_and_clears() {
        let t = "Players are Cursed with Enfeeble";
        let mut marks = MapCheckConfig::default();
        mark(&mut marks, t, ANNOYING);
        mark(&mut marks, t, DEADLY);
        assert_eq!((marks.deadly.len(), marks.annoying.len()), (1, 0));
        mark(&mut marks, t, "");
        assert_eq!(marks, MapCheckConfig::default());
    }
}
