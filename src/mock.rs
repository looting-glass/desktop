//! Dev aid: `LOOTLENS_MOCK=1` answers every trade API and poe.ninja request with canned data,
//! without touching the network, so UI work can run as often as needed without rate limits.
//! The stats and bases cover the items in `dev/*.txt`; other mods show up as unmatched.

use serde_json::{Value, json};

/// Always on in unit tests, so tests never touch the network.
pub fn enabled() -> bool {
    cfg!(test) || std::env::var_os("LOOTLENS_MOCK").is_some()
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
];
const GROUPS: &[&str] = &["explicit", "implicit", "desecrated", "fractured", "crafted", "rune"];
const BASES: &[&str] = &["Slink Boots", "Dragonscale Boots", "Ezomyte Axe", "Rattling Sceptre"];
/// (amount, currency) for the fake listings, cheapest first.
const PRICES: &[(f64, &str)] = &[
    (1.0, "chaos"), (5.0, "chaos"), (12.0, "chaos"), (1.0, "divine"), (1.0, "divine"),
    (2.0, "divine"), (3.0, "divine"), (5.0, "divine"), (15.0, "divine"), (50.0, "divine"),
];

/// Canned response for a request, or `None` for an unknown endpoint.
pub fn response(method: &str, url: &str) -> Option<Value> {
    let path = url.split('?').next().unwrap_or(url);
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
        let groups: Vec<Value> = GROUPS
            .iter()
            .map(|g| {
                let entries: Vec<Value> = STATS
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        let text = if poe2 { t.strip_prefix('+').unwrap_or(t) } else { t };
                        json!({ "id": format!("{g}.stat_mock{i}"), "text": text })
                    })
                    .collect();
                json!({ "id": g, "entries": entries })
            })
            .collect();
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
