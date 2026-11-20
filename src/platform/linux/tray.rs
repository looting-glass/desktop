//! StatusNotifierItem tray icon (native KDE system tray, pure D-Bus).

use crate::config::Game;
use crate::{Event, send_event};
use ksni::{
    MenuItem, TrayMethods,
    menu::{RadioGroup, RadioItem, StandardItem},
};
use tokio::sync::OnceCell;

pub struct Tray {
    pub game: Game,
}

static HANDLE: OnceCell<ksni::Handle<Tray>> = OnceCell::const_new();

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "lootlens".into()
    }
    fn title(&self) -> String {
        format!("Loot Lens — {}", self.game.label())
    }
    fn icon_name(&self) -> String {
        "view-financial-list".into()
    }
    fn activate(&mut self, _x: i32, _y: i32) {
        send_event(Event::OpenSettings);
    }
    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            RadioGroup {
                selected: self.game as usize,
                select: Box::new(|_: &mut Self, i| send_event(Event::SetGame(Game::ALL[i]))),
                options: Game::ALL
                    .iter()
                    .map(|g| RadioItem { label: g.label().into(), ..Default::default() })
                    .collect(),
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Settings…".into(),
                icon_name: "configure".into(),
                activate: Box::new(|_: &mut Self| send_event(Event::OpenSettings)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|_: &mut Self| send_event(Event::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub async fn spawn(game: Game) {
    match (Tray { game }).spawn().await {
        Ok(h) => {
            let _ = HANDLE.set(h);
        }
        Err(e) => log::warn!("tray unavailable: {e}"),
    }
}

pub async fn set_game(game: Game) {
    if let Some(h) = HANDLE.get() {
        h.update(|t| t.game = game).await;
    }
}
