//! What the desktop can do, probed once at start: the Wayland globals the compositor advertises,
//! the portals it offers and the compositor IPC it runs. `select` decides from these alone.

use anyhow::Result;
use ashpd::zbus::{Connection, Proxy, fdo::DBusProxy, names::BusName};
use std::collections::BTreeSet;
use wayland_client::{Dispatch, QueueHandle, protocol::wl_registry};

#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// `XDG_CURRENT_DESKTOP` ("KDE", "Hyprland", "GNOME"): a label for logs, never a reason to
    /// pick a provider.
    pub desktop: String,
    /// Interface names of the compositor's Wayland globals ("ext_data_control_manager_v1").
    pub wayland_globals: BTreeSet<String>,
    /// Version of the GlobalShortcuts portal, if the desktop offers one.
    pub global_shortcuts_portal: Option<u32>,
    /// Version of the RemoteDesktop portal, if the desktop offers one.
    pub remote_desktop_portal: Option<u32>,
    /// Hyprland's IPC (`HYPRLAND_INSTANCE_SIGNATURE`), which `hyprctl` talks to.
    pub hyprland_ipc: bool,
    /// KWin on the session bus (`org.kde.KWin`), which runs our focus script.
    pub kwin: bool,
    /// KDE's shortcut service on the session bus (`org.kde.kglobalaccel`).
    pub kglobalaccel: bool,
}

impl Facts {
    pub fn has_global(&self, interface: &str) -> bool {
        self.wayland_globals.contains(interface)
    }

    pub async fn probe() -> Facts {
        let wayland_globals = tokio::task::spawn_blocking(wayland_globals)
            .await
            .map_err(anyhow::Error::from)
            .and_then(|globals| globals)
            .unwrap_or_else(|e| {
                log::warn!("couldn't list the compositor's Wayland globals: {e:#}");
                BTreeSet::new()
            });
        let mut facts = Facts {
            desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
            wayland_globals,
            hyprland_ipc: std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some(),
            ..Facts::default()
        };
        match Connection::session().await {
            Ok(conn) => facts.probe_session_bus(&conn).await,
            Err(e) => log::warn!("no session bus, so no portals or KDE services: {e}"),
        }
        facts
    }

    async fn probe_session_bus(&mut self, conn: &Connection) {
        self.global_shortcuts_portal = portal_version(conn, "org.freedesktop.portal.GlobalShortcuts").await;
        self.remote_desktop_portal = portal_version(conn, "org.freedesktop.portal.RemoteDesktop").await;
        self.kwin = has_owner(conn, "org.kde.KWin").await;
        self.kglobalaccel = has_owner(conn, "org.kde.kglobalaccel").await;
    }
}

/// The `version` property of a portal interface; an error means the desktop doesn't offer it.
async fn portal_version(conn: &Connection, interface: &str) -> Option<u32> {
    let version = async {
        let portal = Proxy::new(conn, "org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop", interface).await?;
        portal.get_property::<u32>("version").await
    };
    version.await.map_err(|e| log::debug!("no {interface}: {e}")).ok()
}

async fn has_owner(conn: &Connection, name: &str) -> bool {
    let owned = async { anyhow::Ok(DBusProxy::new(conn).await?.name_has_owner(BusName::try_from(name)?).await?) };
    owned.await.map_err(|e| log::debug!("{name}: {e:#}")).unwrap_or(false)
}

/// Collects the interface names the registry announces.
#[derive(Default)]
struct Globals(BTreeSet<String>);

impl Dispatch<wl_registry::WlRegistry, ()> for Globals {
    fn event(
        globals: &mut Self,
        _: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &wayland_client::Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { interface, .. } = event {
            globals.0.insert(interface);
        }
    }
}

fn wayland_globals() -> Result<BTreeSet<String>> {
    let conn = wayland_client::Connection::connect_to_env()?;
    let mut queue = conn.new_event_queue();
    let _registry = conn.display().get_registry(&queue.handle(), ());
    let mut globals = Globals::default();
    queue.roundtrip(&mut globals)?;
    Ok(globals.0)
}
