// Release builds on Windows are GUI apps without a console window; debug builds keep the
// console for log output.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod bridge;
mod config;
mod devcache;
mod mock;
mod item;
mod ninja;
mod platform;
mod trade;

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
    OpenManual,
    /// Price check pasted item text (manual check window).
    CheckText(String),
    HideOverlay,
    Quit,
}

pub static CONFIG: LazyLock<Mutex<Config>> = LazyLock::new(|| {
    let mut c = Config::load();
    // Dev aid: LOOTLENS_GAME=poe1|poe2 overrides the game for this run (e.g. for screenshots).
    match std::env::var("LOOTLENS_GAME").as_deref() {
        Ok("poe1") => c.game = Game::Poe1,
        Ok("poe2") => c.game = Game::Poe2,
        _ => {}
    }
    Mutex::new(c)
});
static EVENTS: OnceLock<async_channel::Sender<Event>> = OnceLock::new();
static CHECK_GEN: AtomicU64 = AtomicU64::new(0);

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
    rt().spawn(async move {
        platform::init().await;
        platform::spawn_tray(config().game).await;
        tokio::spawn(async move {
            if let Err(e) = platform::run_shortcuts(tx).await {
                log::error!("{e:#}");
                show_error(format!("Global shortcuts unavailable: {e:#}"));
            }
        });
        // Dev aid: LOOTLENS_DEMO=item.txt price-checks that file's text once at startup.
        if let Some(path) = std::env::var_os("LOOTLENS_DEMO") {
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                    tokio::spawn(check_text(text));
                }
                Err(e) => log::error!("LOOTLENS_DEMO: {e}"),
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
        Event::OpenManual => ui(|b| b.open_manual()),
        Event::CheckText(text) => check_text(text).await,
        Event::HideOverlay => {
            ui(|b| b.set_overlay_visible(false));
            platform::focus_game().await;
        }
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

/// Parse item text, show the overlay with default filters and run the first search.
async fn check_text(text: String) {
    // Keep the last copied item for troubleshooting parsing/matching problems.
    if let Some(dir) = dirs::state_dir().or_else(dirs::data_local_dir).map(|d| d.join("lootlens")) {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("last-item.txt"), &text);
    }
    let Some(item) = item::parse(&text) else {
        return show_error("The clipboard does not contain a Path of Exile item.".into());
    };
    let cfg = config();
    let title = q(item.display_name());
    use item::Rarity::*;
    if matches!(item.rarity, Currency | DivinationCard | Normal | Gem | Other) {
        let (game, league, ua) = (cfg.game, cfg.league().to_string(), cfg.user_agent());
        let (name, stack) = (item.base.clone(), item.stack_size);
        tokio::spawn(async move {
            if let Some((main, detail)) = ninja::lookup(game, &league, &name, stack, &ua).await {
                ui(move |mut b| {
                    b.as_mut().set_ninja(q(main));
                    b.as_mut().set_ninja_detail(q(detail));
                });
            }
        });
    }
    ui(move |mut b| {
        b.as_mut().set_title(title);
        b.as_mut().set_ninja(q(""));
        b.as_mut().set_ninja_detail(q(""));
        b.as_mut().set_spec(q(""));
        b.as_mut().set_overlay_state(q("loading"));
        b.as_mut().set_listings(q("[]"));
        b.as_mut().set_trade_url(q(""));
        b.as_mut().set_overlay_visible(true);
    });
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
    let res = trade::search(&spec, &config()).await;
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
            let msg = q(format!("{e:#}"));
            ui(move |mut b| {
                b.as_mut().set_message(msg);
                b.as_mut().set_overlay_state(q("searchError"));
            });
        }
    }
}

unsafe extern "C" {
    fn lootlens_qt_main() -> i32;
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    cxx_qt::init_crate!(cxx_qt_lib);
    cxx_qt::init_qml_module!("dev.lootlens");
    let code = unsafe { lootlens_qt_main() };
    std::process::exit(code);
}
