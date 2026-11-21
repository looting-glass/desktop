//! poe.ninja exchange prices for stackables (currency, fragments, essences, cards, …).
//! Same API shape for both games: `/{poe1|poe2}/api/economy/exchange/current/overview`.

use crate::config::{Game, urlencode};
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

const TTL: Duration = Duration::from_secs(15 * 60);

const POE1_TYPES: &[&str] = &[
    "Currency", "Fragment", "Essence", "Fossil", "Resonator", "Oil", "DeliriumOrb", "Omen",
    "DivinationCard", "Tattoo", "Artifact",
];
const POE2_TYPES: &[&str] = &[
    "Currency", "Fragments", "Essences", "Ritual", "Runes", "Delirium", "Breach", "Expedition", "Abyss",
    "SoulCores", "UncutGems", "Idols", "LineageSupportGems",
];

#[derive(Clone, Debug)]
pub struct Price {
    /// Value in each known currency id ("divine", "chaos", "exalted", …).
    values: HashMap<String, f64>,
    /// 7-day change in percent.
    pub change: f64,
}

type Table = HashMap<String, Price>;
static CACHE: LazyLock<Mutex<HashMap<(Game, String), (Instant, std::sync::Arc<Table>)>>> =
    LazyLock::new(Default::default);

static HTTP: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder().timeout(Duration::from_secs(15)).build().expect("http client")
});

async fn fetch_type(game: Game, league: &str, kind: &str, ua: &str) -> Result<Vec<(String, Price)>> {
    let seg = match game {
        Game::Poe1 => "poe1",
        Game::Poe2 => "poe2",
    };
    let url = format!(
        "https://poe.ninja/{seg}/api/economy/exchange/current/overview?league={}&type={kind}",
        urlencode(league)
    );
    let v = crate::devcache::json(HTTP.get(url).header("User-Agent", ua), |req| async move {
        Ok(req.send().await?.error_for_status()?.json::<Value>().await?)
    })
    .await?;
    let core = &v["core"];
    let primary = core["primary"].as_str().unwrap_or("chaos").to_string();
    // rates[c] = units of currency c per 1 primary.
    let mut rates: HashMap<String, f64> = core["rates"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(k, r)| Some((k.clone(), r.as_f64()?)))
        .collect();
    rates.insert(primary, 1.0);

    let names: HashMap<&str, &str> = v["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|i| Some((i["id"].as_str()?, i["name"].as_str()?)))
        .collect();
    Ok(v["lines"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|l| {
            let name = names.get(l["id"].as_str()?)?;
            let p = l["primaryValue"].as_f64()?;
            let values = rates.iter().map(|(c, r)| (c.clone(), p * r)).collect();
            let change = l["sparkline"]["totalChange"].as_f64().unwrap_or(0.0);
            Some((name.to_lowercase(), Price { values, change }))
        })
        .collect())
}

async fn table(game: Game, league: &str, ua: &str) -> std::sync::Arc<Table> {
    let key = (game, league.to_string());
    if let Some((at, t)) = CACHE.lock().unwrap().get(&key) {
        if at.elapsed() < TTL {
            return t.clone();
        }
    }
    let types = match game {
        Game::Poe1 => POE1_TYPES,
        Game::Poe2 => POE2_TYPES,
    };
    let results = futures_util::future::join_all(types.iter().map(|t| fetch_type(game, league, t, ua))).await;
    let mut map = Table::new();
    for r in results {
        match r {
            Ok(rows) => map.extend(rows),
            Err(e) => log::warn!("poe.ninja: {e}"),
        }
    }
    log::info!("poe.ninja {} {league}: {} prices", game.short(), map.len());
    let t = std::sync::Arc::new(map);
    CACHE.lock().unwrap().insert(key, (Instant::now(), t.clone()));
    t
}

fn fmt(v: f64) -> String {
    if v >= 100.0 {
        format!("{v:.0}")
    } else if v >= 10.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    }
}

/// How many of currency `cur` ("chaos", "exalted") one Divine Orb is worth.
pub async fn per_divine(game: Game, league: &str, cur: &str, ua: &str) -> Option<f64> {
    table(game, league, ua).await.get("divine orb")?.values.get(cur).copied()
}

/// e.g. ("≈ 1.20 divine · 257 chaos", "▲ 5% (7d) · stack of 12: 3.1 divine"); None if unknown.
pub async fn lookup(game: Game, league: &str, name: &str, stack: Option<u32>, ua: &str) -> Option<(String, String)> {
    let t = table(game, league, ua).await;
    let p = t.get(&name.to_lowercase())?;
    let small = if game == Game::Poe2 && p.values.contains_key("exalted") { "exalted" } else { "chaos" };
    let show = |mult: f64| -> Option<String> {
        let s = p.values.get(small)? * mult;
        Some(match p.values.get("divine").map(|d| d * mult) {
            Some(d) if d >= 1.0 => format!("{} divine · {} {small}", fmt(d), fmt(s)),
            _ => format!("{} {small}", fmt(s)),
        })
    };
    let main = format!("≈ {}", show(1.0)?);
    let arrow = if p.change >= 0.0 { "▲" } else { "▼" };
    let mut detail = format!("{arrow} {:.0}% (7d)", p.change.abs());
    if let Some(n) = stack.filter(|n| *n > 1) {
        detail.push_str(&format!(" · stack of {n}: {}", show(n as f64)?));
    }
    Some((main, detail))
}

#[cfg(test)]
mod tests {
    //! Run against `mock.rs` (always on in tests).
    use super::*;

    fn block<T>(f: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(f)
    }

    #[test]
    fn poe1_price_in_divine_and_chaos_with_stack() {
        let (main, detail) = block(lookup(Game::Poe1, "Standard", "Divine Orb", Some(10), "test")).unwrap();
        assert_eq!(main, "≈ 1.00 divine · 182 chaos");
        assert_eq!(detail, "▲ 2% (7d) · stack of 10: 10.0 divine · 1820 chaos");
    }

    #[test]
    fn poe2_prices_small_currency_in_exalted() {
        let (main, detail) = block(lookup(Game::Poe2, "Standard", "chaos orb", None, "test")).unwrap();
        assert_eq!(main, "≈ 20.0 exalted");
        assert_eq!(detail, "▼ 1% (7d)");
    }

    #[test]
    fn divine_rate_in_small_currency() {
        assert_eq!(block(per_divine(Game::Poe1, "Standard", "chaos", "test")), Some(182.0));
        assert_eq!(block(per_divine(Game::Poe2, "Standard", "exalted", "test")), Some(400.0));
    }

    #[test]
    fn unknown_item_has_no_price() {
        assert!(block(lookup(Game::Poe1, "Standard", "Mirror of Kalandra", None, "test")).is_none());
    }
}
