//! Parser for the text PoE 1 / PoE 2 put on the clipboard when you press Ctrl+C on an item.

use regex::Regex;
use std::sync::LazyLock;

static NUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+(?:\.\d+)?").unwrap());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Rarity {
    #[default]
    Normal,
    Magic,
    Rare,
    Unique,
    Currency,
    Gem,
    DivinationCard,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModKind {
    Explicit,
    Implicit,
    Crafted,
    Enchant,
    Fractured,
    Rune,
    Desecrated,
}

impl ModKind {
    /// Stat group id used by the trade API's `/data/stats`.
    pub fn trade_type(self) -> &'static str {
        match self {
            ModKind::Explicit => "explicit",
            ModKind::Implicit => "implicit",
            ModKind::Crafted => "crafted",
            ModKind::Enchant => "enchant",
            ModKind::Fractured => "fractured",
            ModKind::Rune => "rune",
            ModKind::Desecrated => "desecrated",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Mod {
    pub kind: ModKind,
    pub text: String,
    /// Text with numbers replaced by `#`, the format the trade API uses.
    pub template: String,
    pub values: Vec<f64>,
    /// Advanced descriptions (Ctrl+Alt+C) only: the mod's tier (1 is the best, in both games)
    /// and the possible range of each rolled value, e.g. `+52(50-59)` -> `(50, 59)`.
    pub tier: Option<u32>,
    pub ranges: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, Default)]
pub struct Item {
    pub class: String,
    pub rarity: Rarity,
    pub name: Option<String>,
    pub base: String,
    pub item_level: Option<u32>,
    pub gem_level: Option<u32>,
    pub quality: Option<u32>,
    pub stack_size: Option<u32>,
    pub corrupted: bool,
    pub unidentified: bool,
    /// Item shows local defence/weapon properties, so local stat variants should be preferred.
    pub has_local_props: bool,
    /// Defence properties as trade filter keys: ("ar"|"ev"|"es"|"ward"|"block", value).
    pub props: Vec<(&'static str, f64)>,
    pub weapon: Option<Weapon>,
    pub mods: Vec<Mod>,
    /// "Sockets:" line. PoE 1: "R-G-B B" (groups of linked sockets); PoE 2: "S S" (rune sockets).
    pub sockets: Option<String>,
    /// PoE 1 influences as trade names: "shaper", "elder", "crusader", "hunter", "redeemer", "warlord".
    pub influences: Vec<&'static str>,
    /// Special item kinds as trade `misc_filters` keys: "fractured_item", "synthesised_item",
    /// "searing_item", "tangled_item", "veiled".
    pub flags: Vec<&'static str>,
    /// PoE 1 maps and PoE 2 waystones: tier and the area's bonuses.
    pub map: Option<MapInfo>,
}

/// Map/waystone header lines; the bonuses are percentages ("Item Quantity: +87%").
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MapInfo {
    pub tier: Option<u32>,
    pub quantity: Option<u32>,
    pub rarity: Option<u32>,
    pub pack_size: Option<u32>,
}

/// Weapon damage lines, as average damage per hit.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Weapon {
    pub phys: f64,
    pub ele: f64,
    pub chaos: f64,
    pub aps: f64,
    pub crit: f64,
}

static RANGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+)-(\d+)").unwrap());

/// Sum of the averages of every `a-b` range on the line.
fn avg_ranges(s: &str) -> f64 {
    RANGE
        .captures_iter(s)
        .map(|c| (c[1].parse::<f64>().unwrap_or(0.0) + c[2].parse::<f64>().unwrap_or(0.0)) / 2.0)
        .sum()
}

fn first_float(s: &str) -> f64 {
    NUM.find(s).and_then(|m| m.as_str().parse().ok()).unwrap_or(0.0)
}

impl Item {
    /// Number of sockets (any colour, including rune sockets).
    pub fn socket_count(&self) -> u32 {
        self.sockets.as_deref().map_or(0, |s| s.chars().filter(|c| c.is_ascii_alphabetic()).count() as u32)
    }

    /// Size of the largest group of linked sockets (PoE 1).
    pub fn max_links(&self) -> u32 {
        self.sockets.as_deref().map_or(0, |s| {
            s.split_whitespace()
                .map(|g| g.split('-').filter(|x| !x.is_empty()).count() as u32)
                .max()
                .unwrap_or(0)
        })
    }

    /// A PoE 1 map: what the map check works on. (PoE 2 waystones parse too, but aren't checked.)
    pub fn is_map(&self) -> bool {
        self.class == "Maps"
    }

    pub fn display_name(&self) -> String {
        match &self.name {
            Some(n) => format!("{n} — {}", self.base),
            None => self.base.clone(),
        }
    }
}

const DEFENCES: &[(&str, &str)] = &[
    ("Armour: ", "ar"), ("Evasion Rating: ", "ev"), ("Evasion: ", "ev"), ("Energy Shield: ", "es"),
    ("Ward: ", "ward"), ("Chance to Block: ", "block"), ("Block chance: ", "block"),
];

const LOCAL_PROPS: &[&str] = &[
    "Armour:", "Evasion Rating:", "Evasion:", "Energy Shield:", "Ward:", "Physical Damage:",
    "Elemental Damage:", "Chaos Damage:", "Attacks per Second:", "Critical Strike Chance:", "Critical Hit Chance:",
    "Block chance:", "Chance to Block:", "Spirit:",
];

/// Lines PoE 1 adds at the end of influenced items.
const INFLUENCES: &[(&str, &str)] = &[
    ("Shaper Item", "shaper"), ("Elder Item", "elder"), ("Crusader Item", "crusader"),
    ("Hunter Item", "hunter"), ("Redeemer Item", "redeemer"), ("Warlord Item", "warlord"),
];

/// Lines that mark special items, with their trade `misc_filters` key.
const FLAG_LINES: &[(&str, &str)] = &[
    ("Fractured Item", "fractured_item"), ("Synthesised Item", "synthesised_item"),
    ("Searing Exarch Item", "searing_item"), ("Eater of Worlds Item", "tangled_item"),
];

const SUFFIXES: &[(&str, ModKind)] = &[
    (" (implicit)", ModKind::Implicit),
    (" (crafted)", ModKind::Crafted),
    (" (enchant)", ModKind::Enchant),
    (" (fractured)", ModKind::Fractured),
    (" (rune)", ModKind::Rune),
    (" (desecrated)", ModKind::Desecrated),
];

fn leading_num(s: &str) -> Option<u32> {
    let s: String = s.chars().filter(|c| *c != ',' && *c != '+').collect();
    NUM.find(&s)?.as_str().split('.').next()?.parse().ok()
}

pub fn parse(text: &str) -> Option<Item> {
    let text = text.replace("\r\n", "\n");
    let sections: Vec<Vec<&str>> = text
        .split("--------")
        .map(|s| s.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>())
        .filter(|s| !s.is_empty())
        .collect();
    let head = sections.first()?;

    let mut it = Item::default();
    let mut i = 0;
    if let Some(c) = head[0].strip_prefix("Item Class: ") {
        it.class = c.to_string();
        i = 1;
    }
    it.rarity = match head.get(i)?.strip_prefix("Rarity: ")? {
        "Normal" => Rarity::Normal,
        "Magic" => Rarity::Magic,
        "Rare" => Rarity::Rare,
        "Unique" => Rarity::Unique,
        "Currency" => Rarity::Currency,
        "Gem" => Rarity::Gem,
        "Divination Card" => Rarity::DivinationCard,
        _ => Rarity::Other,
    };
    let names = &head[i + 1..];
    match names {
        [] => return None,
        [base] => it.base = base.to_string(),
        [name, base, ..] => {
            it.name = Some(name.to_string());
            it.base = base.to_string();
        }
    }
    for prefix in ["Superior ", "Synthesised "] {
        if let Some(b) = it.base.strip_prefix(prefix) {
            it.base = b.to_string();
        }
    }

    let is_map = matches!(it.class.as_str(), "Maps" | "Waystones");
    for sec in &sections[1..] {
        let is_requirements = sec[0].starts_with("Requirements");
        // Advanced descriptions put the mod kind and tier in a `{ … }` header above its line(s);
        // a hybrid mod has several lines under one header.
        let mut header: Option<ModKind> = None;
        let mut header_tier: Option<u32> = None;
        for &line in sec {
            if line.starts_with('{') {
                header = header_kind(line);
                header_tier = TIER.captures(line).and_then(|c| c[1].parse().ok());
            } else if let Some(v) = line.strip_prefix("Item Level: ") {
                it.item_level = leading_num(v);
            } else if let Some(v) = line.strip_prefix("Level: ") {
                if it.rarity == Rarity::Gem && !is_requirements && it.gem_level.is_none() {
                    it.gem_level = leading_num(v);
                }
            } else if let Some(v) = line.strip_prefix("Map Tier: ").or(line.strip_prefix("Waystone Tier: ")) {
                it.map.get_or_insert_with(MapInfo::default).tier = leading_num(v);
            } else if let Some(v) = line.strip_prefix("Item Quantity: ") {
                it.map.get_or_insert_with(MapInfo::default).quantity = leading_num(v);
            } else if let Some(v) = line.strip_prefix("Item Rarity: ") {
                it.map.get_or_insert_with(MapInfo::default).rarity = leading_num(v);
            } else if let Some(v) = line.strip_prefix("Monster Pack Size: ") {
                it.map.get_or_insert_with(MapInfo::default).pack_size = leading_num(v);
            } else if is_map && line.ends_with('.') {
                // "Travel to this Map by using it in a personal Map Device. …": usage text, not a mod.
            } else if let Some(v) = line.strip_prefix("Quality: ") {
                it.quality = leading_num(v);
            } else if let Some(v) = line.strip_prefix("Stack Size: ") {
                it.stack_size = leading_num(v.split('/').next().unwrap_or(v));
            } else if line == "Corrupted" {
                it.corrupted = true;
            } else if line == "Unidentified" {
                it.unidentified = true;
            } else if let Some(v) = line.strip_prefix("Sockets: ") {
                it.sockets = Some(v.trim().to_string());
            } else if let Some((_, inf)) = INFLUENCES.iter().find(|(l, _)| *l == line) {
                it.influences.push(inf);
            } else if let Some((_, flag)) = FLAG_LINES.iter().find(|(l, _)| *l == line) {
                if !it.flags.contains(flag) {
                    it.flags.push(flag);
                }
            } else if line == "Veiled Prefix" || line == "Veiled Suffix" {
                // An unrevealed veiled mod: no stat to search, but the trade site can filter on it.
                if !it.flags.contains(&"veiled") {
                    it.flags.push("veiled");
                }
            } else if LOCAL_PROPS.iter().any(|p| line.starts_with(p)) {
                it.has_local_props = true;
                let w = it.weapon.get_or_insert_with(Weapon::default);
                if let Some(v) = line.strip_prefix("Physical Damage: ") {
                    w.phys = avg_ranges(v);
                } else if let Some(v) = line.strip_prefix("Elemental Damage: ") {
                    w.ele = avg_ranges(v);
                } else if let Some(v) = line.strip_prefix("Chaos Damage: ") {
                    w.chaos = avg_ranges(v);
                } else if let Some(v) = line.strip_prefix("Attacks per Second: ") {
                    w.aps = first_float(v);
                } else if let Some(v) = line.strip_prefix("Critical Strike Chance: ").or(line.strip_prefix("Critical Hit Chance: ")) {
                    w.crit = first_float(v);
                }
                if let Some((p, key)) = DEFENCES.iter().find(|(p, _)| line.starts_with(p)) {
                    if let Some(v) = leading_num(&line[p.len()..]) {
                        it.props.push((key, v as f64));
                    }
                }
            } else if (!line.contains(": ") || line.starts_with("Added Small Passive Skills grant: ")) && !is_requirements {
                let mut m = parse_mod(line);
                m.tier = header_tier;
                if let (ModKind::Explicit, Some(k)) = (m.kind, header) {
                    m.kind = k;
                }
                it.mods.push(m);
            }
        }
    }
    Some(it)
}

fn header_kind(header: &str) -> Option<ModKind> {
    let h = header.to_lowercase();
    if h.contains("desecrated") {
        Some(ModKind::Desecrated)
    } else if h.contains("fractured") {
        Some(ModKind::Fractured)
    } else if h.contains("crafted") {
        Some(ModKind::Crafted)
    } else {
        None
    }
}

/// `[Presence|Presence]` / `[Presence]` keyword markup -> plain word.
static KEYWORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[(?:[^\]|]*\|)?([^\]]*)\]").unwrap());
/// Roll ranges from advanced descriptions: `13(10-15)` -> `13`, keeping `10` and `15`.
static ROLL_RANGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d)\((-?\d+(?:\.\d+)?)-(\d+(?:\.\d+)?)\)").unwrap());
/// `(Tier: 4)` in an advanced description's mod header. Crafted mods have a `(Rank: N)` instead.
static TIER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\(Tier: (\d+)\)").unwrap());

fn parse_mod(line: &str) -> Mod {
    let mut kind = ModKind::Explicit;
    let clean = KEYWORD.replace_all(line, "$1");
    let ranges = ROLL_RANGE
        .captures_iter(&clean)
        .filter_map(|c| Some((c[2].parse().ok()?, c[3].parse().ok()?)))
        .collect();
    let clean = ROLL_RANGE.replace_all(&clean, "$1").into_owned();
    let line = clean.as_str();
    let mut text = line;
    for (suf, k) in SUFFIXES {
        if let Some(t) = line.strip_suffix(suf) {
            kind = *k;
            text = t;
            break;
        }
    }
    Mod {
        kind,
        text: text.to_string(),

        template: NUM.replace_all(text, "#").into_owned(),
        values: NUM.find_iter(text).filter_map(|m| m.as_str().parse().ok()).collect(),
        tier: None,
        ranges,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RING: &str = "Item Class: Rings\nRarity: Rare\nDusk Loop\nRuby Ring\n--------\nRequirements:\nLevel: 40\n--------\nItem Level: 75\n--------\n+29% to Fire Resistance (implicit)\n--------\n+52 to maximum Life\nAdds 3 to 7 Fire Damage to Attacks\n--------\nCorrupted\n";

    #[test]
    fn parses_rare() {
        let it = parse(RING).unwrap();
        assert_eq!(it.rarity, Rarity::Rare);
        assert_eq!(it.name.as_deref(), Some("Dusk Loop"));
        assert_eq!(it.base, "Ruby Ring");
        assert_eq!(it.item_level, Some(75));
        assert!(it.corrupted);
        let boots = parse("Item Class: Boots\nRarity: Rare\nX\nY Boots\n--------\nArmour: 67 (augmented)\nEvasion Rating: 60\n--------\n").unwrap();
        assert_eq!(boots.props, vec![("ar", 67.0), ("ev", 60.0)]);
        let axe = parse("Item Class: Two Hand Axes\nRarity: Rare\nX\nY Axe\n--------\nPhysical Damage: 10-20 (augmented)\nElemental Damage: 5-15 (augmented), 1-3 (augmented)\nAttacks per Second: 1.50\n--------\n").unwrap();
        let w = axe.weapon.unwrap();
        assert_eq!((w.phys, w.ele, w.aps), (15.0, 12.0, 1.5));
        assert_eq!(it.mods.len(), 3);
        assert_eq!(it.mods[0].kind, ModKind::Implicit);
        assert_eq!(it.mods[0].template, "+#% to Fire Resistance");
        assert_eq!(it.mods[2].template, "Adds # to # Fire Damage to Attacks");
        assert_eq!(it.mods[2].values, vec![3.0, 7.0]);
    }

    #[test]
    fn parses_poe2_advanced_description() {
        let it = parse(include_str!("../dev/sceptre-poe2.txt")).unwrap();
        assert!(it.has_local_props, "Spirit: makes increased Spirit local");
        let t: Vec<(&str, &str)> = it.mods.iter().map(|m| (m.kind.trade_type(), m.template.as_str())).collect();
        assert_eq!(t, vec![
            ("rune", "Minions in your Presence have Onslaught while you are on Low Runic Ward"),
            ("explicit", "Allies in your Presence deal #% increased Damage"),
            ("explicit", "+# to maximum Mana"),
            ("desecrated", "#% increased Spirit"),
            ("explicit", "#% increased Presence Area of Effect"),
            ("explicit", "Allies in your Presence have #% increased Attack Speed"),
            ("explicit", "+# to Level of all Minion Skills"),
        ]);
        assert_eq!(it.mods[1].values, vec![74.0]);
        assert_eq!((it.mods[0].tier, it.mods[0].ranges.len()), (None, 0), "rune mods have no header");
        assert_eq!((it.mods[1].tier, &it.mods[1].ranges), (Some(4), &vec![(65.0, 74.0)]));
        assert_eq!((it.mods[2].tier, &it.mods[2].ranges), (Some(11), &vec![(10.0, 14.0)]));
        assert_eq!((it.mods[6].tier, it.mods[6].ranges.len()), (Some(1), 0), "fixed value, no range");
    }

    #[test]
    fn parses_poe1_advanced_description() {
        let it = parse("Item Class: Boots\nRarity: Rare\nX\nSlink Boots\n--------\nItem Level: 82\n--------\n{ Prefix Modifier \"Virile\" (Tier: 4) — Life }\n+52(50-59) to maximum Life\n{ Master Crafted Suffix Modifier \"of Craft\" (Rank: 1) }\n+29(28-35)% to Lightning Resistance (crafted)\n").unwrap();
        let t: Vec<(&str, &str)> = it.mods.iter().map(|m| (m.kind.trade_type(), m.template.as_str())).collect();
        assert_eq!(t, vec![("explicit", "+# to maximum Life"), ("crafted", "+#% to Lightning Resistance")]);
        assert_eq!(it.mods[0].values, vec![52.0]);
        assert_eq!((it.mods[0].tier, &it.mods[0].ranges), (Some(4), &vec![(50.0, 59.0)]));
        assert_eq!(it.mods[1].tier, None, "crafted mods have a rank, not a tier");
        // Plain Ctrl+C text has neither.
        let plain = parse(RING).unwrap();
        assert!(plain.mods.iter().all(|m| m.tier.is_none() && m.ranges.is_empty()));
    }

    #[test]
    fn hybrid_and_two_value_mods_keep_tier_and_ranges() {
        let it = parse("Item Class: Body Armours\nRarity: Rare\nX\nY\n--------\nItem Level: 84\n--------\n{ Prefix Modifier \"Rhino's\" (Tier: 1) — Defences, Life }\n+28(24-28) to Armour\n+27(24-28) to maximum Life\n{ Suffix Modifier \"of Ire\" (Tier: 3) — Damage }\nAdds 6(5-7) to 12(11-13) Fire Damage to Attacks\n").unwrap();
        assert_eq!(it.mods.iter().map(|m| m.tier).collect::<Vec<_>>(), vec![Some(1), Some(1), Some(3)]);
        assert_eq!(it.mods[2].ranges, vec![(5.0, 7.0), (11.0, 13.0)]);
        assert_eq!(it.mods[2].values, vec![6.0, 12.0]);
    }

    #[test]
    fn parses_maps_and_waystones() {
        let map = parse(include_str!("../dev/map.txt")).unwrap();
        assert!(map.is_map());
        assert_eq!(map.map, Some(MapInfo { tier: Some(16), quantity: Some(87), rarity: Some(45), pack_size: Some(28) }));
        assert_eq!(map.quality, Some(20));
        assert_eq!(map.mods.len(), 6, "usage text isn't a mod: {:?}", map.mods);
        assert_eq!(map.mods[1].template, "Monsters reflect #% of Elemental Damage");

        let way = parse(include_str!("../dev/waystone-poe2.txt")).unwrap();
        assert!(!way.is_map(), "the map check is PoE 1 only");
        assert_eq!(way.map, Some(MapInfo { tier: Some(15), quantity: Some(60), rarity: Some(38), pack_size: Some(15) }));
        assert_eq!(way.mods.len(), 5, "{:?}", way.mods);
        assert_eq!(way.mods[1].template, "Players are Cursed with Enfeeble");

        assert!(!parse(RING).unwrap().is_map());
    }

    #[test]
    fn strips_poe2_markup() {
        let m = parse_mod("[Allies|Allies] in your [Presence] deal 74(70-80)% increased Damage");
        assert_eq!(m.template, "Allies in your Presence deal #% increased Damage");
        assert_eq!(m.values, vec![74.0]);
    }

    #[test]
    fn parses_weapon_numbers() {
        let it = parse(include_str!("../dev/axe.txt")).unwrap();
        let w = it.weapon.expect("weapon stats");
        assert_eq!((w.phys, w.ele, w.aps, w.crit), (160.0, 30.0, 1.32, 5.0));
        assert_eq!(it.quality, Some(8));
        assert_eq!(it.item_level, Some(81));
    }

    #[test]
    fn parses_every_dev_sample() {
        for (name, text) in [
            ("axe", include_str!("../dev/axe.txt")),
            ("boots", include_str!("../dev/boots.txt")),
            ("boots-poe2", include_str!("../dev/boots-poe2.txt")),
            ("sceptre-poe2", include_str!("../dev/sceptre-poe2.txt")),
            ("divine", include_str!("../dev/divine.txt")),
            ("unique", include_str!("../dev/unique.txt")),
            ("sixlink", include_str!("../dev/sixlink.txt")),
            ("influenced", include_str!("../dev/influenced.txt")),
            ("cluster", include_str!("../dev/cluster.txt")),
            ("runes-poe2", include_str!("../dev/runes-poe2.txt")),
            ("chaos-poe2", include_str!("../dev/chaos-poe2.txt")),
            ("ring", include_str!("../dev/ring.txt")),
            ("ring-poe2", include_str!("../dev/ring-poe2.txt")),
            ("map", include_str!("../dev/map.txt")),
            ("waystone-poe2", include_str!("../dev/waystone-poe2.txt")),
        ] {
            let it = parse(text).unwrap_or_else(|| panic!("{name} doesn't parse"));
            assert!(!it.base.is_empty(), "{name} has no base");
        }
        let boots2 = parse(include_str!("../dev/boots-poe2.txt")).unwrap();
        assert_eq!(boots2.base, "Dragonscale Boots");
        assert_eq!(boots2.mods.len(), 3);
        assert_eq!(parse(include_str!("../dev/chaos-poe2.txt")).unwrap().rarity, Rarity::Currency);
    }

    #[test]
    fn parses_sockets_and_links() {
        let six = parse(include_str!("../dev/sixlink.txt")).unwrap();
        assert_eq!((six.socket_count(), six.max_links()), (6, 6));
        let split = parse("Item Class: Gloves\nRarity: Rare\nX\nY Gloves\n--------\nSockets: R-G B-B \n").unwrap();
        assert_eq!((split.socket_count(), split.max_links()), (4, 2));
        let runes = parse(include_str!("../dev/runes-poe2.txt")).unwrap();
        assert_eq!(runes.socket_count(), 2);
        assert_eq!(runes.mods.iter().filter(|m| m.kind == ModKind::Rune).count(), 2, "socketed runes are mods");
        assert_eq!(parse(include_str!("../dev/boots.txt")).unwrap().sockets, None);
    }

    #[test]
    fn parses_influences_and_special_items() {
        let ring = parse(include_str!("../dev/influenced.txt")).unwrap();
        assert_eq!(ring.influences, vec!["hunter", "redeemer"]);
        assert_eq!(ring.flags, vec!["veiled", "fractured_item"]);
        assert!(ring.mods.iter().all(|m| !m.text.starts_with("Veiled") && !m.text.ends_with(" Item")));
        assert_eq!(ring.mods.iter().filter(|m| m.kind == ModKind::Fractured).count(), 1);
        let synth = parse("Item Class: Rings\nRarity: Rare\nX\nSynthesised Opal Ring\n--------\nSynthesised Item\nSearing Exarch Item\nEater of Worlds Item\n").unwrap();
        assert_eq!(synth.base, "Opal Ring");
        assert_eq!(synth.flags, vec!["synthesised_item", "searing_item", "tangled_item"]);
    }

    #[test]
    fn rejects_non_item_text() {
        assert!(parse("hello world").is_none());
        assert!(parse("").is_none());
    }

    #[test]
    fn parses_gem_and_currency() {
        let gem = parse("Item Class: Skill Gems\nRarity: Gem\nVaal Grace\n--------\nLevel: 20 (Max)\nQuality: +20% (augmented)\n--------\nRequirements:\nLevel: 70\n").unwrap();
        assert_eq!(gem.gem_level, Some(20));
        assert_eq!(gem.quality, Some(20));
        let cur = parse("Item Class: Stackable Currency\nRarity: Currency\nDivine Orb\n--------\nStack Size: 1,234/20\n").unwrap();
        assert_eq!(cur.base, "Divine Orb");
        assert_eq!(cur.stack_size, Some(1234));
    }
}
