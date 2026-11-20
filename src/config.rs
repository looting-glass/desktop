use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Game {
    #[default]
    Poe1,
    Poe2,
}

impl Game {
    pub const ALL: [Game; 2] = [Game::Poe1, Game::Poe2];

    pub fn label(self) -> &'static str {
        match self {
            Game::Poe1 => "Path of Exile",
            Game::Poe2 => "Path of Exile 2",
        }
    }
    pub fn short(self) -> &'static str {
        match self {
            Game::Poe1 => "PoE 1",
            Game::Poe2 => "PoE 2",
        }
    }
    pub fn other(self) -> Game {
        match self {
            Game::Poe1 => Game::Poe2,
            Game::Poe2 => Game::Poe1,
        }
    }
    /// Base URL of the trade API, e.g. `https://www.pathofexile.com/api/trade2`.
    pub fn api(self) -> &'static str {
        match self {
            Game::Poe1 => "https://www.pathofexile.com/api/trade",
            Game::Poe2 => "https://www.pathofexile.com/api/trade2",
        }
    }
    /// Path segment between the endpoint and the league (`poe2/` realm for PoE 2).
    pub fn realm_path(self) -> &'static str {
        match self {
            Game::Poe1 => "",
            Game::Poe2 => "poe2/",
        }
    }
    pub fn realm(self) -> &'static str {
        match self {
            Game::Poe1 => "pc",
            Game::Poe2 => "poe2",
        }
    }
    pub fn web_search(self, league: &str, id: &str) -> String {
        let seg = match self {
            Game::Poe1 => "trade",
            Game::Poe2 => "trade2",
        };
        format!(
            "https://www.pathofexile.com/{seg}/search/{}{}/{id}",
            self.realm_path(),
            urlencode(league)
        )
    }
}

pub fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GameConfig {
    pub league: String,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self { league: "Standard".into() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    #[default]
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Which game price checks are made against.
    pub game: Game,
    pub poe1: GameConfig,
    pub poe2: GameConfig,
    pub trade: TradeConfig,
    pub overlay: OverlayConfig,
    /// Optional contact info appended to the User-Agent (GGG asks API clients to identify themselves).
    pub contact: String,
    /// Global hotkeys, on platforms where Looting Glass registers them itself (Windows). On Linux
    /// they are bound in the desktop's shortcut settings through the GlobalShortcuts portal.
    #[cfg(windows)]
    pub shortcuts: ShortcutsConfig,
}

/// Hotkeys as "Ctrl+Shift+D"-style strings; empty = unassigned.
#[cfg(windows)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ShortcutsConfig {
    pub price_check: String,
    pub toggle_game: String,
    pub manual_check: String,
    pub settings: String,
    pub hide_overlay: String,
}

#[cfg(windows)]
impl Default for ShortcutsConfig {
    fn default() -> Self {
        Self {
            price_check: "Ctrl+D".into(),
            toggle_game: String::new(),
            manual_check: String::new(),
            settings: String::new(),
            hide_overlay: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TradeConfig {
    /// Listing status filter: "online", "any" (PoE 2 also supports "securable").
    pub status: String,
    /// Stat minimums are set to rolled value minus this percentage.
    pub stat_tolerance: u32,
    /// Number of listings to fetch and show.
    pub max_results: u32,
}

impl Default for TradeConfig {
    fn default() -> Self {
        Self { status: "online".into(), stat_tolerance: 10, max_results: 20 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OverlayConfig {
    pub side: Side,
    pub margin_top: i32,
    pub margin_side: i32,
    pub width: i32,
    /// Height cap in pixels; 0 = fit content up to the screen height.
    pub max_height: i32,
    /// Keep the overlay open when the game is clicked (close with ✕ or Esc only).
    pub pinned: bool,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self { side: Side::Right, margin_top: 80, margin_side: 24, width: 380, max_height: 0, pinned: false }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            game: Game::Poe1,
            poe1: GameConfig::default(),
            poe2: GameConfig::default(),
            trade: TradeConfig::default(),
            overlay: OverlayConfig::default(),
            contact: String::new(),
            #[cfg(windows)]
            shortcuts: ShortcutsConfig::default(),
        }
    }
}

impl Config {
    pub fn dir() -> PathBuf {
        dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("lootingglass")
    }
    fn path() -> PathBuf {
        Self::dir().join("config.toml")
    }

    pub fn load() -> Self {
        Self::migrate_from_old_names();
        match std::fs::read_to_string(Self::path()) {
            Ok(s) => toml::from_str(&s).unwrap_or_else(|e| {
                log::error!("invalid config, using defaults: {e}");
                Self::default()
            }),
            Err(_) => {
                let c = Self::default();
                let _ = c.save();
                c
            }
        }
    }

    /// One-time carry-over of settings from the app's previous names (Loot Lens, Glimmer).
    fn migrate_from_old_names() {
        if Self::path().exists() {
            return;
        }
        let Some(config) = dirs::config_dir() else { return };
        let old = ["lootlens", "glimmer"].map(|d| config.join(d).join("config.toml"));
        if let Some(old) = old.iter().find(|o| o.exists()) {
            let _ = std::fs::create_dir_all(Self::dir());
            if std::fs::copy(old, Self::path()).is_ok() {
                log::info!("migrated settings from {}", old.display());
            }
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(Self::dir())?;
        std::fs::write(Self::path(), toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn game_cfg(&self, g: Game) -> &GameConfig {
        match g {
            Game::Poe1 => &self.poe1,
            Game::Poe2 => &self.poe2,
        }
    }
    pub fn game_cfg_mut(&mut self, g: Game) -> &mut GameConfig {
        match g {
            Game::Poe1 => &mut self.poe1,
            Game::Poe2 => &mut self.poe2,
        }
    }
    pub fn league(&self) -> &str {
        &self.game_cfg(self.game).league
    }

    pub fn user_agent(&self) -> String {
        let v = env!("CARGO_PKG_VERSION");
        if self.contact.trim().is_empty() {
            format!("LootingGlass/{v}")
        } else {
            format!("LootingGlass/{v} (contact: {})", self.contact.trim())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_config_fills_in_defaults() {
        let c: Config = toml::from_str("game = \"poe2\"\n[poe2]\nleague = \"Dawn\"\n").unwrap();
        assert_eq!(c.game, Game::Poe2);
        assert_eq!(c.league(), "Dawn");
        assert_eq!(c.poe1.league, "Standard");
        assert_eq!(c.trade.stat_tolerance, 10);
        assert_eq!(c.overlay.width, 380);
    }

    #[test]
    fn config_round_trips_through_toml() {
        let mut c = Config::default();
        c.game = Game::Poe2;
        c.trade.max_results = 7;
        let back: Config = toml::from_str(&toml::to_string(&c).unwrap()).unwrap();
        assert_eq!(back.game, Game::Poe2);
        assert_eq!(back.trade.max_results, 7);
    }

    #[test]
    fn trade_site_urls() {
        assert_eq!(
            Game::Poe1.web_search("Standard", "abc"),
            "https://www.pathofexile.com/trade/search/Standard/abc"
        );
        assert_eq!(
            Game::Poe2.web_search("Rise of the Abyssal", "abc"),
            "https://www.pathofexile.com/trade2/search/poe2/Rise%20of%20the%20Abyssal/abc"
        );
    }
}
