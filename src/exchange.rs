//! Trade site bulk exchange for stackables: the cheapest current offers for an item, next to the
//! poe.ninja average. Ids come from the trade API's `/data/static`; both games share the API shape
//! (`/api/trade/exchange/<league>`, `/api/trade2/exchange/poe2/<league>`).

use crate::config::{Game, urlencode};
use crate::trade::{HTTP, send};
use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use tokio::sync::OnceCell;

/// Lowercase item name -> exchange id ("divine orb" -> "divine"), per game.
static IDS: [OnceCell<HashMap<String, String>>; 2] = [OnceCell::const_new(), OnceCell::const_new()];

/// Offers shown in the overlay.
const SHOWN: usize = 4;

async fn ids(game: Game, ua: &str) -> Result<&'static HashMap<String, String>> {
    IDS[game as usize]
        .get_or_try_init(|| async {
            let v = send(HTTP.get(format!("{}/data/static", game.api())), ua).await?;
            Ok::<_, anyhow::Error>(
                v["result"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|g| g["entries"].as_array().into_iter().flatten())
                    .filter_map(|e| Some((e["text"].as_str()?.to_lowercase(), e["id"].as_str()?.to_string())))
                    .collect(),
            )
        })
        .await
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Offer {
    /// "880 chaos for 1"
    pub ratio: String,
    pub stock: u64,
    /// Price of one item in the paying currency.
    pub unit: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Exchange {
    /// Cheapest offers first.
    pub offers: Vec<Offer>,
    /// Best price per unit, e.g. "880 chaos · 1.00 divine".
    pub best: String,
    pub total: u64,
    pub url: String,
}

/// Currency the item is priced in: chaos (PoE 1) or exalted (PoE 2), or divine for that currency itself.
fn paying(game: Game, id: &str) -> &'static str {
    let small = match game {
        Game::Poe1 => "chaos",
        Game::Poe2 => "exalted",
    };
    if id == small { "divine" } else { small }
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

/// Bulk exchange offers for `name`; None if the item isn't traded on the exchange. The best unit
/// price is also shown in divine, at poe.ninja's rate.
pub async fn lookup(game: Game, league: &str, status: &str, name: &str, ua: &str) -> Result<Option<Exchange>> {
    let Some(id) = ids(game, ua).await?.get(&name.to_lowercase()) else {
        return Ok(None);
    };
    let pay = paying(game, id);
    // The exchange knows "online" and "any"; PoE 2's instant buyout is a search-only status.
    let status = if status == "any" { "any" } else { "online" };
    let body = json!({
        "query": { "status": { "option": status }, "have": [pay], "want": [id] },
        "sort": { "have": "asc" },
        "engine": "new",
    });
    let url = format!("{}/exchange/{}{}", game.api(), game.realm_path(), urlencode(league));
    let v = send(HTTP.post(url).json(&body), ua).await?;
    let search_id = v["id"].as_str().context("exchange returned no id")?;

    let mut offers: Vec<Offer> = v["result"]
        .as_object()
        .into_iter()
        .flat_map(|r| r.values())
        .flat_map(|r| r["listing"]["offers"].as_array().into_iter().flatten())
        .filter_map(offer)
        .collect();
    offers.sort_by(|a, b| a.unit.total_cmp(&b.unit));
    offers.truncate(SHOWN);
    let Some(first) = offers.first() else {
        return Ok(None);
    };
    let mut best = format!("{} {pay}", fmt(first.unit));
    if pay != "divine"
        && let Some(d) = crate::ninja::per_divine(game, league, pay, ua).await.filter(|d| *d > 0.0)
    {
        best.push_str(&format!(" · {} divine", fmt(first.unit / d)));
    }
    Ok(Some(Exchange {
        best,
        offers,
        total: v["total"].as_u64().unwrap_or(0),
        url: web_url(game, league, search_id),
    }))
}

fn offer(o: &Value) -> Option<Offer> {
    let paid = o["exchange"]["amount"].as_f64()?;
    let got = o["item"]["amount"].as_f64().filter(|g| *g > 0.0)?;
    let pay = o["exchange"]["currency"].as_str()?;
    Some(Offer {
        ratio: format!("{} {pay} for {}", fmt_amount(paid), fmt_amount(got)),
        stock: o["item"]["stock"].as_u64().unwrap_or(0),
        unit: paid / got,
    })
}

fn fmt_amount(v: f64) -> String {
    if v.fract() == 0.0 { format!("{v:.0}") } else { format!("{v}") }
}

fn web_url(game: Game, league: &str, id: &str) -> String {
    let seg = match game {
        Game::Poe1 => "trade",
        Game::Poe2 => "trade2",
    };
    format!("https://www.pathofexile.com/{seg}/exchange/{}{}/{id}", game.realm_path(), urlencode(league))
}

#[cfg(test)]
mod tests {
    //! Run against `mock.rs` (always on in tests).
    use super::*;

    fn block<T>(f: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(f)
    }

    #[test]
    fn poe1_offers_cheapest_first_with_divine_value() {
        let ex = block(lookup(Game::Poe1, "Standard", "online", "Exalted Orb", "test")).unwrap().unwrap();
        let units: Vec<f64> = ex.offers.iter().map(|o| o.unit).collect();
        assert_eq!(units, vec![14.0, 15.0, 15.5, 16.0]);
        assert_eq!(ex.offers[0].ratio, "70 chaos for 5");
        assert_eq!(ex.offers[0].stock, 40);
        assert_eq!(ex.best, "14.0 chaos · 0.08 divine");
        assert_eq!(ex.url, "https://www.pathofexile.com/trade/exchange/Standard/mockex");
    }

    #[test]
    fn poe2_prices_in_exalted_and_divine_in_divine_itself() {
        let ex = block(lookup(Game::Poe2, "Mock League", "securable", "Chaos Orb", "test")).unwrap().unwrap();
        assert_eq!(ex.best, "14.0 exalted · 0.04 divine", "400 exalted per divine in the mock");
        assert_eq!(ex.url, "https://www.pathofexile.com/trade2/exchange/poe2/Mock%20League/mockex");
        // The paying currency itself is priced in divine.
        let ex = block(lookup(Game::Poe2, "Standard", "online", "Exalted Orb", "test")).unwrap().unwrap();
        assert!(ex.best.ends_with(" divine") && !ex.best.contains('·'), "{}", ex.best);
    }

    #[test]
    fn items_not_on_the_exchange_have_no_offers() {
        assert_eq!(block(lookup(Game::Poe1, "Standard", "online", "Tabula Rasa", "test")).unwrap(), None);
    }
}
