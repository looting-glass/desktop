//! Dev aid: `LOOTINGGLASS_MOCK=1` answers every trade API and poe.ninja request with canned data,
//! without touching the network, so UI work can run as often as needed without rate limits.
//! The stats and bases cover the items in `dev/*.txt`; other mods show up as unmatched.

use serde_json::{Value, json};

/// Always on in unit tests, so tests never touch the network.
pub fn enabled() -> bool {
    cfg!(test) || std::env::var_os("LOOTINGGLASS_MOCK").is_some()
}

/// Stat texts as the PoE 1 trade API lists them; " (Local)" marks local mods. The PoE 2 API drops
/// the leading "+" ("# to maximum Life"), which the mock reproduces.
const STATS: &[&str] = &[
    "+# to maximum Life",
    "+# to maximum Mana",
    "#% increased Movement Speed",
    "+#% to Fire Resistance",
    "+#% to Cold Resistance",
    "+#% to Lightning Resistance",
    "+#% to Chaos Resistance",
    "+#% to all Elemental Resistances",
    "+# to maximum Energy Shield",
    "+# to Strength",
    "+# to Dexterity",
    "+# to Intelligence",
    "#% increased Physical Damage (Local)",
    "Adds # to # Fire Damage (Local)",
    "#% increased Attack Speed (Local)",
    "#% increased Cast Speed",
    "#% increased Spirit",
    "#% increased Presence Area of Effect",
    "Allies in your Presence deal #% increased Damage",
    "Allies in your Presence have #% increased Attack Speed",
    "+# to Level of all Minion Skills",
    "+#% to Fire and Cold Resistances",
    "Adds # Passive Skills",
    "# Added Passive Skills are Jewel Sockets",
    "1 Added Passive Skill is Prismatic Heart",
    "1 Added Passive Skill is Burning Bright",
];
/// Option stats: listed with their numbers and a `|<option>` id, like the real data does.
const OPTION_STATS: &[(&str, &str, &str)] = &[
    ("enchant", "enchant.stat_3948993189|13", "Added Small Passive Skills grant: 12% increased Fire Damage"),
];
/// The trade API's "pseudo" group: ids and texts as the real `/data/stats` lists them. Both games
/// share most of them; only PoE 2 has the all-attributes total.
const PSEUDO: &[(&str, &str)] = &[
    ("pseudo_total_fire_resistance", "+#% total to Fire Resistance"),
    ("pseudo_total_cold_resistance", "+#% total to Cold Resistance"),
    ("pseudo_total_lightning_resistance", "+#% total to Lightning Resistance"),
    ("pseudo_total_chaos_resistance", "+#% total to Chaos Resistance"),
    ("pseudo_total_elemental_resistance", "+#% total Elemental Resistance"),
    ("pseudo_total_resistance", "+#% total Resistance"),
    ("pseudo_total_strength", "+# total to Strength"),
    ("pseudo_total_dexterity", "+# total to Dexterity"),
    ("pseudo_total_intelligence", "+# total to Intelligence"),
    ("pseudo_total_all_attributes", "+# total to all Attributes"),
    ("pseudo_total_life", "+# total maximum Life"),
    ("pseudo_total_mana", "+# total maximum Mana"),
    ("pseudo_total_energy_shield", "+# total maximum Energy Shield"),
];
const PSEUDO_POE2: &[(&str, &str)] = &[("pseudo_total_attributes", "+# total to Attributes")];
/// PoE 1 searches influences through pseudo stats.
const PSEUDO_POE1: &[(&str, &str)] = &[
    ("pseudo_has_shaper_influence", "Has Shaper Influence"),
    ("pseudo_has_elder_influence", "Has Elder Influence"),
    ("pseudo_has_crusader_influence", "Has Crusader Influence"),
    ("pseudo_has_redeemer_influence", "Has Redeemer Influence"),
    ("pseudo_has_hunter_influence", "Has Hunter Influence"),
    ("pseudo_has_warlord_influence", "Has Warlord Influence"),
];
const GROUPS: &[&str] = &["explicit", "implicit", "desecrated", "fractured", "crafted", "rune", "enchant"];
const BASES: &[&str] = &[
    "Slink Boots", "Dragonscale Boots", "Ezomyte Axe", "Rattling Sceptre", "Ruby Ring", "Astral Plate",
    "Two-Stone Ring", "Large Cluster Jewel", "Expert Hexer's Robe",
];
/// (amount, currency) for the fake listings, cheapest first.
const PRICES: &[(f64, &str)] = &[
    (1.0, "chaos"), (5.0, "chaos"), (12.0, "chaos"), (1.0, "divine"), (1.0, "divine"),
    (2.0, "divine"), (3.0, "divine"), (5.0, "divine"), (15.0, "divine"), (50.0, "divine"),
];

/// Canned response for a request, or `None` for an unknown endpoint.
pub fn response(method: &str, url: &str) -> Option<Value> {
    let path = url.split('?').next().unwrap_or(url);
    if url.contains("api.github.com") && path.ends_with("/releases/latest") {
        return Some(release());
    }
    if url.contains("poe.ninja") {
        return Some(ninja(url));
    }
    if path.ends_with("/data/leagues") {
        return Some(json!({ "result": [
            { "id": "Standard", "realm": "pc" },
            { "id": "Mock League", "realm": "pc" },
            { "id": "Standard", "realm": "poe2" },
            { "id": "Mock League", "realm": "poe2" },
        ]}));
    }
    if path.ends_with("/data/stats") {
        let poe2 = path.contains("/trade2/");
        let mut groups: Vec<Value> = GROUPS
            .iter()
            .map(|g| {
                let mut entries: Vec<Value> = STATS
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        let text = if poe2 { t.strip_prefix('+').unwrap_or(t) } else { t };
                        json!({ "id": format!("{g}.stat_mock{i}"), "text": text })
                    })
                    .collect();
                entries.extend(
                    OPTION_STATS.iter().filter(|(og, ..)| og == g).map(|(_, id, text)| json!({ "id": id, "text": text })),
                );
                json!({ "id": g, "entries": entries })
            })
            .collect();
        let pseudo: Vec<Value> = PSEUDO
            .iter()
            .chain(if poe2 { PSEUDO_POE2 } else { PSEUDO_POE1 })
            .map(|(id, text)| json!({ "id": format!("pseudo.{id}"), "text": text }))
            .collect();
        groups.insert(0, json!({ "id": "pseudo", "entries": pseudo }));
        return Some(json!({ "result": groups }));
    }
    if path.ends_with("/data/items") {
        let entries: Vec<Value> = BASES.iter().map(|b| json!({ "type": b })).collect();
        return Some(json!({ "result": [{ "label": "Mock", "entries": entries }] }));
    }
    if method == "POST" && path.contains("/search/") {
        let ids: Vec<String> = (0..13).map(|i| format!("mock{i}")).collect();
        return Some(json!({ "id": "mock", "total": 13, "result": ids }));
    }
    if let Some(ids) = path.split("/fetch/").nth(1) {
        let now = chrono::Utc::now();
        let result: Vec<Value> = ids
            .split(',')
            .filter_map(|id| id.strip_prefix("mock")?.parse::<usize>().ok())
            .map(|i| {
                let (amount, currency) = PRICES[i % PRICES.len()];
                let indexed = now - chrono::Duration::hours(7 * i as i64 + 3);
                json!({
                    "listing": {
                        "price": { "amount": amount, "currency": currency },
                        "account": { "name": format!("MockSeller#{}", 1000 + i * 37) },
                        "indexed": indexed.to_rfc3339(),
                    },
                    "item": { "ilvl": 75 + (i * 3) % 9, "corrupted": i == 4 },
                })
            })
            .collect();
        return Some(json!({ "result": result }));
    }
    None
}

/// Latest GitHub release: the running version, or v99.0.0 with `LOOTINGGLASS_MOCK_UPDATE=1` so the
/// update notice can be seen.
fn release() -> Value {
    let v = if std::env::var_os("LOOTINGGLASS_MOCK_UPDATE").is_some() { "99.0.0" } else { env!("CARGO_PKG_VERSION") };
    let base = "https://github.com/looting-glass/desktop/releases";
    let assets: Vec<Value> = ["windows-x64-setup.exe", "windows-x64.zip", "x86_64.tar.gz"]
        .iter()
        .flat_map(|s| [format!("lootingglass-{v}-{s}"), format!("lootingglass-{v}-{s}.sha256")])
        .map(|n| json!({ "name": n, "browser_download_url": format!("{base}/download/v{v}/{n}") }))
        .collect();
    json!({
        "tag_name": format!("v{v}"),
        "html_url": format!("{base}/tag/v{v}"),
        "body": "## [99.0.0](https://example.invalid) (2026-11-21)\n\n## Features\n\n* **overlay:** a mock feature ([#99](https://example.invalid))\n\n## Bug Fixes\n\n* a mock fix",
        "assets": assets,
    })
}

/// poe.ninja exchange overview: a few currencies, priced in chaos (PoE 1) or exalted (PoE 2).
fn ninja(url: &str) -> Value {
    if !url.contains("type=Currency") {
        return json!({ "core": { "primary": "chaos", "rates": {} }, "items": [], "lines": [] });
    }
    let poe2 = url.contains("/poe2/");
    let (primary, rates, lines) = if poe2 {
        ("exalted", json!({ "divine": 0.0025, "chaos": 0.05 }),
         [("divine-orb", "Divine Orb", 400.0, 3.1), ("chaos-orb", "Chaos Orb", 20.0, -1.2), ("exalted-orb", "Exalted Orb", 1.0, 0.0)])
    } else {
        ("chaos", json!({ "divine": 0.0055 }),
         [("divine-orb", "Divine Orb", 182.0, 2.4), ("chaos-orb", "Chaos Orb", 1.0, 0.0), ("exalted-orb", "Exalted Orb", 15.0, -4.0)])
    };
    let items: Vec<Value> = lines.iter().map(|(id, name, ..)| json!({ "id": id, "name": name })).collect();
    let lines: Vec<Value> = lines
        .iter()
        .map(|(id, _, v, change)| json!({ "id": id, "primaryValue": v, "sparkline": { "totalChange": change } }))
        .collect();
    json!({ "core": { "primary": primary, "rates": rates }, "items": items, "lines": lines })
}
