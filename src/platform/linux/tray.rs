//! StatusNotifierItem tray icon (native KDE system tray, pure D-Bus).

use crate::config::Game;
use crate::{Event, send_event};
use ksni::{
    MenuItem, TrayMethods,
    menu::{RadioGroup, RadioItem, StandardItem},
};
use tokio::sync::OnceCell;

const ICONS: [&[u8]; 5] = [
    include_bytes!("../../../data/icons/png/16.png"),
    include_bytes!("../../../data/icons/png/22.png"),
    include_bytes!("../../../data/icons/png/32.png"),
    include_bytes!("../../../data/icons/png/48.png"),
    include_bytes!("../../../data/icons/png/64.png"),
];

/// PNG (RGBA) -> SNI pixmap (ARGB32, network byte order).
fn decode(bytes: &[u8]) -> Option<ksni::Icon> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba {
        return None;
    }
    let data = buf[..info.buffer_size()]
        .chunks_exact(4)
        .flat_map(|p| [p[3], p[0], p[1], p[2]])
        .collect();
    Some(ksni::Icon { width: info.width as i32, height: info.height as i32, data })
}

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
    /// The icon is sent as pixels rather than a theme name: Plasma's panel resolves names
    /// against an icon theme cached at login and fails for freshly installed icons.
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        ICONS.iter().filter_map(|png| decode(png)).collect()
    }
    fn activate(&mut self, _x: i32, _y: i32) {
        send_event(Event::OpenManual);
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
                label: "Manual check…".into(),
                icon_name: "edit-paste".into(),
                activate: Box::new(|_: &mut Self| send_event(Event::OpenManual)),
                ..Default::default()
            }
            .into(),
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
