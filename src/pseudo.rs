//! Pseudo stats: the trade site's totals over related mods ("+#% total Elemental Resistance").
//! Searching on a total finds items with the same value split differently across mods.

use crate::config::Game;
use crate::item::Mod;

/// A trade-site pseudo stat and the item mods that add to it.
pub struct Pseudo {
    /// Stat id in the trade API's "pseudo" group.
    pub id: &'static str,
    /// Stat text as the trade API lists it; `#` is the total.
    pub text: &'static str,
    /// (mod template without a leading "+", how much one point of the mod adds)
    parts: &'static [(&'static str, f64)],
    /// Worth showing even when one mod alone makes up the total.
    always: bool,
}

const FIRE: (&str, f64) = ("#% to Fire Resistance", 1.0);
const COLD: (&str, f64) = ("#% to Cold Resistance", 1.0);
const LIGHTNING: (&str, f64) = ("#% to Lightning Resistance", 1.0);
const CHAOS: (&str, f64) = ("#% to Chaos Resistance", 1.0);
const ELEMENTAL: &[(&str, f64)] = &[
    FIRE, COLD, LIGHTNING,
    ("#% to Fire and Cold Resistances", 2.0),
    ("#% to Fire and Lightning Resistances", 2.0),
    ("#% to Cold and Lightning Resistances", 2.0),
    ("#% to all Elemental Resistances", 3.0),
    ("#% to Fire and Chaos Resistances", 1.0),
    ("#% to Cold and Chaos Resistances", 1.0),
    ("#% to Lightning and Chaos Resistances", 1.0),
];
const ALL_RES: &[(&str, f64)] = &[
    FIRE, COLD, LIGHTNING, CHAOS,
    ("#% to Fire and Cold Resistances", 2.0),
    ("#% to Fire and Lightning Resistances", 2.0),
    ("#% to Cold and Lightning Resistances", 2.0),
    ("#% to all Elemental Resistances", 3.0),
    ("#% to Fire and Chaos Resistances", 2.0),
    ("#% to Cold and Chaos Resistances", 2.0),
    ("#% to Lightning and Chaos Resistances", 2.0),
];
const STR: &[(&str, f64)] = &[
    ("# to Strength", 1.0), ("# to Strength and Dexterity", 1.0), ("# to Strength and Intelligence", 1.0),
    ("# to all Attributes", 1.0),
];
const DEX: &[(&str, f64)] = &[
    ("# to Dexterity", 1.0), ("# to Strength and Dexterity", 1.0), ("# to Dexterity and Intelligence", 1.0),
    ("# to all Attributes", 1.0),
];
const INT: &[(&str, f64)] = &[
    ("# to Intelligence", 1.0), ("# to Strength and Intelligence", 1.0), ("# to Dexterity and Intelligence", 1.0),
    ("# to all Attributes", 1.0),
];

macro_rules! pseudo {
    ($id:literal, $text:literal, $parts:expr, $always:literal) => {
        Pseudo { id: concat!("pseudo.", $id), text: $text, parts: $parts, always: $always }
    };
}

const SHARED: &[Pseudo] = &[
    pseudo!("pseudo_total_elemental_resistance", "+#% total Elemental Resistance", ELEMENTAL, true),
    pseudo!("pseudo_total_resistance", "+#% total Resistance", ALL_RES, false),
    pseudo!("pseudo_total_energy_shield", "+# total maximum Energy Shield", &[("# to maximum Energy Shield", 1.0)], true),
    pseudo!("pseudo_total_strength", "+# total to Strength", STR, false),
    pseudo!("pseudo_total_dexterity", "+# total to Dexterity", DEX, false),
    pseudo!("pseudo_total_intelligence", "+# total to Intelligence", INT, false),
];

/// PoE 1: the trade site adds half a point of life per Strength and of mana per Intelligence.
const POE1: &[Pseudo] = &[
    pseudo!("pseudo_total_life", "+# total maximum Life", &[
        ("# to maximum Life", 1.0), ("# to Strength", 0.5), ("# to Strength and Dexterity", 0.5),
        ("# to Strength and Intelligence", 0.5), ("# to all Attributes", 0.5),
    ], true),
    pseudo!("pseudo_total_mana", "+# total maximum Mana", &[
        ("# to maximum Mana", 1.0), ("# to Intelligence", 0.5), ("# to Dexterity and Intelligence", 0.5),
        ("# to Strength and Intelligence", 0.5), ("# to all Attributes", 0.5),
    ], true),
];

/// PoE 2: flat life and mana only (attributes are left out, so the total is a safe lower bound),
/// plus an all-attributes total that PoE 1's trade site doesn't have.
const POE2: &[Pseudo] = &[
    pseudo!("pseudo_total_life", "+# total maximum Life", &[("# to maximum Life", 1.0)], true),
    pseudo!("pseudo_total_mana", "+# total maximum Mana", &[("# to maximum Mana", 1.0)], true),
    pseudo!("pseudo_total_attributes", "+# total to Attributes", &[
        ("# to Strength", 1.0), ("# to Dexterity", 1.0), ("# to Intelligence", 1.0),
        ("# to Strength and Dexterity", 2.0), ("# to Strength and Intelligence", 2.0),
        ("# to Dexterity and Intelligence", 2.0), ("# to all Attributes", 3.0),
    ], false),
];

/// The pseudo stats a game's trade site offers, most useful first.
pub fn table(game: Game) -> impl Iterator<Item = &'static Pseudo> {
    let own = match game {
        Game::Poe1 => POE1,
        Game::Poe2 => POE2,
    };
    SHARED[..1].iter().chain(own).chain(&SHARED[1..])
}

/// A pseudo stat's total on one item.
pub struct Total {
    pub pseudo: &'static Pseudo,
    pub value: f64,
    /// Indexes into the item's mods that add to the total.
    pub mods: Vec<usize>,
}

impl Total {
    /// Label for the filter list, e.g. "+55% total Elemental Resistance".
    pub fn label(&self) -> String {
        self.pseudo.text.replacen('#', &format!("{}", self.value.floor()), 1)
    }
}

impl Total {
    /// Whether the total stands in for the whole mod, so the mod's own filter can be dropped while
    /// the total is searched. "+20% to Fire and Chaos Resistances" adds chaos resistance the
    /// elemental total leaves out, and Strength adds more than the life it gives.
    pub fn covers(&self, m: &Mod) -> bool {
        let t = m.template.strip_prefix('+').unwrap_or(&m.template);
        let outside: &[&str] = match self.pseudo.id {
            "pseudo.pseudo_total_elemental_resistance" => &["Chaos"],
            "pseudo.pseudo_total_life" | "pseudo.pseudo_total_mana" => &["Strength", "Dexterity", "Intelligence", "Attributes"],
            _ => &[],
        };
        self.pseudo.parts.iter().any(|(p, _)| *p == t) && !outside.iter().any(|w| t.contains(w))
    }
}

/// Totals for the item's mods; skips totals that just repeat a single mod, total resistance
/// without chaos resistance (the elemental total again), and flat energy shield
/// on armour (local there, so not part of the trade site's global total).
pub fn totals(game: Game, mods: &[Mod], local_defences: bool) -> Vec<Total> {
    let mut out: Vec<Total> = Vec::new();
    for p in table(game) {
        if local_defences && p.id == "pseudo.pseudo_total_energy_shield" {
            continue;
        }
        let mut t = Total { pseudo: p, value: 0.0, mods: Vec::new() };
        let mut single = true;
        for (i, m) in mods.iter().enumerate() {
            let template = m.template.strip_prefix('+').unwrap_or(&m.template);
            let Some(part) = p.parts.iter().position(|(text, _)| *text == template) else { continue };
            let Some(v) = m.values.first() else { continue };
            let mul = p.parts[part].1;
            t.value += v * mul;
            // The pseudo's own mod (listed first) on its own: the total is that mod again.
            single &= t.mods.is_empty() && part == 0 && mul == 1.0;
            t.mods.push(i);
        }
        if t.mods.is_empty() || (single && !p.always) {
            continue;
        }
        // Total resistance without chaos is the elemental total again.
        if p.id == "pseudo.pseudo_total_resistance" && !t.mods.iter().any(|&i| mods[i].template.contains("Chaos")) {
            continue;
        }
        out.push(t);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::parse;

    fn item(mods: &str) -> Vec<Mod> {
        parse(&format!("Item Class: Rings\nRarity: Rare\nX\nRuby Ring\n--------\nItem Level: 80\n--------\n{mods}"))
            .unwrap()
            .mods
    }

    fn summary(game: Game, mods: &str) -> Vec<(&'static str, f64)> {
        totals(game, &item(mods), false).iter().map(|t| (t.pseudo.id, t.value)).collect()
    }

    const RING: &str = "+40% to Fire Resistance\n+35% to Cold Resistance\n+30% to Lightning Resistance\n\
                        +12% to Chaos Resistance\n+20 to Strength\n+60 to maximum Life\n";

    #[test]
    fn poe1_totals() {
        assert_eq!(summary(Game::Poe1, RING), vec![
            ("pseudo.pseudo_total_elemental_resistance", 105.0),
            ("pseudo.pseudo_total_life", 70.0),
            ("pseudo.pseudo_total_resistance", 117.0),
        ]);
    }

    #[test]
    fn poe2_totals_use_poe2_rules() {
        // Same mods: no life from Strength, and the attributes total only PoE 2 has.
        let poe2 = summary(Game::Poe2, &RING.replace("+20 to Strength", "+20 to Strength\n+15 to Dexterity"));
        assert_eq!(poe2, vec![
            ("pseudo.pseudo_total_elemental_resistance", 105.0),
            ("pseudo.pseudo_total_life", 60.0),
            ("pseudo.pseudo_total_attributes", 35.0),
            ("pseudo.pseudo_total_resistance", 117.0),
        ]);
        assert!(table(Game::Poe1).all(|p| p.id != "pseudo.pseudo_total_attributes"));
    }

    #[test]
    fn hybrid_mods_count_once_per_element() {
        let mods = "+15% to all Elemental Resistances\n+20% to Fire and Chaos Resistances\n+18 to all Attributes\n";
        assert_eq!(summary(Game::Poe1, mods), vec![
            ("pseudo.pseudo_total_elemental_resistance", 65.0),
            ("pseudo.pseudo_total_life", 9.0),
            ("pseudo.pseudo_total_mana", 9.0),
            ("pseudo.pseudo_total_resistance", 85.0),
            ("pseudo.pseudo_total_strength", 18.0),
            ("pseudo.pseudo_total_dexterity", 18.0),
            ("pseudo.pseudo_total_intelligence", 18.0),
        ]);
        assert_eq!(summary(Game::Poe2, mods), vec![
            ("pseudo.pseudo_total_elemental_resistance", 65.0),
            ("pseudo.pseudo_total_attributes", 54.0),
            ("pseudo.pseudo_total_resistance", 85.0),
            ("pseudo.pseudo_total_strength", 18.0),
            ("pseudo.pseudo_total_dexterity", 18.0),
            ("pseudo.pseudo_total_intelligence", 18.0),
        ]);
    }

    #[test]
    fn attribute_totals_need_a_hybrid_mod() {
        let s = summary(Game::Poe1, "+20 to Strength\n+10 to Strength and Dexterity\n");
        assert!(s.contains(&("pseudo.pseudo_total_strength", 30.0)));
        assert!(s.contains(&("pseudo.pseudo_total_dexterity", 10.0)), "a hybrid alone still differs from the mod");
        assert!(summary(Game::Poe1, "+20 to Dexterity\n").is_empty(), "a lone mod is its own total");
    }

    #[test]
    fn implicit_and_explicit_add_up() {
        let t = totals(Game::Poe1, &item("+10% to Cold Resistance (implicit)\n+30% to Cold Resistance\n"), false);
        assert_eq!(t[0].label(), "+40% total Elemental Resistance");
        assert_eq!(t[0].mods, vec![0, 1]);
    }

    #[test]
    fn totals_cover_only_whole_mods() {
        let mods = item("+20% to Fire and Chaos Resistances\n+30% to Cold Resistance\n+20 to Strength\n+50 to maximum Life\n");
        let t = totals(Game::Poe1, &mods, false);
        let covered = |id: &str| -> Vec<usize> {
            let t = t.iter().find(|t| t.pseudo.id == id).unwrap();
            (0..mods.len()).filter(|&i| t.covers(&mods[i])).collect()
        };
        assert_eq!(covered("pseudo.pseudo_total_elemental_resistance"), vec![1]);
        assert_eq!(covered("pseudo.pseudo_total_life"), vec![3]);
        assert_eq!(covered("pseudo.pseudo_total_resistance"), vec![0, 1]);
    }

    #[test]
    fn energy_shield_total_skips_local_flat_es() {
        let mods = item("+40 to maximum Energy Shield\n");
        assert_eq!(totals(Game::Poe2, &mods, false)[0].pseudo.id, "pseudo.pseudo_total_energy_shield");
        assert!(totals(Game::Poe2, &mods, true).is_empty());
    }
}
