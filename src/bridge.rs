//! Qt/QML bridge: a single `Backend` singleton the QML views bind to.
//! All logic runs on the tokio runtime; UI updates are queued onto the Qt thread.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(bool, overlay_visible)]
        #[qproperty(QString, overlay_state)]
        #[qproperty(QString, title)]
        #[qproperty(QString, subtitle)]
        #[qproperty(QString, message)]
        #[qproperty(QString, listings)]
        #[qproperty(QString, spec)]
        #[qproperty(QString, ninja)]
        #[qproperty(QString, ninja_detail)]
        #[qproperty(QString, exchange)]
        #[qproperty(QString, map_info)]
        #[qproperty(i32, overlay_max_height)]
        #[qproperty(QString, snapshot_dir)]
        #[qproperty(QString, demo_text)]
        #[qproperty(QString, trade_url)]
        #[qproperty(QString, info_url)]
        #[qproperty(QString, info_label)]
        #[qproperty(QString, game_label)]
        #[qproperty(QString, shortcuts_text)]
        #[qproperty(QString, leagues)]
        #[qproperty(i32, side)]
        #[qproperty(i32, overlay_width)]
        #[qproperty(i32, margin_top)]
        #[qproperty(i32, margin_side)]
        #[qproperty(bool, pinned)]
        #[qproperty(QString, version)]
        #[qproperty(QString, update_version)]
        #[qproperty(QString, update_url)]
        #[qproperty(QString, update_notes)]
        #[qproperty(QString, update_mode)]
        #[qproperty(QString, update_status)]
        #[qproperty(bool, update_busy)]
        #[qproperty(bool, update_ready)]
        #[qproperty(QString, last_checked)]
        type Backend = super::BackendRust;

        #[qinvokable]
        fn config_json(self: &Backend) -> QString;
        #[qinvokable]
        fn config_path(self: &Backend) -> QString;
        #[qinvokable]
        fn save_config(self: Pin<&mut Backend>, json: &QString) -> QString;
        #[qinvokable]
        fn configure_shortcuts(self: &Backend);
        #[qinvokable]
        fn refresh_shortcuts(self: &Backend);
        #[qinvokable]
        fn refresh_leagues(self: &Backend);
        #[qinvokable]
        fn hide_overlay(self: Pin<&mut Backend>);
        #[qinvokable]
        fn toggle_game(self: &Backend);
        #[qinvokable]
        fn search(self: &Backend, spec: &QString);
        #[qinvokable]
        fn check_text(self: &Backend, text: &QString) -> QString;
        #[qinvokable]
        fn mark_map_mod(self: &Backend, mod_template: &QString, level: &QString);
        #[qinvokable]
        fn price_check_map(self: &Backend);
        #[qinvokable]
        fn open_settings_window(self: Pin<&mut Backend>);
        #[qinvokable]
        fn save_overlay_size(self: &Backend);
        #[qinvokable]
        fn check_updates(self: &Backend);
        #[qinvokable]
        fn install_update(self: &Backend);
        #[qinvokable]
        fn restart_app(self: &Backend);

        #[qsignal]
        fn open_settings(self: Pin<&mut Backend>);
        #[qsignal]
        fn open_about(self: Pin<&mut Backend>);
        #[qsignal]
        fn open_manual(self: Pin<&mut Backend>);
        /// Open a link in the default browser (hotkey actions; QML calls Qt.openUrlExternally).
        #[qsignal]
        fn open_url(self: Pin<&mut Backend>, url: QString);
    }

    impl cxx_qt::Threading for Backend {}
    impl cxx_qt::Initialize for Backend {}
}

use crate::config::{Config, Game};
use crate::{Event, config, rt, send_event};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use std::pin::Pin;
use std::sync::{Mutex, OnceLock};

#[derive(Default)]
pub struct BackendRust {
    overlay_visible: bool,
    overlay_state: QString,
    title: QString,
    subtitle: QString,
    message: QString,
    listings: QString,
    spec: QString,
    ninja: QString,
    ninja_detail: QString,
    /// Bulk exchange offers as JSON (`exchange::Exchange`), or empty.
    exchange: QString,
    /// Map check view as JSON (`mapcheck::MapView`), or empty.
    map_info: QString,
    overlay_max_height: i32,
    snapshot_dir: QString,
    demo_text: QString,
    trade_url: QString,
    info_url: QString,
    info_label: QString,
    game_label: QString,
    shortcuts_text: QString,
    leagues: QString,
    side: i32,
    overlay_width: i32,
    margin_top: i32,
    margin_side: i32,
    pinned: bool,
    version: QString,
    update_version: QString,
    update_url: QString,
    update_notes: QString,
    update_mode: QString,
    update_status: QString,
    update_busy: bool,
    update_ready: bool,
    last_checked: QString,
}

type UiFn = Box<dyn FnOnce(Pin<&mut qobject::Backend>) + Send>;
static UI: OnceLock<Mutex<cxx_qt::CxxQtThread<qobject::Backend>>> = OnceLock::new();

/// Run `f` on the Qt thread with the Backend object.
pub fn ui(f: impl FnOnce(Pin<&mut qobject::Backend>) + Send + 'static) {
    if let Some(t) = UI.get() {
        let f: UiFn = Box::new(f);
        let _ = t.lock().unwrap().queue(move |b| f(b));
    }
}

pub fn q(s: impl AsRef<str>) -> QString {
    QString::from(s.as_ref())
}

/// Push config-derived values (game label, overlay geometry) to QML.
pub fn apply_config(cfg: &Config) {
    let label = q(format!("{} · {}", cfg.game.short(), cfg.league()));
    let o = cfg.overlay.clone();
    let info_label = q(cfg.links.site.label());
    ui(move |mut b| {
        b.as_mut().set_game_label(label);
        b.as_mut().set_info_label(info_label);
        b.as_mut().set_side(o.side as i32);
        b.as_mut().set_overlay_width(o.width);
        b.as_mut().set_overlay_max_height(o.max_height);
        b.as_mut().set_margin_top(o.margin_top);
        b.as_mut().set_margin_side(o.margin_side);
        b.as_mut().set_pinned(o.pinned);
    });
}

impl cxx_qt::Initialize for qobject::Backend {
    fn initialize(mut self: Pin<&mut Self>) {
        // Dev aid: LOOTINGGLASS_SNAPSHOT=<dir> makes QML save overlay/settings screenshots there.
        if let Ok(dir) = std::env::var("LOOTINGGLASS_SNAPSHOT") {
            self.as_mut().set_snapshot_dir(q(dir));
            // Sample text for the manual check window screenshot.
            if let Some(t) = std::env::var_os("LOOTINGGLASS_DEMO").and_then(|p| std::fs::read_to_string(p).ok()) {
                self.as_mut().set_demo_text(q(t));
            }
        }
        self.as_mut().set_version(q(env!("CARGO_PKG_VERSION")));
        let _ = UI.set(Mutex::new(self.qt_thread()));
        apply_config(&config());
        crate::start_background();
    }
}

impl qobject::Backend {
    fn config_json(&self) -> QString {
        q(serde_json::to_string(&config()).unwrap_or_default())
    }

    fn config_path(&self) -> QString {
        q(Config::dir().join("config.toml").display().to_string())
    }

    /// Returns an empty string on success, otherwise an error message.
    fn save_config(self: Pin<&mut Self>, json: &QString) -> QString {
        let res = serde_json::from_str::<Config>(&json.to_string())
            .map_err(anyhow::Error::from)
            .and_then(|c| {
                c.save()?;
                Ok(c)
            });
        match res {
            Ok(c) => {
                let game = c.game;
                *crate::CONFIG.lock().unwrap() = c;
                send_event(Event::SetGame(game));
                crate::platform::reload_shortcuts();
                QString::default()
            }
            Err(e) => q(e.to_string()),
        }
    }

    fn configure_shortcuts(&self) {
        rt().spawn(crate::platform::configure_shortcuts());
    }

    fn refresh_shortcuts(&self) {
        rt().spawn(async {
            let s = q(crate::platform::describe_shortcuts().await);
            ui(move |b| b.set_shortcuts_text(s));
        });
    }

    fn refresh_leagues(&self) {
        let ua = config().user_agent();
        rt().spawn(async move {
            let mut map = serde_json::Map::new();
            for g in Game::ALL {
                let key = serde_json::to_value(g).unwrap().as_str().unwrap().to_string();
                match crate::trade::leagues(g, &ua).await {
                    Ok(l) => {
                        map.insert(key, l.into());
                    }
                    Err(e) => log::warn!("leagues for {}: {e}", g.short()),
                }
            }
            let s = q(serde_json::Value::Object(map).to_string());
            ui(move |b| b.set_leagues(s));
        });
    }

    fn hide_overlay(self: Pin<&mut Self>) {
        self.set_overlay_visible(false);
        rt().spawn(crate::platform::focus_game());
    }

    fn toggle_game(&self) {
        send_event(Event::ToggleGame);
    }

    /// Persist the size/position chosen with the overlay's resize grip and drag handle.
    fn save_overlay_size(&self) {
        let mut c = crate::CONFIG.lock().unwrap();
        c.overlay.width = *self.overlay_width();
        c.overlay.max_height = *self.overlay_max_height();
        c.overlay.margin_top = *self.margin_top();
        c.overlay.margin_side = *self.margin_side();
        c.overlay.pinned = *self.pinned();
        if let Err(e) = c.save() {
            log::error!("saving config: {e}");
        }
    }

    /// Manual check: validate pasted text, then price check it like a hotkey copy.
    /// Returns an error message, or "" when the check was started.
    fn check_text(&self, text: &QString) -> QString {
        let text = text.to_string();
        if crate::item::parse(&text).is_none() {
            return q("That doesn't look like Path of Exile item text. Copy an item in game with Ctrl+C and paste it here.");
        }
        send_event(Event::CheckText(text));
        QString::default()
    }

    fn mark_map_mod(&self, mod_template: &QString, level: &QString) {
        send_event(Event::MarkMapMod(mod_template.to_string(), level.to_string()));
    }

    fn price_check_map(&self) {
        send_event(Event::PriceCheckMap);
    }

    fn open_settings_window(self: Pin<&mut Self>) {
        self.open_settings();
    }

    fn check_updates(&self) {
        rt().spawn(crate::update::check());
    }

    fn install_update(&self) {
        rt().spawn(crate::update::install());
    }

    fn restart_app(&self) {
        crate::update::restart();
    }

    fn search(&self, spec: &QString) {
        send_event(Event::Search(spec.to_string()));
    }
}

// Silence "unused" for the generated rust() accessor helper trait.
#[allow(unused)]
fn _assert(b: &qobject::Backend) -> &BackendRust {
    b.rust()
}
