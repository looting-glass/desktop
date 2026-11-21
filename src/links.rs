//! Links to an item's page on poedb or the community wiki, per game.

use crate::config::{Game, urlencode};
use crate::item::{Item, Rarity};
use serde::{Deserialize, Serialize};

/// Where "open item info" goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InfoSite {
    /// poedb.tw (PoE 1) / poe2db.tw (PoE 2): mod pools and base data, the better default.
    #[default]
    Poedb,
    /// poewiki.net (PoE 1) / poe2wiki.net (PoE 2)
    Wiki,
}

impl InfoSite {
    /// Button text in the overlay.
    pub fn label(self) -> &'static str {
        match self {
            InfoSite::Wiki => "Wiki",
            InfoSite::Poedb => "poedb",
        }
    }
}

/// The page title for an item: uniques by name, everything else by base type. `base` is the
/// resolved base type (magic items carry their affixes in `item.base`).
pub fn page_name<'a>(item: &'a Item, base: &'a str) -> &'a str {
    match (&item.name, item.rarity) {
        (Some(name), Rarity::Unique) => name,
        _ => base,
    }
}

/// URL of the item's page. Wiki titles keep punctuation ("Kaom%27s_Heart"); poedb drops
/// apostrophes and commas ("Kaoms_Heart"). Both use underscores for spaces.
pub fn url(game: Game, site: InfoSite, name: &str) -> String {
    match site {
        InfoSite::Wiki => {
            let host = match game {
                Game::Poe1 => "www.poewiki.net",
                Game::Poe2 => "www.poe2wiki.net",
            };
            format!("https://{host}/wiki/{}", urlencode(&name.replace(' ', "_")))
        }
        InfoSite::Poedb => {
            let host = match game {
                Game::Poe1 => "poedb.tw",
                Game::Poe2 => "poe2db.tw",
            };
            let slug: String = name.replace(' ', "_").chars().filter(|c| !matches!(c, '\'' | ',')).collect();
            format!("https://{host}/us/{}", urlencode(&slug))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::parse;

    #[test]
    fn wiki_and_poedb_per_game() {
        assert_eq!(url(Game::Poe1, InfoSite::Wiki, "Kaom's Heart"), "https://www.poewiki.net/wiki/Kaom%27s_Heart");
        assert_eq!(url(Game::Poe2, InfoSite::Wiki, "Dragonscale Boots"), "https://www.poe2wiki.net/wiki/Dragonscale_Boots");
        assert_eq!(url(Game::Poe1, InfoSite::Poedb, "Kaom's Heart"), "https://poedb.tw/us/Kaoms_Heart");
        assert_eq!(url(Game::Poe2, InfoSite::Poedb, "Rattling Sceptre"), "https://poe2db.tw/us/Rattling_Sceptre");
        assert_eq!(url(Game::Poe1, InfoSite::Poedb, "Self-Flagellation"), "https://poedb.tw/us/Self-Flagellation");
        assert_eq!(url(Game::Poe1, InfoSite::Poedb, "Mjölner"), "https://poedb.tw/us/Mj%C3%B6lner");
    }

    #[test]
    fn poedb_is_the_default() {
        assert_eq!(InfoSite::default(), InfoSite::Poedb);
        assert_eq!(crate::config::Config::default().links.site.label(), "poedb");
    }

    #[test]
    fn uniques_open_their_name_others_their_base() {
        let unique = parse("Item Class: Body Armours\nRarity: Unique\nKaom's Heart\nGlorious Plate\n--------\n").unwrap();
        assert_eq!(page_name(&unique, &unique.base), "Kaom's Heart");
        let rare = parse(include_str!("../dev/boots.txt")).unwrap();
        assert_eq!(page_name(&rare, &rare.base), "Slink Boots");
        let unid = parse("Item Class: Rings\nRarity: Unique\nRuby Ring\n--------\nUnidentified\n").unwrap();
        assert_eq!(page_name(&unid, &unid.base), "Ruby Ring", "unidentified uniques have no name yet");
    }
}
