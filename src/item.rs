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
    if let Some(b) = it.base.strip_prefix("Superior ") {
        it.base = b.to_string();
    }

    for sec in &sections[1..] {
        let is_requirements = sec[0].starts_with("Requirements");
        // Advanced descriptions put the mod kind in a `{ … }` header above its line(s).
        let mut header: Option<ModKind> = None;
        for &line in sec {
            if line.starts_with('{') {
                header = header_kind(line);
            } else if let Some(v) = line.strip_prefix("Item Level: ") {
                it.item_level = leading_num(v);
            } else if let Some(v) = line.strip_prefix("Level: ") {
                if it.rarity == Rarity::Gem && !is_requirements && it.gem_level.is_none() {
                    it.gem_level = leading_num(v);
                }
            } else if let Some(v) = line.strip_prefix("Quality: ") {
                it.quality = leading_num(v);
            } else if let Some(v) = line.strip_prefix("Stack Size: ") {
                it.stack_size = leading_num(v.split('/').next().unwrap_or(v));
            } else if line == "Corrupted" {
                it.corrupted = true;
            } else if line == "Unidentified" {
                it.unidentified = true;
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
            } else if !line.contains(": ") && !is_requirements {
                let mut m = parse_mod(line);
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
/// Roll ranges from advanced descriptions: `13(10-15)` -> `13`.
static ROLL_RANGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d)\(-?\d+(?:\.\d+)?-\d+(?:\.\d+)?\)").unwrap());

fn parse_mod(line: &str) -> Mod {
    let mut kind = ModKind::Explicit;
    let clean = KEYWORD.replace_all(line, "$1");
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
    }

    #[test]
    fn parses_poe1_advanced_description() {
        let it = parse("Item Class: Boots\nRarity: Rare\nX\nSlink Boots\n--------\nItem Level: 82\n--------\n{ Prefix Modifier \"Virile\" (Tier: 4) — Life }\n+52(50-59) to maximum Life\n{ Master Crafted Suffix Modifier \"of Craft\" (Rank: 1) }\n+29(28-35)% to Lightning Resistance (crafted)\n").unwrap();
        let t: Vec<(&str, &str)> = it.mods.iter().map(|m| (m.kind.trade_type(), m.template.as_str())).collect();
        assert_eq!(t, vec![("explicit", "+# to maximum Life"), ("crafted", "+#% to Lightning Resistance")]);
        assert_eq!(it.mods[0].values, vec![52.0]);
    }

    #[test]
    fn strips_poe2_markup() {
        let m = parse_mod("[Allies|Allies] in your [Presence] deal 74(70-80)% increased Damage");
        assert_eq!(m.template, "Allies in your Presence deal #% increased Damage");
        assert_eq!(m.values, vec![74.0]);
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
