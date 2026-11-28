//! `lootingglass trigger <id>`: a key bound in the window manager's config runs that command,
//! which hands the shortcut id to the running instance over a Unix socket next to its
//! single-instance lock. The message is the id and a newline.

use crate::Event;
use crate::platform::BindHelp;
use anyhow::{Context, Result};
use std::convert::Infallible;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

/// Longer than any shortcut id; a client sending more is cut off.
const MAX_MESSAGE: u64 = 64;
/// How long a client gets to send its id; `trigger` sends it right after connecting.
const READ_TIMEOUT: Duration = Duration::from_secs(1);
/// Pause after a failed accept, such as running out of file descriptors, before trying again.
const ACCEPT_RETRY: Duration = Duration::from_millis(200);

fn socket_path() -> Result<PathBuf> {
    Ok(crate::instance_dir().context("no state directory for the trigger socket")?.join("trigger.sock"))
}

/// Take shortcut ids from `trigger` commands and forward their events for as long as the app
/// runs.
pub async fn run(tx: async_channel::Sender<Event>) -> Result<()> {
    let path = socket_path()?;
    let Some(listener) = listen(&path).await? else {
        // Only a dev run (`LOOTINGGLASS_DEMO`) gets here: it skips the single-instance lock and
        // leaves the commands to the copy in the tray.
        log::info!("another Looting Glass takes `lootingglass trigger` commands; this one won't");
        return Ok(());
    };
    log::info!("bind keys to `lootingglass trigger <id>` in your window manager; listening on {}", path.display());
    match serve(listener, tx).await {}
}

/// Bind the socket, replacing one left by a copy that crashed. A socket that still answers
/// belongs to a copy that's running, which keeps it: then there's nothing to bind.
async fn listen(path: &Path) -> Result<Option<UnixListener>> {
    if UnixStream::connect(path).await.is_ok() {
        return Ok(None);
    }
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await.with_context(|| format!("Couldn't create {}", dir.display()))?;
    }
    let _ = tokio::fs::remove_file(path).await;
    let listener = UnixListener::bind(path).with_context(|| format!("Couldn't listen for shortcuts on {}", path.display()))?;
    Ok(Some(listener))
}

async fn serve(listener: UnixListener, tx: async_channel::Sender<Event>) -> Infallible {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(receive(stream, tx.clone()));
            }
            Err(e) => {
                log::warn!("trigger: {e}");
                tokio::time::sleep(ACCEPT_RETRY).await;
            }
        }
    }
}

/// Read one shortcut id from the client and pass on its event.
async fn receive(stream: UnixStream, tx: async_channel::Sender<Event>) {
    let mut line = String::new();
    let mut reader = BufReader::new(stream).take(MAX_MESSAGE);
    match tokio::time::timeout(READ_TIMEOUT, reader.read_line(&mut line)).await {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => return log::warn!("trigger: {e}"),
        Err(_) => return log::warn!("trigger: no shortcut id within {READ_TIMEOUT:?}"),
    }
    let id = line.trim_end();
    // Nothing sent: another copy's `listen` checking whether we're running.
    if id.is_empty() {
        return;
    }
    let Some(event) = super::event(id) else {
        return log::warn!("trigger: unknown shortcut {id:?}");
    };
    if let Err(e) = tx.try_send(event) {
        log::warn!("trigger: couldn't pass on {id}: {e}");
    }
}

/// The commands to bind keys to, one per shortcut.
pub fn bind_help() -> BindHelp {
    BindHelp {
        text: "Bind a key to each command you want in your window manager's config (Sway: bindsym, Umbriel: [keybinds]):".into(),
        snippet: super::snippet(|def| format!("lootingglass trigger {}", def.id)),
        desktop_editor: false,
    }
}

/// Send `id` to the running instance. Blocking.
pub fn send(id: &str) -> Result<()> {
    send_to(&socket_path()?, id)
}

fn send_to(socket: &Path, id: &str) -> Result<()> {
    let mut stream = std::os::unix::net::UnixStream::connect(socket).context(
        "Looting Glass isn't running, or takes its shortcuts from the desktop here instead of from `lootingglass trigger`",
    )?;
    stream.write_all(format!("{id}\n").as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A folder for one test's socket, under the system's temporary folder; not created yet.
    fn scratch_dir() -> PathBuf {
        static RUN: AtomicU32 = AtomicU32::new(0);
        std::env::temp_dir()
            .join(format!("lootingglass-trigger-{}-{}", std::process::id(), RUN.fetch_add(1, Ordering::Relaxed)))
    }

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(future)
    }

    /// Serves a socket in a fresh folder and hands `test` its path and the events.
    fn with_listener(test: impl AsyncFnOnce(PathBuf, async_channel::Receiver<Event>)) {
        let dir = scratch_dir();
        let path = dir.join("trigger.sock");
        block_on(async {
            let (tx, rx) = async_channel::bounded(4);
            tokio::spawn(serve(listen(&path).await.unwrap().unwrap(), tx));
            test(path.clone(), rx).await;
        });
        std::fs::remove_dir_all(dir).unwrap();
    }

    async fn write_raw(path: &Path, message: &'static [u8]) {
        let path = path.to_owned();
        tokio::task::spawn_blocking(move || std::os::unix::net::UnixStream::connect(path).unwrap().write_all(message))
            .await
            .unwrap()
            .unwrap();
    }

    #[test]
    fn trigger_reaches_the_running_instance() {
        with_listener(async |path, rx| {
            tokio::task::spawn_blocking(move || send_to(&path, "price-check")).await.unwrap().unwrap();
            assert!(matches!(rx.recv().await, Ok(Event::PriceCheck)));
        });
    }

    #[test]
    fn unknown_and_oversized_messages_are_dropped() {
        with_listener(async |path, rx| {
            write_raw(&path, b"pricecheck\n").await;
            write_raw(&path, &[b'x'; 200]).await;
            write_raw(&path, b"toggle-game\n").await;
            assert!(matches!(rx.recv().await, Ok(Event::ToggleGame)));
        });
    }

    #[test]
    fn a_running_copy_keeps_its_socket() {
        with_listener(async |path, _| {
            assert!(listen(&path).await.unwrap().is_none());
        });
    }

    #[test]
    fn a_crashed_copys_socket_is_replaced() {
        let dir = scratch_dir();
        let path = dir.join("trigger.sock");
        std::fs::create_dir_all(&dir).unwrap();
        drop(std::os::unix::net::UnixListener::bind(&path).unwrap());
        assert!(block_on(listen(&path)).unwrap().is_some());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn bind_help_lists_a_command_per_shortcut() {
        let snippet = bind_help().snippet;
        assert_eq!(snippet.lines().count(), super::super::SHORTCUT_DEFS.len());
        assert_eq!(snippet.lines().next(), Some("lootingglass trigger price-check   # Price check hovered item"));
    }

    #[test]
    fn nobody_listening() {
        let e = send_to(Path::new("/nonexistent/trigger.sock"), "price-check").unwrap_err();
        assert!(e.to_string().starts_with("Looting Glass isn't running"), "{e}");
    }
}
