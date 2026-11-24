// Release builds on Windows are GUI apps without a console window; debug builds keep the
// console for log output.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod bridge;
mod config;
mod devcache;
mod mock;
mod item;
mod links;
mod exchange;
mod mapcheck;
mod ninja;
mod platform;
mod pseudo;
mod ratelimit;
mod trade;
mod update;

use bridge::{apply_config, q, ui};
use config::{Config, Game};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Event {
    PriceCheck,
    /// Re-run a search with a spec edited in the overlay (JSON).
    Search(String),
    ToggleGame,
    SetGame(Game),
    OpenSettings,
    /// Settings window on the About page (update notice).
    OpenAbout,
    /// Tray "Check for updates…": About page plus a check.
    CheckUpdates,
    OpenManual,
    /// Price check pasted item text (manual check window).
    CheckText(String),
    HideOverlay,
    /// Open the hovered item's wiki/poedb page.
    OpenItemInfo,
    /// Mark a map mod template "deadly", "annoying" or "" (clear), from the overlay.
    MarkMapMod(String, String),
    Quit,
}

pub static CONFIG: LazyLock<Mutex<Config>> = LazyLock::new(|| {
    let mut c = Config::load();
    // Dev aid: LOOTINGGLASS_GAME=poe1|poe2 overrides the game for this run (e.g. for screenshots).
    match std::env::var("LOOTINGGLASS_GAME").as_deref() {
        Ok("poe1") => c.game = Game::Poe1,
        Ok("poe2") => c.game = Game::Poe2,
        _ => {}
    }
    Mutex::new(c)
});
static EVENTS: OnceLock<async_channel::Sender<Event>> = OnceLock::new();
static CHECK_GEN: AtomicU64 = AtomicU64::new(0);
/// The running price search and bulk exchange lookup. A new check or re-search cancels the old
/// ones, so requests whose results would be thrown away don't spend the trade site's rate limit
/// (or wait for it).
static SEARCH_TASK: Mutex<Option<tokio::task::AbortHandle>> = Mutex::new(None);
static EXCHANGE_TASK: Mutex<Option<tokio::task::AbortHandle>> = Mutex::new(None);

/// Puts `task` in `slot` (None to just empty it) and cancels the task it replaces.
fn replace_task(slot: &Mutex<Option<tokio::task::AbortHandle>>, task: Option<tokio::task::AbortHandle>) {
    if let Some(old) = std::mem::replace(&mut *slot.lock().unwrap(), task) {
        old.abort();
    }
}

/// Item text of the map last shown by the map check (re-coloured when a mod is marked).
static LAST_MAP: Mutex<Option<String>> = Mutex::new(None);

pub fn config() -> Config {
    CONFIG.lock().unwrap().clone()
}

pub fn rt() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

pub fn send_event(ev: Event) {
    if let Some(tx) = EVENTS.get() {
        let _ = tx.try_send(ev);
    }
}

/// Called once the QML Backend exists: wire up portals, tray and the event loop.
pub fn start_background() {
    let (tx, rx) = async_channel::unbounded();
    if EVENTS.set(tx.clone()).is_err() {
        return;
    }
    ratelimit::on_wait(|secs| {
        let secs = i32::try_from(secs).unwrap_or(i32::MAX);
        ui(move |b| b.set_rate_wait(secs));
    });
    ratelimit::on_warning(|text| ui(move |b| b.set_rate_warning(q(text))));
    rt().spawn(async move {
        platform::init().await;
        platform::spawn_tray(config().game).await;
        tokio::spawn(update::run());
        tokio::spawn(async move {
            if let Err(e) = platform::run_shortcuts(tx).await {
                log::error!("{e:#}");
                show_error(format!("Global shortcuts unavailable: {e:#}"));
            }
        });
        // Dev aid: LOOTINGGLASS_DEMO=item.txt price-checks that file's text once at startup.
        if let Some(path) = std::env::var_os("LOOTINGGLASS_DEMO") {
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                    tokio::spawn(check_text(text));
                }
                Err(e) => log::error!("LOOTINGGLASS_DEMO: {e}"),
            }
        }
        while let Ok(ev) = rx.recv().await {
            tokio::spawn(handle(ev));
        }
    });
}

fn show_error(msg: String) {
    ui(move |mut b| {
        b.as_mut().set_overlay_state(q("error"));
        b.as_mut().set_message(q(msg));
        b.as_mut().set_listings(q("[]"));
        b.as_mut().set_trade_url(q(""));
        b.as_mut().set_overlay_visible(true);
    });
}

async fn handle(ev: Event) {
    match ev {
        Event::PriceCheck => price_check().await,
        Event::Search(json) => match serde_json::from_str::<trade::Spec>(&json) {
            Ok(spec) => run_search(spec).await,
            Err(e) => log::error!("bad spec from overlay: {e}"),
        },
        Event::ToggleGame => set_game(config().game.other()).await,
        Event::SetGame(g) => set_game(g).await,
        Event::OpenSettings => ui(|b| b.open_settings()),
        Event::OpenAbout => ui(|b| b.open_about()),
        // Tray "Check for updates…": answer with a notification; the About page (where updates
        // are installed) opens on a click, or right away where there are no notifications.
        Event::CheckUpdates => {
            let v = env!("CARGO_PKG_VERSION");
            let shown = match update::check().await {
                update::Checked::UpToDate => {
                    platform::notify("Looting Glass is up to date", &format!("You have the latest version, {v}."), None).await
                }
                update::Checked::NotReady(new) => {
                    platform::notify(
                        &format!("Looting Glass {new} is on its way"),
                        &format!("You have {v}. Its downloads aren't ready yet; try again in a few minutes."),
                        None,
                    )
                    .await
                }
                update::Checked::Available(new) => {
                    platform::notify(
                        &format!("Looting Glass {new} is available"),
                        &format!("You have {v}. Click to see what's new and install it."),
                        Some(Event::OpenAbout),
                    )
                    .await
                }
                update::Checked::Failed(e) => {
                    platform::notify("Couldn't check for updates", &e, Some(Event::OpenAbout)).await
                }
            };
            if !shown {
                ui(|b| b.open_about());
            }
        }
        Event::OpenManual => ui(|b| b.open_manual()),
        Event::CheckText(text) => check_text(text).await,
        Event::HideOverlay => {
            ui(|b| b.set_overlay_visible(false));
            platform::focus_game().await;
        }
        Event::OpenItemInfo => open_item_info().await,
        Event::MarkMapMod(template, level) => mark_map_mod(&template, &level),
        Event::Quit => std::process::exit(0),
    }
}

async fn set_game(g: Game) {
    let cfg = {
        let mut c = CONFIG.lock().unwrap();
        c.game = g;
        if let Err(e) = c.save() {
            log::error!("saving config: {e}");
        }
        c.clone()
    };
    apply_config(&cfg);
    platform::set_tray_game(g).await;
}

async fn price_check() {
    ui(|b| b.set_overlay_visible(false));
    // Let focus return to the game if the overlay had it.
    tokio::time::sleep(Duration::from_millis(80)).await;

    let text = match platform::copy_item().await {
        Ok(t) => t,
        Err(e) => return show_error(format!("{e:#}")),
    };
    check_text(text).await;
}

/// Hotkey: copy the hovered item and open its page on the configured site, without a price check.
async fn open_item_info() {
    let text = match platform::copy_item().await {
        Ok(t) => t,
        Err(e) => return show_error(format!("{e:#}")),
    };
    let Some(item) = item::parse(&text) else {
        return show_error("The clipboard does not contain a Path of Exile item.".into());
    };
    let url = q(item_info_url(&item, &config()).await);
    ui(move |b| b.open_url(url));
}

/// A price check on a PoE 1 map also shows the map check (its mods, coloured by the player's
/// marks) when that's turned on.
fn wants_map_check(item: &item::Item, cfg: &Config) -> bool {
    item.is_map() && cfg.game == Game::Poe1 && cfg.map_check.enabled
}

fn map_view(item: &item::Item) -> cxx_qt_lib::QString {
    q(serde_json::to_string(&mapcheck::view(item, &config().map_check)).unwrap_or_default())
}

fn mark_map_mod(template: &str, level: &str) {
    {
        let mut c = CONFIG.lock().unwrap();
        mapcheck::mark(&mut c.map_check, template, level);
        if let Err(e) = c.save() {
            log::error!("saving config: {e}");
        }
    }
    let text = LAST_MAP.lock().unwrap().clone();
    if let Some(item) = text.as_deref().and_then(item::parse) {
        let view = map_view(&item);
        ui(move |b| b.set_map_info(view));
    }
}

/// Wiki/poedb page for an item; magic items are resolved to their base type first.
async fn item_info_url(item: &item::Item, cfg: &Config) -> String {
    let base = trade::base_type(item, cfg).await.unwrap_or_else(|| item.base.clone());
    links::url(cfg.game, cfg.links.site, links::page_name(item, &base))
}

/// Parse item text, show the overlay with default filters and run the first search.
async fn check_text(text: String) {
    // Keep the last copied item for troubleshooting parsing/matching problems.
    if let Some(dir) = dirs::state_dir().or_else(dirs::data_local_dir).map(|d| d.join("lootingglass")) {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("last-item.txt"), &text);
    }
    let Some(item) = item::parse(&text) else {
        return show_error("The clipboard does not contain a Path of Exile item.".into());
    };
    // A new item: the previous one's search and exchange lookup are no longer wanted.
    replace_task(&SEARCH_TASK, None);
    replace_task(&EXCHANGE_TASK, None);
    ratelimit::refresh_warning();
    let cfg = config();
    let title = q(item.display_name());
    // PoE 1 maps: the map check (marked mods) shows above the usual price check.
    let map = if wants_map_check(&item, &cfg) {
        *LAST_MAP.lock().unwrap() = Some(text.clone());
        map_view(&item)
    } else {
        *LAST_MAP.lock().unwrap() = None;
        q("")
    };
    use item::Rarity::*;
    if matches!(item.rarity, Currency | DivinationCard | Normal | Gem | Other) {
        let (game, league, ua) = (cfg.game, cfg.league().to_string(), cfg.user_agent());
        let (name, stack) = (item.base.clone(), item.stack_size);
        let (league2, name2, ua2, status) = (league.clone(), name.clone(), ua.clone(), cfg.trade.status.clone());
        tokio::spawn(async move {
            if let Some((main, detail)) = ninja::lookup(game, &league, &name, stack, &ua).await {
                ui(move |mut b| {
                    b.as_mut().set_ninja(q(main));
                    b.as_mut().set_ninja_detail(q(detail));
                });
            }
        });
        // The trade site's bulk exchange: what people ask right now, next to the poe.ninja average.
        let task = tokio::spawn(async move {
            match exchange::lookup(game, &league2, &status, &name2, &ua2).await {
                Ok(Some(ex)) => {
                    let json = q(serde_json::to_string(&ex).unwrap_or_default());
                    ui(move |b| b.set_exchange(json));
                }
                Ok(None) => {}
                Err(e) => log::warn!("bulk exchange: {e:#}"),
            }
        });
        replace_task(&EXCHANGE_TASK, Some(task.abort_handle()));
    }
    ui(move |mut b| {
        b.as_mut().set_title(title);
        b.as_mut().set_ninja(q(""));
        b.as_mut().set_ninja_detail(q(""));
        b.as_mut().set_exchange(q(""));
        b.as_mut().set_map_info(map);
        b.as_mut().set_spec(q(""));
        b.as_mut().set_overlay_state(q("loading"));
        b.as_mut().set_listings(q("[]"));
        b.as_mut().set_trade_url(q(""));
        b.as_mut().set_info_url(q(""));
        b.as_mut().set_overlay_visible(true);
    });
    let info = q(item_info_url(&item, &cfg).await);
    ui(move |b| b.set_info_url(info));
    let spec = match trade::prepare(&item, &cfg).await {
        Ok(s) => s,
        Err(e) => return show_error(format!("{e:#}")),
    };
    let json = q(serde_json::to_string(&spec).unwrap_or_default());
    ui(move |b| b.set_spec(json));
    run_search(spec).await;
}

async fn run_search(spec: trade::Spec) {
    let generation = CHECK_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    ui(|b| b.set_overlay_state(q("loading")));
    let cfg = config();
    let task = tokio::spawn(async move { trade::search(&spec, &cfg).await });
    replace_task(&SEARCH_TASK, Some(task.abort_handle()));
    let Ok(res) = task.await else {
        return; // cancelled: a newer search or check replaced it
    };
    if CHECK_GEN.load(Ordering::SeqCst) != generation {
        return; // a newer search superseded this one
    }
    match res {
        Ok(r) => {
            let listings = q(serde_json::to_string(&r.listings).unwrap_or_default());
            let msg = q(format!("Matched: {}", r.total));
            let url = q(r.url);
            ui(move |mut b| {
                b.as_mut().set_listings(listings);
                b.as_mut().set_message(msg);
                b.as_mut().set_trade_url(url);
                b.as_mut().set_overlay_state(q("result"));
            });
        }
        Err(e) => {
            log::warn!("search failed: {e:#}");
            let msg = q(format!("{e:#}"));
            ui(move |mut b| {
                b.as_mut().set_message(msg);
                b.as_mut().set_overlay_state(q("searchError"));
            });
        }
    }
}

unsafe extern "C" {
    fn lootingglass_qt_main() -> i32;
}

/// Holds an exclusive lock on `instance.lock` for the life of the process, so a second copy
/// (autostart plus a manual start, say) exits instead of fighting the first for the hotkey.
/// The OS drops the lock when the process ends, crashes included. Returns false if another
/// copy holds it. Dev runs (`LOOTINGGLASS_DEMO`/`LOOTINGGLASS_SNAPSHOT`) skip the check.
fn single_instance() -> bool {
    if ["LOOTINGGLASS_DEMO", "LOOTINGGLASS_SNAPSHOT"].iter().any(|v| std::env::var_os(v).is_some()) {
        return true;
    }
    let Some(dir) = dirs::state_dir().or_else(dirs::data_local_dir).map(|d| d.join("lootingglass")) else {
        return true;
    };
    let _ = std::fs::create_dir_all(&dir);
    let file = match std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(dir.join("instance.lock")) {
        Ok(f) => f,
        Err(e) => {
            log::warn!("instance lock: {e}");
            return true; // can't tell; don't block start-up
        }
    };
    // After an update the old copy starts the new one and exits; give it a moment to let go.
    let tries = if std::env::var_os("LOOTINGGLASS_RESTARTED").is_some() { 50 } else { 1 };
    for i in 0..tries {
        match file.try_lock() {
            Ok(()) => {
                std::mem::forget(file); // keep the lock until exit
                return true;
            }
            Err(std::fs::TryLockError::WouldBlock) if i + 1 < tries => {
                std::thread::sleep(Duration::from_millis(200))
            }
            Err(std::fs::TryLockError::WouldBlock) => return false,
            Err(std::fs::TryLockError::Error(e)) => {
                log::warn!("instance lock: {e}");
                return true;
            }
        }
    }
    false
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    if !single_instance() {
        log::info!("Looting Glass is already running; exiting.");
        return;
    }
    cxx_qt::init_crate!(cxx_qt_lib);
    cxx_qt::init_qml_module!("dev.lootingglass");
    let code = unsafe { lootingglass_qt_main() };
    std::process::exit(code);
}
