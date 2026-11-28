//! `lootingglass trigger <id>`: a key bound in the window manager's config runs that command,
//! which hands the shortcut id to the running instance over a Unix socket next to its
//! single-instance lock. The message is the id and a newline.

use crate::Event;
use anyhow::{Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

/// Longer than any shortcut id; a client sending more is cut off.
const MAX_MESSAGE: u64 = 64;
/// How long a client gets to send its id; `trigger` sends it right after connecting.
const READ_TIMEOUT: Duration = Duration::from_secs(1);

fn socket_path() -> Result<PathBuf> {
    Ok(crate::instance_dir().context("no state directory for the trigger socket")?.join("trigger.sock"))
}

/// Take shortcut ids from `trigger` commands and forward their events.
pub async fn run(tx: async_channel::Sender<Event>) -> Result<()> {
    let path = socket_path()?;
    // We hold the single-instance lock, so a socket already there was left by a copy that crashed.
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).with_context(|| format!("Couldn't listen for shortcuts on {}", path.display()))?;
    log::info!("bind keys to `lootingglass trigger <id>` in your window manager; listening on {}", path.display());
    serve(listener, tx).await
}

async fn serve(listener: UnixListener, tx: async_channel::Sender<Event>) -> Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(receive(stream, tx.clone()));
    }
}

async fn receive(stream: UnixStream, tx: async_channel::Sender<Event>) {
    let mut id = String::new();
    let mut reader = BufReader::new(stream).take(MAX_MESSAGE);
    match tokio::time::timeout(READ_TIMEOUT, reader.read_line(&mut id)).await {
        Ok(Ok(_)) => match super::event(id.trim_end()) {
            Some(event) => {
                let _ = tx.send(event).await;
            }
            None => log::warn!("trigger: unknown shortcut {id:?}"),
        },
        Ok(Err(e)) => log::warn!("trigger: {e}"),
        Err(_) => log::warn!("trigger: no shortcut id within {READ_TIMEOUT:?}"),
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

    /// Serves a socket in a fresh temporary folder and hands `test` its path and the events.
    fn with_listener(test: impl AsyncFnOnce(PathBuf, async_channel::Receiver<Event>)) {
        static RUN: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir()
            .join(format!("lootingglass-trigger-{}-{}", std::process::id(), RUN.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("trigger.sock");
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
            let (tx, rx) = async_channel::bounded(4);
            tokio::spawn(serve(UnixListener::bind(&path).unwrap(), tx));
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
    fn nobody_listening() {
        let e = send_to(Path::new("/nonexistent/trigger.sock"), "price-check").unwrap_err();
        assert!(e.to_string().starts_with("Looting Glass isn't running"), "{e}");
    }
}
