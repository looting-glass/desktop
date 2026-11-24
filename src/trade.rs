//! Official trade API client (pathofexile.com/api/trade for PoE 1, /api/trade2 for PoE 2).

use crate::config::{Config, Game, urlencode};
use crate::item::{Item, ModKind, Rarity};
use crate::pseudo;
use crate::ratelimit;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::LazyLock;
use tokio::sync::OnceCell;

pub(crate) static HTTP: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .expect("http client")
});

/// Static trade data per game, fetched once per run.
struct GameData {
    /// (stat group, text with `#`) -> [(stat id, is local)]
    stats: HashMap<(String, String), Vec<(String, bool)>>,
    /// Known base types, longest first (for resolving magic item names).
    bases: Vec<String>,
}

static DATA: [OnceCell<GameData>; 2] = [OnceCell::const_new(), OnceCell::const_new()];

#[derive(Debug, Clone, Serialize)]
pub struct Listing {
    pub price: String,
    pub seller: String,
    pub age: String,
    pub ilvl: Option<u64>,
    pub corrupted: bool,
}

#[derive(Debug, Clone)]
pub struct PriceResult {
    pub total: u64,
    pub listings: Vec<Listing>,
    pub url: String,
}

/// One toggleable search criterion shown in the overlay.
/// `id` is a trade stat id (`<id>|<option>` for option stats such as cluster jewel enchants), or
/// `prop:<key>` / `misc:<key>` / `socket:<key>` / `flag:<key>` for item properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    pub id: String,
    pub label: String,
    pub tag: String,
    pub enabled: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// Mod tier and possible roll range ("50–59"), shown next to stat filters from advanced
    /// item text (Ctrl+Alt+C).
    #[serde(default)]
    pub tier: Option<u32>,
    #[serde(default)]
    pub range: Option<String>,
}

/// Everything needed to (re)run a search; round-trips through the overlay as JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spec {
    pub game: Game,
    pub league: String,
    pub online: bool,
    pub name: Option<String>,
    pub base: Option<String>,
    pub use_base: bool,
    /// Trade category for the item class (e.g. "armour.boots"); searched when `use_base` is off.
    pub category: Option<String>,
    pub corrupted: bool,
    pub filter_corrupted: bool,
    pub class: String,
    pub item_level: Option<u32>,
    pub filters: Vec<Filter>,
}

pub(crate) async fn send(req: reqwest::RequestBuilder, ua: &str) -> Result<Value> {
    crate::devcache::json(req, |req| send_live(req, ua)).await
}

/// Sends a trade API request over the network, after the rate limiter allows it (see
/// `ratelimit.rs`); every reply's limit headers feed back into the limiter.
async fn send_live(req: reqwest::RequestBuilder, ua: &str) -> Result<Value> {
    let req = req.header(reqwest::header::USER_AGENT, ua).build()?;
    let endpoint = ratelimit::endpoint(req.url().path());
    ratelimit::acquire(&endpoint).await?;
    let r = HTTP.execute(req).await?;
    let status = r.status();
    if let Some(e) = ratelimit::observe(&endpoint, status.as_u16(), r.headers()) {
        return Err(e);
    }
    let body = r.text().await?;
    let v: Value = serde_json::from_str(&body)
        .with_context(|| format!("Unexpected response ({status}) from trade API"))?;
    if !status.is_success() {
        bail!("{}", v["error"]["message"].as_str().unwrap_or(status.as_str()));
    }
    Ok(v)
}

pub async fn leagues(game: Game, ua: &str) -> Result<Vec<String>> {
    let v = send(HTTP.get(format!("{}/data/leagues", game.api())), ua).await?;
    #[derive(Deserialize)]
    struct L {
        id: String,
        realm: Option<String>,
    }
    let ls: Vec<L> = serde_json::from_value(v["result"].clone())?;
    Ok(ls
        .into_iter()
        .filter(|l| l.realm.as_deref().is_none_or(|r| r == game.realm()))
        .map(|l| l.id)
        .collect())
}

async fn data(game: Game, ua: &str) -> Result<&'static GameData> {
    DATA[game as usize]
        .get_or_try_init(|| async {
            let (stats, items) = futures_util::try_join!(
                send(HTTP.get(format!("{}/data/stats", game.api())), ua),
                send(HTTP.get(format!("{}/data/items", game.api())), ua),
            )?;
            let mut map: HashMap<(String, String), Vec<(String, bool)>> = HashMap::new();
            for g in stats["result"].as_array().into_iter().flatten() {
                let group = g["id"].as_str().unwrap_or_default();
                for e in g["entries"].as_array().into_iter().flatten() {
                    let (Some(id), Some(text)) = (e["id"].as_str(), e["text"].as_str()) else { continue };
                    let (text, local) = match text.strip_suffix(" (Local)") {
                        Some(t) => (t, true),
                        None => (text, false),
                    };
                    map.entry((group.to_string(), text.to_string()))
                        .or_default()
                        .push((id.to_string(), local));
                }
            }
            let mut bases: Vec<String> = items["result"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|g| g["entries"].as_array().into_iter().flatten())
                .filter(|e| e.get("name").is_none())
                .filter_map(|e| e["type"].as_str().map(str::to_string))
                .collect();
            bases.sort_by_key(|b| std::cmp::Reverse(b.len()));
            bases.dedup();
            log::info!("{} trade data: {} stats, {} bases", game.short(), map.len(), bases.len());
            Ok(GameData { stats: map, bases })
        })
        .await
}

fn age(indexed: &str) -> String {
    let Ok(t) = chrono::DateTime::parse_from_rfc3339(indexed) else { return String::new() };
    let mins = (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_minutes().max(0);
    match mins {
        0..60 => format!("{mins}m"),
        60..1440 => format!("{}h", mins / 60),
        _ => format!("{}d", mins / 1440),
    }
}

/// Mods worth enabling by default (APT-style heuristic); everything else starts unchecked.
const IMPORTANT: &[&str] = &[
    "Resistance", "maximum Life", "Movement Speed", "all Attributes", "Spirit", "Level of all",
    "maximum Energy Shield", "Critical", "increased Physical Damage", "Attack Speed", "Cast Speed",
    "Allies in your Presence",
];

const PROP_LABELS: &[(&str, &str)] = &[
    ("ar", "Armour"), ("ev", "Evasion Rating"), ("es", "Energy Shield"), ("ward", "Ward"), ("block", "Block"),
];

/// Trade-site category for an "Item Class:" value (same ids on both trade sites; game-only
/// classes simply never appear in the other game's item text).
fn category(class: &str) -> Option<&'static str> {
    Some(match class {
        "Boots" => "armour.boots",
        "Gloves" => "armour.gloves",
        "Helmets" => "armour.helmet",
        "Body Armours" => "armour.chest",
        "Shields" => "armour.shield",
        "Quivers" => "armour.quiver",
        "Foci" => "armour.focus",
        "Bucklers" => "armour.buckler",
        "Rings" => "accessory.ring",
        "Amulets" => "accessory.amulet",
        "Belts" => "accessory.belt",
        "Claws" => "weapon.claw",
        "Daggers" => "weapon.dagger",
        "Rune Daggers" => "weapon.runedagger",
        "One Hand Swords" => "weapon.onesword",
        "Thrusting One Hand Swords" => "weapon.rapier",
        "One Hand Axes" => "weapon.oneaxe",
        "One Hand Maces" => "weapon.onemace",
        "Sceptres" => "weapon.sceptre",
        "Spears" => "weapon.spear",
        "Flails" => "weapon.flail",
        "Two Hand Swords" => "weapon.twosword",
        "Two Hand Axes" => "weapon.twoaxe",
        "Two Hand Maces" => "weapon.twomace",
        "Warstaves" | "Quarterstaves" => "weapon.warstaff",
        "Staves" => "weapon.staff",
        "Bows" => "weapon.bow",
        "Crossbows" => "weapon.crossbow",
        "Wands" => "weapon.wand",
        "Talismans" => "weapon.talisman",
        "Jewels" | "Abyss Jewels" => "jewel",
        "Maps" => "map",
        "Waystones" => "map.waystone",
        _ => return None,
    })
}

/// The item's base type: magic item names carry affixes ("Sapphire Ring of the Whale"), so they
/// are matched against the trade site's base list.
fn resolve_base(item: &Item, d: &GameData) -> Option<String> {
    match item.rarity {
        Rarity::Magic => d.bases.iter().find(|b| item.base.contains(b.as_str())).cloned(),
        _ => Some(item.base.clone()),
    }
}

/// Base type for an item, or `None` when the trade data can't be loaded or doesn't know it.
pub async fn base_type(item: &Item, cfg: &Config) -> Option<String> {
    let d = data(cfg.game, &cfg.user_agent()).await.ok()?;
    resolve_base(item, d)
}

/// Trade category for an item: cluster jewels have their own, other items go by class.
fn item_category(item: &Item) -> Option<&'static str> {
    if item.base.ends_with("Cluster Jewel") {
        return Some("jewel.cluster");
    }
    category(&item.class)
}

/// Special-item flags each trade site can filter on (`misc_filters` yes/no options).
fn flag_supported(game: Game, key: &str) -> bool {
    match game {
        Game::Poe1 => true,
        Game::Poe2 => matches!(key, "fractured_item" | "veiled"),
    }
}

const FLAG_LABELS: &[(&str, &str)] = &[
    ("fractured_item", "Fractured item"), ("synthesised_item", "Synthesised item"),
    ("searing_item", "Searing Exarch item"), ("tangled_item", "Eater of Worlds item"),
    ("veiled", "Has a veiled mod"),
];

/// Build the default search for an item.
pub async fn prepare(item: &Item, cfg: &Config) -> Result<Spec> {
    let d = data(cfg.game, &cfg.user_agent()).await.context("Failed to load trade data")?;
    let base = resolve_base(item, d);
    let mut filters = Vec::new();
    let f = |id: String, label: String, tag: &str, enabled: bool, min: Option<f64>| Filter {
        id, label, tag: tag.into(), enabled, min, max: None, tier: None, range: None,
    };

    // The trade site compares armour/evasion/ES and physical DPS at 20% quality.
    let quality = item.quality.unwrap_or(0) as f64;
    let can_quality = !item.corrupted && quality < 20.0;
    let q20 = |v: f64, inc: f64| {
        if can_quality { (v * (120.0 + inc) / (100.0 + quality + inc)).round() } else { v.round() }
    };
    let local_inc = |needle: &str| -> f64 {
        item.mods
            .iter()
            .filter(|m| m.template.starts_with("#% increased ") && !m.template.contains("maximum"))
            .filter(|m| m.template.contains(needle))
            .filter_map(|m| m.values.first())
            .sum()
    };
    let qlabel = |name: &str, raw: f64, adj: f64| {
        if adj != raw.round() { format!("{name}: {raw} (Q20: {adj})") } else { format!("{name}: {raw}") }
    };

    for (key, v) in &item.props {
        let name = PROP_LABELS.iter().find(|(k, _)| k == key).map_or(*key, |(_, l)| *l);
        let adj = match *key {
            "ar" => q20(*v, local_inc("Armour")),
            "ev" => q20(*v, local_inc("Evasion")),
            "es" => q20(*v, local_inc("Energy Shield")),
            _ => *v,
        };
        filters.push(f(format!("prop:{key}"), qlabel(name, *v, adj), "property", false, Some(adj)));
    }

    if let Some(w) = item.weapon.as_ref().filter(|w| w.aps > 0.0) {
        let pdps_raw = (w.phys * w.aps).round();
        let pdps = q20(w.phys, local_inc("Physical Damage")) * w.aps;
        let pdps = pdps.round();
        let edps = (w.ele * w.aps).round();
        let cdps = (w.chaos * w.aps).round();
        let dps = pdps + edps + cdps;
        if w.phys > 0.0 {
            filters.push(f("prop:pdps".into(), qlabel("Physical DPS", pdps_raw, pdps), "weapon", true, Some(pdps)));
        }
        if w.ele > 0.0 {
            filters.push(f("prop:edps".into(), format!("Elemental DPS: {edps}"), "weapon", edps > pdps, Some(edps)));
        }
        filters.push(f("prop:dps".into(), format!("Total DPS: {dps}"), "weapon", false, Some(dps)));
        filters.push(f("prop:aps".into(), format!("Attacks per Second: {:.2}", w.aps), "weapon", false, Some(w.aps)));
        if w.crit > 0.0 {
            filters.push(f("prop:crit".into(), format!("Critical Chance: {:.2}%", w.crit), "weapon", false, Some(w.crit)));
        }
    }
    if let Some(l) = item.gem_level {
        filters.push(f("misc:gem_level".into(), format!("Gem Level: {l}"), "property", true, Some(l as f64)));
    }
    if let (Some(q), true) = (item.quality, item.rarity == Rarity::Gem) {
        filters.push(f("misc:quality".into(), format!("Quality: {q}%"), "property", false, Some(q as f64)));
    }
    if let (Some(l), false) = (item.item_level, matches!(item.rarity, Rarity::Gem | Rarity::Currency)) {
        filters.push(f("misc:ilvl".into(), format!("Item Level: {l}"), "property", false, Some(l as f64)));
    }

    // Sockets: links set a PoE 1 item's price (5- and 6-links); PoE 2 has rune sockets instead.
    let unique = item.rarity == Rarity::Unique;
    match (cfg.game, item.socket_count()) {
        (_, 0) => {}
        (Game::Poe1, n) => {
            let links = item.max_links();
            if links >= 2 {
                filters.push(f("socket:links".into(), format!("Links: {links}"), "socket", links >= 5, Some(links as f64)));
            }
            filters.push(f("socket:sockets".into(), format!("Sockets: {n}"), "socket", false, Some(n as f64)));
        }
        (Game::Poe2, n) => {
            filters.push(f("prop:rune_sockets".into(), format!("Rune Sockets: {n}"), "socket", false, Some(n as f64)));
        }
    }
    // PoE 1 influences are pseudo stats on the trade site.
    if cfg.game == Game::Poe1 {
        for inf in &item.influences {
            let id = format!("pseudo.pseudo_has_{inf}_influence");
            if !d.stats.values().flatten().any(|(i, _)| *i == id) {
                log::info!("no trade stat {id}");
                continue;
            }
            let mut name = inf.to_string();
            name[..1].make_ascii_uppercase();
            filters.push(f(id, format!("{name} influence"), "influence", !unique, None));
        }
    }
    // Maps and waystones are priced by tier: their base ("Map (Tier 12)") names nothing else.
    if let Some(t) = item.map.as_ref().and_then(|m| m.tier) {
        let what = if item.class == "Waystones" { "Waystone" } else { "Map" };
        let mut tier = f("map:map_tier".into(), format!("{what} Tier: {t}"), "property", true, Some(t as f64));
        tier.max = Some(t as f64);
        filters.push(tier);
    }
    for flag in item.flags.iter().filter(|k| flag_supported(cfg.game, k)) {
        let label = FLAG_LABELS.iter().find(|(k, _)| k == flag).map_or(*flag, |(_, l)| *l);
        filters.push(f(format!("flag:{flag}"), label.into(), "property", !unique && *flag != "veiled", None));
    }

    let tol = 1.0 - cfg.trade.stat_tolerance as f64 / 100.0;
    let mut stats = Vec::new();
    // Index into `item.mods` for each entry of `stats`.
    let mut stat_mods = Vec::new();
    for (mi, m) in item.mods.iter().enumerate() {
        // PoE 2's trade data drops the leading "+" ("# to maximum Mana"); PoE 1 keeps it, so the
        // fallback is PoE 2-only to avoid matching a different PoE 1 stat.
        let group = m.kind.trade_type().to_string();
        let ids = d.stats.get(&(group.clone(), m.template.clone())).or_else(|| {
            (cfg.game == Game::Poe2)
                .then(|| d.stats.get(&(group.clone(), m.template.replace("+#", "#"))))
                .flatten()
        });
        // Option stats are listed with their numbers ("Added Small Passive Skills grant: 12%
        // increased Fire Damage"), so match the mod's own text.
        let by_text = ids.is_none();
        let ids = ids.or_else(|| d.stats.get(&(group, m.text.clone())));
        let Some(ids) = ids else {
            log::info!("no trade stat for {} mod: {:?}", m.kind.trade_type(), m.template);
            continue;
        };
        let id = ids.iter().find(|(_, l)| *l == item.has_local_props).unwrap_or(&ids[0]).0.clone();
        // A stat matched by its full text has nothing left to compare ("1 Added Passive Skill is …").
        let avg = |v: &mut dyn Iterator<Item = f64>, n: usize| v.sum::<f64>() / n as f64;
        // With "minimum from tier", a ranged roll's minimum is the bottom of its tier's range.
        let tier_min = (cfg.trade.min_from_tier && !by_text && m.ranges.len() == m.values.len() && !m.ranges.is_empty())
            .then(|| avg(&mut m.ranges.iter().map(|r| r.0), m.ranges.len()).floor());
        let min = tier_min.or_else(|| {
            (!m.values.is_empty() && !by_text)
                .then(|| avg(&mut m.values.iter().copied(), m.values.len()))
                .filter(|avg| *avg > 0.0)
                .map(|avg| (avg * tol).floor())
        });
        let important = IMPORTANT.iter().any(|k| m.template.contains(k));
        let mut filter = f(id, m.text.clone(), m.kind.trade_type(), important, min);
        filter.tier = m.tier;
        filter.range = (!m.ranges.is_empty()).then(|| {
            m.ranges.iter().map(|(lo, hi)| format!("{}–{}", num_text(*lo), num_text(*hi))).collect::<Vec<_>>().join(", ")
        });
        stats.push(filter);
        stat_mods.push(mi);
    }
    // Few mods: search them all. Uniques: search by name, stats are opt-in for roll checks.
    if stats.len() <= 3 {
        stats.iter_mut().for_each(|s| s.enabled = true);
    }
    // Cluster jewels are priced by their enchant (passive count and type) and notables.
    if item_category(item) == Some("jewel.cluster") {
        for (s, mi) in stats.iter_mut().zip(&stat_mods) {
            let m = &item.mods[*mi];
            if m.kind == ModKind::Enchant || m.text.starts_with("1 Added Passive Skill is ") {
                s.enabled = true;
            }
        }
    }
    if item.rarity == Rarity::Unique {
        stats.iter_mut().for_each(|s| s.enabled = false);
    }

    // Totals over related mods. On armour and jewellery the elemental resistance and life totals
    // replace their mods by default; the mods stay listed, unticked, for an exact search.
    let gear = item_category(item).is_some_and(|c| c.starts_with("armour.") || c.starts_with("accessory."));
    for t in pseudo::totals(cfg.game, &item.mods, item.has_local_props) {
        let known = d.stats.get(&("pseudo".to_string(), t.pseudo.text.to_string()));
        if !known.is_some_and(|ids| ids.iter().any(|(id, _)| id == t.pseudo.id)) {
            log::info!("no {} trade stat {}", cfg.game.short(), t.pseudo.id);
            continue;
        }
        let on = gear
            && item.rarity != Rarity::Unique
            && matches!(t.pseudo.id, "pseudo.pseudo_total_elemental_resistance" | "pseudo.pseudo_total_life");
        if on {
            for (s, mi) in stats.iter_mut().zip(&stat_mods) {
                if t.covers(&item.mods[*mi]) {
                    s.enabled = false;
                }
            }
        }
        let min = (t.value * tol).floor();
        filters.push(f(t.pseudo.id.to_string(), t.label(), "pseudo", on, (min > 0.0).then_some(min)));
    }
    filters.extend(stats);

    Ok(Spec {
        game: cfg.game,
        league: cfg.league().to_string(),
        online: cfg.trade.status != "any",
        name: (item.rarity == Rarity::Unique).then(|| item.name.clone()).flatten(),
        // A map's base is only its tier, which the map tier filter covers.
        base: if item.map.is_some() { None } else { base },
        // Rares are about their mods, not the exact base: search the whole category by
        // default (like Awakened PoE Trade); exact base stays one click away. Maps always
        // search their category.
        use_base: item.map.is_none() && !(item.rarity == Rarity::Rare && item_category(item).is_some()),
        category: item_category(item).map(str::to_string),
        corrupted: item.corrupted,
        filter_corrupted: matches!(item.rarity, Rarity::Unique | Rarity::Gem),
        class: item.class.clone(),
        item_level: item.item_level,
        filters,
    })
}

/// Whole numbers go out as integers: some filters (links, sockets) reject `6.0`.
/// `50` for whole numbers, `1.5` otherwise.
fn num_text(x: f64) -> String {
    if x.fract() == 0.0 { format!("{x:.0}") } else { format!("{x}") }
}

fn num(x: f64) -> Value {
    if x.fract() == 0.0 && x.abs() < 1e15 { json!(x as i64) } else { json!(x) }
}

fn range(f: &Filter) -> Value {
    let mut v = json!({});
    if let Some(m) = f.min { v["min"] = num(m) }
    if let Some(m) = f.max { v["max"] = num(m) }
    v
}

fn build_query(spec: &Spec) -> Value {
    let status = if spec.online { "online" } else { "any" };
    let mut q = json!({ "status": { "option": status } });
    if let Some(n) = &spec.name {
        q["name"] = json!(n);
    }
    if let (true, Some(b)) = (spec.use_base, &spec.base) {
        q["type"] = json!(b);
    }
    let category = spec.category.as_ref().filter(|_| !spec.use_base);

    // Filter groups differ between the two trade sites.
    let (def_group, lvl_group) = match spec.game {
        Game::Poe1 => ("armour_filters", "misc_filters"),
        Game::Poe2 => ("equipment_filters", "type_filters"),
    };
    let mut groups: serde_json::Map<String, Value> = serde_json::Map::new();
    let mut put = |group: &str, key: &str, val: Value| {
        groups.entry(group).or_insert_with(|| json!({ "filters": {} }))["filters"][key] = val;
    };
    if let Some(c) = category {
        put("type_filters", "category", json!({ "option": c }));
    }
    if spec.filter_corrupted {
        put("misc_filters", "corrupted", json!({ "option": spec.corrupted.to_string() }));
    }
    let mut stats = Vec::new();
    for f in spec.filters.iter().filter(|f| f.enabled) {
        if let Some(k) = f.id.strip_prefix("prop:") {
            let g = match (k, spec.game) {
                ("pdps" | "edps" | "dps" | "aps" | "crit", Game::Poe1) => "weapon_filters",
                _ => def_group,
            };
            put(g, k, range(f));
        } else if let Some(k) = f.id.strip_prefix("misc:") {
            let g = if k == "gem_level" { "misc_filters" } else { lvl_group };
            put(g, k, range(f));
        } else if let Some(k) = f.id.strip_prefix("map:") {
            put("map_filters", k, range(f));
        } else if let Some(k) = f.id.strip_prefix("socket:") {
            put("socket_filters", k, range(f));
        } else if let Some(k) = f.id.strip_prefix("flag:") {
            put("misc_filters", k, json!({ "option": "true" }));
        } else if let Some((id, opt)) = f.id.split_once('|') {
            let opt = opt.parse::<i64>().map_or_else(|_| json!(opt), |n| json!(n));
            stats.push(json!({ "id": id, "value": { "option": opt } }));
        } else {
            stats.push(json!({ "id": f.id, "value": range(f) }));
        }
    }
    if !groups.is_empty() {
        q["filters"] = Value::Object(groups);
    }
    if !stats.is_empty() {
        q["stats"] = json!([{ "type": "and", "filters": stats }]);
    }
    json!({ "query": q, "sort": { "price": "asc" } })
}

pub async fn search(spec: &Spec, cfg: &Config) -> Result<PriceResult> {
    let ua = cfg.user_agent();
    let game = spec.game;
    let body = build_query(spec);
    log::debug!("query: {body}");

    let search_url = format!("{}/search/{}{}", game.api(), game.realm_path(), urlencode(&spec.league));
    let res = send(HTTP.post(search_url).json(&body), &ua).await?;
    let id = res["id"].as_str().context("search returned no id")?.to_string();
    let total = res["total"].as_u64().unwrap_or(0);
    log::info!("search: {total} matches");
    let hashes: Vec<&str> = res["result"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .take(cfg.trade.max_results.clamp(1, 20) as usize)
        .collect();

    let mut listings = Vec::new();
    // The fetch endpoint takes at most 10 ids per request.
    for chunk in hashes.chunks(10) {
        let mut url = format!("{}/fetch/{}?query={id}", game.api(), chunk.join(","));
        if game == Game::Poe2 {
            url.push_str("&realm=poe2");
        }
        let fetched = send(HTTP.get(url), &ua).await?;
        for r in fetched["result"].as_array().into_iter().flatten() {
            let l = &r["listing"];
            let price = match (l["price"]["amount"].as_f64(), l["price"]["currency"].as_str()) {
                (Some(a), Some(c)) => format!("{} {c}", (a * 100.0).round() / 100.0),
                _ => "—".into(),
            };
            listings.push(Listing {
                price,
                seller: l["account"]["name"].as_str().unwrap_or_default().to_string(),
                age: age(l["indexed"].as_str().unwrap_or_default()),
                ilvl: r["item"]["ilvl"].as_u64(),
                corrupted: r["item"]["corrupted"].as_bool().unwrap_or(false),
            });
        }
    }
    Ok(PriceResult { total, listings, url: game.web_search(&spec.league, &id) })
}

#[cfg(test)]
mod tests {
    //! Run against `mock.rs` (always on in tests): no network, deterministic data.
    use super::*;
    use crate::item::parse;

    fn block<T>(f: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(f)
    }

    fn cfg(game: Game) -> Config {
        Config { game, ..Config::default() }
    }

    fn spec_for(sample: &str, game: Game) -> Spec {
        let item = parse(sample).expect("sample parses");
        block(prepare(&item, &cfg(game))).expect("prepare")
    }

    fn filter<'a>(spec: &'a Spec, label_part: &str) -> &'a Filter {
        spec.filters
            .iter()
            .find(|f| f.label.contains(label_part))
            .unwrap_or_else(|| panic!("no filter with {label_part:?} in {:?}", spec.filters))
    }

    #[test]
    fn advanced_text_shows_tiers_and_can_set_minimum_from_tier() {
        let sample = include_str!("../dev/sceptre-poe2.txt");
        let s = spec_for(sample, Game::Poe2);
        let dmg = filter(&s, "increased Damage");
        assert_eq!((dmg.tier, dmg.range.as_deref()), (Some(4), Some("65–74")));
        assert_eq!(dmg.min, Some(66.0), "tolerance by default: 74 minus 10%");
        assert_eq!(filter(&s, "Minion Skills").range, None, "fixed value");

        let mut c = cfg(Game::Poe2);
        c.trade.min_from_tier = true;
        let s = block(prepare(&parse(sample).unwrap(), &c)).unwrap();
        assert_eq!(filter(&s, "increased Damage").min, Some(65.0), "bottom of the tier's range");
        assert_eq!(filter(&s, "Minion Skills").min, Some(3.0), "no range: tolerance as before");
    }

    #[test]
    fn maps_and_waystones_search_their_category_by_tier() {
        for (sample, game, cat, label, tier) in [
            (include_str!("../dev/map.txt"), Game::Poe1, "map", "Map Tier: 12", 12.0),
            (include_str!("../dev/waystone-poe2.txt"), Game::Poe2, "map.waystone", "Waystone Tier: 15", 15.0),
        ] {
            let s = spec_for(sample, game);
            assert_eq!((s.base.as_deref(), s.use_base), (None, false), "{label}: the base names only the tier");
            let t = filter(&s, label);
            assert_eq!((t.enabled, t.min, t.max), (true, Some(tier), Some(tier)));
            let q = build_query(&s);
            assert_eq!(q["query"]["filters"]["type_filters"]["filters"]["category"]["option"], cat);
            assert_eq!(q["query"]["filters"]["map_filters"]["filters"]["map_tier"], json!({ "min": tier as u32, "max": tier as u32 }));
            assert!(q["query"].get("type").is_none());
        }
    }

    #[test]
    fn rare_boots_get_category_and_sensible_defaults() {
        let s = spec_for(include_str!("../dev/boots.txt"), Game::Poe1);
        assert_eq!(s.category.as_deref(), Some("armour.boots"));
        assert!(!s.use_base, "rares search the whole category, not the exact base");
        for on in ["total maximum Life", "total Elemental Resistance", "Movement Speed"] {
            assert!(filter(&s, on).enabled, "{on} should be enabled by default");
        }
        for off in ["+52 to maximum Life", "Cold Resistance", "Lightning Resistance", "Dexterity"] {
            assert!(!filter(&s, off).enabled, "{off} should start unchecked");
        }
        // Minimum = roll minus the 10% default tolerance, rounded down.
        assert_eq!(filter(&s, "+52 to maximum Life").min, Some(46.0));
        assert_eq!(filter(&s, "total Elemental Resistance").min, Some(49.0));
    }

    fn pseudos(s: &Spec) -> Vec<(&str, &str, bool)> {
        s.filters
            .iter()
            .filter(|f| f.tag == "pseudo")
            .map(|f| (f.id.as_str(), f.label.as_str(), f.enabled))
            .collect()
    }

    #[test]
    fn poe1_ring_searches_totals() {
        let s = spec_for(include_str!("../dev/ring.txt"), Game::Poe1);
        assert_eq!(pseudos(&s), vec![
            ("pseudo.pseudo_total_elemental_resistance", "+90% total Elemental Resistance", true),
            ("pseudo.pseudo_total_life", "+58 total maximum Life", true),
            ("pseudo.pseudo_total_resistance", "+107% total Resistance", false),
        ]);
        assert!(filter(&s, "Chaos Resistance").enabled, "chaos isn't part of the elemental total");
        assert!(!filter(&s, "Fire Resistance").enabled, "implicits add to the total too");
        assert!(!filter(&s, "+48 to maximum Life").enabled);
        let q = build_query(&s);
        let ids: Vec<&str> =
            q["query"]["stats"][0]["filters"].as_array().unwrap().iter().map(|f| f["id"].as_str().unwrap()).collect();
        assert!(ids.contains(&"pseudo.pseudo_total_elemental_resistance"));
        assert!(!ids.iter().any(|id| id.ends_with("stat_mock4")), "cold resistance isn't searched on its own");
    }

    #[test]
    fn poe2_ring_searches_poe2_totals() {
        let s = spec_for(include_str!("../dev/ring-poe2.txt"), Game::Poe2);
        assert_eq!(pseudos(&s), vec![
            ("pseudo.pseudo_total_elemental_resistance", "+88% total Elemental Resistance", true),
            ("pseudo.pseudo_total_life", "+61 total maximum Life", true),
            ("pseudo.pseudo_total_attributes", "+32 total to Attributes", false),
        ]);
        assert_eq!(filter(&s, "total Elemental").min, Some(79.0));
        assert!(!filter(&s, "all Elemental Resistances").enabled);
    }

    #[test]
    fn pseudos_start_unchecked_off_armour_and_jewellery() {
        let s = spec_for(include_str!("../dev/sceptre-poe2.txt"), Game::Poe2);
        assert_eq!(pseudos(&s), vec![("pseudo.pseudo_total_mana", "+13 total maximum Mana", false)]);
    }

    #[test]
    fn defences_are_compared_at_20_quality() {
        let s = spec_for(include_str!("../dev/boots.txt"), Game::Poe1);
        let ev = s.filters.iter().find(|f| f.id == "prop:ev").expect("evasion filter");
        assert_eq!(ev.label, "Evasion Rating: 260 (Q20: 312)");
        assert_eq!(ev.min, Some(312.0));
    }

    #[test]
    fn weapon_dps_filters() {
        let s = spec_for(include_str!("../dev/axe.txt"), Game::Poe1);
        let by_id = |id: &str| s.filters.iter().find(|f| f.id == id).unwrap_or_else(|| panic!("no {id}"));
        // 120-200 phys at 1.32 APS, 8% quality and 112% increased local physical damage:
        // 211 now, 223 at 20% quality.
        assert_eq!(by_id("prop:pdps").label, "Physical DPS: 211 (Q20: 223)");
        assert!(by_id("prop:pdps").enabled);
        assert_eq!(by_id("prop:edps").min, Some(40.0));
        assert!(!by_id("prop:edps").enabled, "elemental DPS is lower than physical");
        assert_eq!(by_id("prop:dps").min, Some(263.0));
    }

    #[test]
    fn poe2_matches_stats_listed_without_plus() {
        let s = spec_for(include_str!("../dev/sceptre-poe2.txt"), Game::Poe2);
        // The PoE 2 trade data says "# to maximum Mana"; the item says "+13 to maximum Mana".
        assert!(filter(&s, "+13 to maximum Mana").id.starts_with("explicit."));
        assert!(filter(&s, "Spirit").id.starts_with("desecrated."), "desecrated mods keep their group");
        assert!(!s.filters.iter().any(|f| f.label.contains("Onslaught")), "unknown stats are skipped");
    }

    #[test]
    fn query_filter_groups_differ_per_game() {
        let mut s = spec_for(include_str!("../dev/boots.txt"), Game::Poe1);
        for f in s.filters.iter_mut().filter(|f| f.id == "prop:ev" || f.id == "misc:ilvl") {
            f.enabled = true;
        }
        let q = build_query(&s);
        let filters = &q["query"]["filters"];
        assert_eq!(filters["type_filters"]["filters"]["category"]["option"], "armour.boots");
        assert_eq!(filters["armour_filters"]["filters"]["ev"]["min"], 312.0);
        assert_eq!(filters["misc_filters"]["filters"]["ilvl"]["min"], 82.0);
        assert!(q["query"]["type"].is_null(), "category search sends no base type");

        s.game = Game::Poe2;
        let q = build_query(&s);
        let filters = &q["query"]["filters"];
        assert_eq!(filters["equipment_filters"]["filters"]["ev"]["min"], 312.0);
        assert_eq!(filters["type_filters"]["filters"]["ilvl"]["min"], 82.0);
        assert!(filters["armour_filters"].is_null());
    }

    #[test]
    fn query_uses_exact_base_when_asked() {
        let mut s = spec_for(include_str!("../dev/boots.txt"), Game::Poe1);
        s.use_base = true;
        let q = build_query(&s);
        assert_eq!(q["query"]["type"], "Slink Boots");
        assert!(q["query"]["filters"]["type_filters"].is_null());
    }

    #[test]
    fn magic_items_resolve_to_their_base() {
        let magic = parse("Item Class: Boots\nRarity: Magic\nFleet Slink Boots of the Fox\n--------\n").unwrap();
        assert_eq!(block(base_type(&magic, &cfg(Game::Poe1))).as_deref(), Some("Slink Boots"));
        let rare = parse(include_str!("../dev/boots.txt")).unwrap();
        assert_eq!(block(base_type(&rare, &cfg(Game::Poe1))).as_deref(), Some("Slink Boots"));
    }

    fn by_id<'a>(s: &'a Spec, id: &str) -> &'a Filter {
        s.filters.iter().find(|f| f.id == id).unwrap_or_else(|| panic!("no {id} in {:?}", s.filters))
    }

    #[test]
    fn six_links_are_searched() {
        let s = spec_for(include_str!("../dev/sixlink.txt"), Game::Poe1);
        let links = by_id(&s, "socket:links");
        assert_eq!((links.label.as_str(), links.min, links.enabled), ("Links: 6", Some(6.0), true));
        assert!(!by_id(&s, "socket:sockets").enabled);
        let q = build_query(&s);
        assert_eq!(q["query"]["filters"]["socket_filters"]["filters"]["links"]["min"], json!(6), "the API wants an integer");
        assert!(q["query"]["filters"]["socket_filters"]["filters"]["sockets"].is_null());
    }

    #[test]
    fn influences_and_special_items_are_searched() {
        let s = spec_for(include_str!("../dev/influenced.txt"), Game::Poe1);
        assert!(by_id(&s, "pseudo.pseudo_has_hunter_influence").enabled);
        assert!(by_id(&s, "pseudo.pseudo_has_redeemer_influence").enabled);
        assert!(by_id(&s, "flag:fractured_item").enabled);
        assert!(!by_id(&s, "flag:veiled").enabled, "veiled is opt-in");
        let q = build_query(&s);
        assert_eq!(q["query"]["filters"]["misc_filters"]["filters"]["fractured_item"]["option"], "true");
        let ids: Vec<&str> =
            q["query"]["stats"][0]["filters"].as_array().unwrap().iter().map(|f| f["id"].as_str().unwrap()).collect();
        assert!(ids.contains(&"pseudo.pseudo_has_hunter_influence"));
        // The same lines in a PoE 2 item: no influences there, and only the flags PoE 2 has.
        let s2 = spec_for(include_str!("../dev/influenced.txt"), Game::Poe2);
        assert!(!s2.filters.iter().any(|f| f.tag == "influence"));
        assert!(s2.filters.iter().any(|f| f.id == "flag:fractured_item"));
        let synth = parse("Item Class: Rings\nRarity: Rare\nX\nSynthesised Opal Ring\n--------\nSynthesised Item\n").unwrap();
        let s2 = block(prepare(&synth, &cfg(Game::Poe2))).unwrap();
        assert!(!s2.filters.iter().any(|f| f.id == "flag:synthesised_item"), "PoE 2 has no synthesised items");
    }

    #[test]
    fn cluster_jewels_search_enchant_and_notables() {
        let s = spec_for(include_str!("../dev/cluster.txt"), Game::Poe1);
        assert_eq!(s.category.as_deref(), Some("jewel.cluster"));
        for on in ["Adds 8 Passive Skills", "12% increased Fire Damage", "Prismatic Heart", "Burning Bright"] {
            assert!(filter(&s, on).enabled, "{on} should be ticked");
        }
        let small = filter(&s, "Added Small Passive Skills grant");
        assert_eq!((small.id.as_str(), small.min), ("enchant.stat_3948993189|13", None));
        assert_eq!(filter(&s, "Prismatic Heart").min, None, "notables have no value to compare");
        let q = build_query(&s);
        let stats = q["query"]["stats"][0]["filters"].as_array().unwrap();
        let opt = stats.iter().find(|f| f["id"] == "enchant.stat_3948993189").expect("option stat sent");
        assert_eq!(opt["value"], json!({ "option": 13 }));
    }

    #[test]
    fn poe2_rune_sockets() {
        let s = spec_for(include_str!("../dev/runes-poe2.txt"), Game::Poe2);
        let rs = by_id(&s, "prop:rune_sockets");
        assert_eq!((rs.label.as_str(), rs.min, rs.enabled), ("Rune Sockets: 2", Some(2.0), false));
        assert!(!s.filters.iter().any(|f| f.id.starts_with("socket:")), "no PoE 1 socket filters");
        assert!(s.filters.iter().any(|f| f.tag == "rune"), "socketed runes are their own mods");
        let mut s = s;
        s.filters.iter_mut().for_each(|f| f.enabled = f.id == "prop:rune_sockets");
        assert_eq!(build_query(&s)["query"]["filters"]["equipment_filters"]["filters"]["rune_sockets"]["min"], 2.0);
    }

    #[test]
    fn category_table() {
        assert_eq!(category("Quarterstaves"), Some("weapon.warstaff"));
        assert_eq!(category("Boots"), Some("armour.boots"));
        assert_eq!(category("Stackable Currency"), None);
    }

    #[test]
    fn search_fetches_listings_in_chunks_of_ten() {
        let s = spec_for(include_str!("../dev/boots.txt"), Game::Poe1);
        let r = block(search(&s, &cfg(Game::Poe1))).unwrap();
        assert_eq!(r.total, 13);
        assert_eq!(r.listings.len(), 13, "10 + 3 ids over two fetches");
        assert_eq!(r.listings[0].price, "1 chaos");
        assert!(r.listings[4].corrupted);
        assert_eq!(r.url, "https://www.pathofexile.com/trade/search/Standard/mock");
    }

    #[test]
    fn leagues_are_filtered_by_realm() {
        let poe2 = block(leagues(Game::Poe2, "test")).unwrap();
        assert_eq!(poe2, vec!["Standard", "Mock League"]);
    }
}
