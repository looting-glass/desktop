//! Update check against the latest GitHub release, and self-update for the installs the app
//! owns: the Windows installer and zip, and the Linux tarball installed with `install.sh` into
//! `~/.local/bin`. Package-managed installs (AUR, anything under `/usr`) and dev builds are only
//! told about new versions. Nothing is installed until the user asks for it on the About page.
//!
//! `LOOTINGGLASS_MOCK=1` answers the check from `mock.rs` without network; add
//! `LOOTINGGLASS_MOCK_UPDATE=1` to make it report a newer version (for UI work and snapshots).

use crate::bridge::{q, ui};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

const LATEST: &str = "https://api.github.com/repos/looting-glass/desktop/releases/latest";
const CURRENT: &str = env!("CARGO_PKG_VERSION");
/// Suffix for files a Windows zip update moved aside; deleted on the next start.
const OLD_SUFFIX: &str = ".update-old";

#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub html_url: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
}

/// `"v1.2.3"` / `"1.2.3"` → `(1, 2, 3)`. Anything else (pre-release suffixes included) is `None`.
pub fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let s = s.trim();
    let s = s.strip_prefix('v').unwrap_or(s);
    let mut it = s.split('.').map(|p| p.parse::<u32>().ok());
    let v = (it.next()??, it.next()??, it.next()??);
    it.next().is_none().then_some(v)
}

/// True when `latest` is a valid version newer than `current`.
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// What the latest release means for this copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offer {
    UpToDate,
    /// Newer, but its downloads aren't attached yet: published before its builds finished, or a
    /// platform's build failed. Not offered; a later check tries again.
    NotReady,
    Available,
}

/// Whether to offer `rel` to a copy running `current` that was installed as `kind`. A newer
/// release counts only once this copy can update from it: the file it updates from and its
/// checksum are attached (or, for installs that are only told about new versions, at least one
/// download is), so "Update" never fails on a missing file.
pub fn offer(rel: &Release, current: &str, kind: Install) -> Offer {
    if !is_newer(&rel.tag_name, current) {
        return Offer::UpToDate;
    }
    let has = |name: &str| rel.assets.iter().any(|a| a.name == name);
    let ready = match kind.asset(rel.tag_name.trim().trim_start_matches('v')) {
        Some(file) => has(&file) && has(&format!("{file}.sha256")),
        None => !rel.assets.is_empty(),
    };
    if ready { Offer::Available } else { Offer::NotReady }
}

/// How this copy was installed, which decides whether it can update itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Install {
    /// Windows, installed with the Inno Setup installer (`unins000.exe` next to the exe).
    WindowsInstaller,
    /// Windows, extracted from the release zip.
    WindowsZip,
    /// Linux, installed by the release tarball's `install.sh` into `~/.local/bin`.
    LinuxUser,
    /// Linux, installed by a package manager (AUR, anything under `/usr`): notify only.
    Package,
    /// Dev builds and unknown locations: notify only.
    Manual,
}

/// Path of the running executable, taken at start-up (an update renames or replaces the file).
static EXE: LazyLock<Option<PathBuf>> = LazyLock::new(|| std::env::current_exe().ok());

impl Install {
    pub fn detect() -> Install {
        if cfg!(debug_assertions) {
            return Install::Manual;
        }
        let Some(exe) = EXE.as_deref() else { return Install::Manual };
        if cfg!(windows) {
            let dir = exe.parent().unwrap_or(Path::new("."));
            windows_kind(dir.join("unins000.exe").exists(), dir.join("Qt6Core.dll").exists())
        } else {
            match dirs::home_dir() {
                Some(home) => linux_kind(exe, &home),
                None => Install::Manual,
            }
        }
    }

    /// Release asset this install updates from.
    fn asset(self, version: &str) -> Option<String> {
        match self {
            Install::WindowsInstaller => Some(format!("lootingglass-{version}-windows-x64-setup.exe")),
            Install::WindowsZip => Some(format!("lootingglass-{version}-windows-x64.zip")),
            Install::LinuxUser => Some(format!("lootingglass-{version}-x86_64.tar.gz")),
            Install::Package | Install::Manual => None,
        }
    }

    /// For QML: "install" (the About page offers to install), "package" or "manual".
    fn mode(self) -> &'static str {
        match self {
            _ if crate::mock::enabled() => "install",
            Install::Package => "package",
            Install::Manual => "manual",
            _ => "install",
        }
    }
}

fn windows_kind(has_uninstaller: bool, has_qt: bool) -> Install {
    match (has_uninstaller, has_qt) {
        (true, _) => Install::WindowsInstaller,
        (false, true) => Install::WindowsZip,
        _ => Install::Manual,
    }
}

fn linux_kind(exe: &Path, home: &Path) -> Install {
    if exe == home.join(".local/bin/lootingglass") {
        Install::LinuxUser
    } else if exe.starts_with("/usr") {
        Install::Package
    } else {
        Install::Manual
    }
}

/// Check that `data` matches a `.sha256` file (`<hex>  <name>`, any case).
pub fn verify_sha256(data: &[u8], sha_file: &str) -> Result<()> {
    let expected = sha_file.split_whitespace().next().context("empty checksum file")?.to_ascii_lowercase();
    let actual: String = Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect();
    if actual != expected {
        bail!("checksum mismatch (expected {expected}, got {actual})");
    }
    Ok(())
}

/// Release notes for the About page: release-please's headings become bold lines (Qt renders
/// Markdown headings huge) and its "[X.Y.Z](compare link) (date)" title is dropped.
pub fn notes(body: &str) -> String {
    body.lines()
        .filter_map(|l| {
            let Some(h) = l.strip_prefix('#') else { return Some(l.to_string()) };
            let h = h.trim_start_matches('#').trim();
            (!h.starts_with('[')).then(|| format!("**{h}**"))
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

static HTTP: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder().timeout(Duration::from_secs(120)).build().expect("http client")
});

async fn latest(ua: &str) -> Result<Release> {
    if crate::mock::enabled() {
        let v = crate::mock::response("GET", LATEST).context("no mock release")?;
        return Ok(serde_json::from_value(v)?);
    }
    let r = HTTP
        .get(LATEST)
        .header(reqwest::header::USER_AGENT, ua)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .timeout(Duration::from_secs(15))
        .send()
        .await?
        .error_for_status()?;
    Ok(r.json().await?)
}

/// Newer release found by the last check.
static FOUND: Mutex<Option<Release>> = Mutex::new(None);

/// What a check found, for whoever asked (the tray answers with a notification).
pub enum Checked {
    UpToDate,
    /// A newer version is out but its downloads aren't ready yet.
    NotReady(String),
    Available(String),
    Failed(String),
}

/// Left by an update before restarting: the version it replaced. The new version reads it on
/// start to say "updated", then deletes it.
fn updated_marker() -> Option<PathBuf> {
    dirs::state_dir().or_else(dirs::data_local_dir).map(|d| d.join("lootingglass/updated-from"))
}

/// After an update and restart: tell the user it worked.
async fn announce_update() {
    let Some(path) = updated_marker() else { return };
    let Ok(old) = std::fs::read_to_string(&path) else { return };
    let _ = std::fs::remove_file(&path);
    let old = old.trim();
    if old == CURRENT {
        return; // the update didn't take; the About page said so when it failed
    }
    log::info!("updated from {old} to {CURRENT}");
    status(format!("Updated from {old} to {CURRENT}."), false);
    crate::platform::notify(
        &format!("Looting Glass updated to {CURRENT}"),
        &format!("You were on {old}. Click to see the About page."),
        Some(crate::Event::OpenAbout),
    )
    .await;
}

fn status(msg: impl Into<String>, busy: bool) {
    let msg = q(msg.into());
    ui(move |mut b| {
        b.as_mut().set_update_status(msg);
        b.as_mut().set_update_busy(busy);
    });
}

/// Background task: clean up after a previous update, then check shortly after start-up and
/// once a day while `[updates] check` is on.
pub async fn run() {
    LazyLock::force(&EXE);
    if Install::detect() == Install::WindowsZip
        && let Some(dir) = EXE.as_deref().and_then(Path::parent)
    {
        remove_old_files(dir);
    }
    announce_update().await;
    let delay = if crate::mock::enabled() { 1 } else { 30 };
    tokio::time::sleep(Duration::from_secs(delay)).await;
    let mut announced: Option<String> = None;
    loop {
        let checked = if crate::config().updates.check { Some(check().await) } else { None };
        if let Some(Checked::Available(v)) = &checked
            && should_announce(v, announced.as_deref(), crate::config().updates.check)
        {
            crate::platform::notify(
                &format!("Looting Glass {v} is available"),
                "Click to see what's new and install it.",
                Some(crate::Event::OpenAbout),
            )
            .await;
            announced = Some(v.clone());
        }
        tokio::time::sleep(next_check(checked.as_ref())).await;
    }
}

/// Whether the background check notifies about `found`, the newer version it just found, given
/// the version it last notified about this run. Once per version, not every day; and not when
/// checks were turned off in Settings while this one was running.
fn should_announce(found: &str, announced: Option<&str>, checks_on: bool) -> bool {
    checks_on && announced != Some(found)
}

/// When to check again: daily, but soon while a new release's downloads are still being
/// attached (the builds take a few minutes).
fn next_check(last: Option<&Checked>) -> Duration {
    match last {
        Some(Checked::NotReady(_)) => Duration::from_secs(15 * 60),
        _ => Duration::from_secs(24 * 60 * 60),
    }
}

/// Ask GitHub for the latest release and publish the result to QML and the tray.
pub async fn check() -> Checked {
    status("Checking for updates…", true);
    let res = latest(&crate::config().user_agent()).await;
    let checked = q(chrono::Local::now().format("%Y-%m-%d %H:%M").to_string());
    ui(move |b| b.set_last_checked(checked));
    let rel = match res {
        Ok(r) => r,
        Err(e) => {
            log::warn!("update check: {e:#}");
            status(format!("Update check failed: {e:#}"), false);
            return Checked::Failed(format!("{e:#}"));
        }
    };
    let version = rel.tag_name.trim_start_matches('v').to_string();
    match offer(&rel, CURRENT, Install::detect()) {
        Offer::UpToDate => {
            log::info!("up to date ({CURRENT})");
            status("Looting Glass is up to date.", false);
            return Checked::UpToDate;
        }
        Offer::NotReady => {
            log::info!("{version} is out, but its downloads aren't attached yet");
            status(format!("Looting Glass {version} is on its way; its downloads aren't ready yet."), false);
            return Checked::NotReady(version);
        }
        Offer::Available => {}
    }
    log::info!("update available: {version}");
    let (v, url, notes, mode) =
        (q(&version), q(&rel.html_url), q(notes(&rel.body)), q(Install::detect().mode()));
    *FOUND.lock().unwrap() = Some(rel);
    ui(move |mut b| {
        b.as_mut().set_update_version(v);
        b.as_mut().set_update_url(url);
        b.as_mut().set_update_notes(notes);
        b.as_mut().set_update_mode(mode);
    });
    status("", false);
    crate::platform::set_tray_update(Some(version.clone())).await;
    Checked::Available(version)
}

/// Download, verify and install the release found by `check`. Reports progress and errors
/// through `update_status`.
pub async fn install() {
    if let Err(e) = try_install().await {
        log::error!("update: {e:#}");
        status(format!("Update failed: {e:#}"), false);
    }
}

async fn try_install() -> Result<()> {
    let rel = FOUND.lock().unwrap().clone().context("no update available")?;
    let version = rel.tag_name.trim_start_matches('v');
    let kind = Install::detect();
    if crate::mock::enabled() {
        status(format!("Mock: would install {version} here."), false);
        return Ok(());
    }
    let name = kind.asset(version).context("this copy of Looting Glass can't update itself")?;
    let url = |n: &str| {
        rel.assets
            .iter()
            .find(|a| a.name == n)
            .map(|a| a.browser_download_url.clone())
            .with_context(|| format!("release has no {n}"))
    };
    let (asset_url, sha_url) = (url(&name)?, url(&format!("{name}.sha256"))?);

    status(format!("Downloading {name}…"), true);
    let ua = crate::config().user_agent();
    let get = |u: String| {
        let ua = ua.clone();
        async move {
            let r = HTTP.get(u).header(reqwest::header::USER_AGENT, ua).send().await?.error_for_status()?;
            anyhow::Ok(r.bytes().await?)
        }
    };
    let sha = get(sha_url).await?;
    let data = get(asset_url).await?;
    verify_sha256(&data, &String::from_utf8_lossy(&sha))?;

    let dir = std::env::temp_dir().join("lootingglass-update");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(&name);
    std::fs::write(&file, &data)?;

    status("Installing…", true);
    if let Some(m) = updated_marker() {
        let _ = std::fs::create_dir_all(m.parent().unwrap());
        let _ = std::fs::write(&m, CURRENT);
    }
    match kind {
        Install::WindowsInstaller => {
            // The installer closes this copy anyway; /relaunch=1 starts the new one when done.
            command(&file).args(["/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/relaunch=1"]).spawn()?;
            std::process::exit(0);
        }
        Install::WindowsZip => {
            let app = EXE.as_deref().and_then(Path::parent).context("unknown install folder")?.to_path_buf();
            tokio::task::spawn_blocking(move || {
                let staged = dir.join("new");
                std::fs::create_dir_all(&staged)?;
                run_ok(command("tar").arg("-xf").arg(&file).arg("-C").arg(&staged))?;
                swap_in(&find_root(&staged, "lootingglass.exe")?, &app)
            })
            .await??;
        }
        Install::LinuxUser => {
            tokio::task::spawn_blocking(move || {
                run_ok(command("tar").arg("-xzf").arg(&file).arg("-C").arg(&dir))?;
                run_ok(command("bash").arg(find_root(&dir, "install.sh")?.join("install.sh")))
            })
            .await??;
        }
        Install::Package | Install::Manual => unreachable!("no asset for {kind:?}"),
    }
    // An install that fails above leaves the marker; the next start sees the same version and
    // says nothing.
    status(format!("Looting Glass {version} is installed. Restart to use it."), false);
    ui(|b| b.set_update_ready(true));
    Ok(())
}

/// Start the freshly installed copy and exit.
pub fn restart() {
    let exe = match Install::detect() {
        Install::LinuxUser => dirs::home_dir().map(|h| h.join(".local/bin/lootingglass")),
        _ => EXE.clone(),
    };
    let Some(exe) = exe else { return };
    let e = relaunch(&exe);
    status(format!("Restart failed: {e}. Start Looting Glass again yourself."), false);
}

/// Starts `exe` and exits; returns only with the reason it couldn't be started. The new process
/// waits for this one's single-instance lock (`LOOTINGGLASS_RESTARTED`).
pub fn relaunch(exe: &std::path::Path) -> std::io::Error {
    match command(exe).env("LOOTINGGLASS_RESTARTED", "1").spawn() {
        Ok(_) => std::process::exit(0),
        Err(e) => e,
    }
}

/// A command that doesn't flash a console window on Windows.
fn command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    #[allow(unused_mut)]
    let mut c = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

fn run_ok(c: &mut std::process::Command) -> Result<()> {
    let out = c.output().with_context(|| format!("running {:?}", c.get_program()))?;
    if !out.status.success() {
        bail!("{:?} failed: {}", c.get_program(), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}

/// The folder holding `marker`: `dir` itself or its single top-level subfolder (release
/// archives contain one `lootingglass-<version>-…` folder).
fn find_root(dir: &Path, marker: &str) -> Result<PathBuf> {
    if dir.join(marker).exists() {
        return Ok(dir.to_path_buf());
    }
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() && p.join(marker).exists() {
            return Ok(p);
        }
    }
    bail!("{marker} not found in the downloaded archive")
}

fn files_under(dir: &Path, base: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            files_under(&p, base, out)?;
        } else if let Ok(rel) = p.strip_prefix(base) {
            out.push(rel.to_path_buf());
        }
    }
    Ok(())
}

fn old_path(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(OLD_SUFFIX);
    PathBuf::from(s)
}

/// Copy every file of `staged` into `app`. Files in use (the running exe, loaded DLLs) can't be
/// overwritten on Windows but can be renamed, so each existing file is first moved aside to
/// `<name>.update-old`. If anything fails, the moved files are put back.
pub fn swap_in(staged: &Path, app: &Path) -> Result<()> {
    let mut files = Vec::new();
    files_under(staged, staged, &mut files)?;
    // (target, whether an old file was moved aside)
    let mut done: Vec<(PathBuf, bool)> = Vec::new();
    let res = (|| -> Result<()> {
        for rel in &files {
            let target = app.join(rel);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let had = target.exists();
            if had {
                let old = old_path(&target);
                let _ = std::fs::remove_file(&old);
                std::fs::rename(&target, &old).with_context(|| format!("moving {} aside", target.display()))?;
            }
            done.push((target.clone(), had));
            std::fs::copy(staged.join(rel), &target).with_context(|| format!("writing {}", target.display()))?;
        }
        Ok(())
    })();
    if res.is_err() {
        for (target, had) in done.iter().rev() {
            let _ = std::fs::remove_file(target);
            if *had {
                let _ = std::fs::rename(old_path(target), target);
            }
        }
    }
    res
}

/// Delete the `.update-old` files a previous zip update left in `dir`.
pub fn remove_old_files(dir: &Path) {
    let mut files = Vec::new();
    if files_under(dir, dir, &mut files).is_err() {
        return;
    }
    for rel in files.iter().filter(|f| f.to_string_lossy().ends_with(OLD_SUFFIX)) {
        if let Err(e) = std::fs::remove_file(dir.join(rel)) {
            log::warn!("removing {}: {e}", rel.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versions() {
        assert_eq!(parse_version("v0.5.0"), Some((0, 5, 0)));
        assert_eq!(parse_version("1.12.3"), Some((1, 12, 3)));
        assert_eq!(parse_version("v1.2"), None);
        assert_eq!(parse_version("v1.2.3.4"), None);
        assert_eq!(parse_version("v1.2.3-rc1"), None);
        assert_eq!(parse_version("latest"), None);
    }

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("v0.5.0", "0.4.1"));
        assert!(is_newer("v0.10.0", "0.9.9"));
        assert!(is_newer("v1.0.0", "0.99.99"));
        assert!(!is_newer("v0.4.1", "0.4.1"));
        assert!(!is_newer("v0.4.0", "0.4.1"));
        assert!(!is_newer("nightly", "0.4.1"));
    }

    #[test]
    fn release_notes_drop_title_and_shrink_headings() {
        let body = "## [0.4.1](https://x/compare/v0.4.0...v0.4.1) (2026-11-20)\n\n\n### Bug Fixes\n\n* **windows:** a fix";
        assert_eq!(notes(body), "**Bug Fixes**\n\n* **windows:** a fix");
    }

    #[test]
    fn checksums() {
        // sha256("hello")
        let sum = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        assert!(verify_sha256(b"hello", &format!("{sum}  file.zip\n")).is_ok());
        assert!(verify_sha256(b"hello", &sum.to_uppercase()).is_ok());
        assert!(verify_sha256(b"hello!", sum).is_err());
        assert!(verify_sha256(b"hello", "").is_err());
    }

    #[test]
    fn install_kinds() {
        let home = Path::new("/home/exile");
        assert_eq!(linux_kind(Path::new("/home/exile/.local/bin/lootingglass"), home), Install::LinuxUser);
        assert_eq!(linux_kind(Path::new("/usr/bin/lootingglass"), home), Install::Package);
        assert_eq!(linux_kind(Path::new("/home/exile/src/target/release/lootingglass"), home), Install::Manual);
        assert_eq!(windows_kind(true, true), Install::WindowsInstaller);
        assert_eq!(windows_kind(false, true), Install::WindowsZip);
        assert_eq!(windows_kind(false, false), Install::Manual);
        assert_eq!(Install::Package.asset("1.0.0"), None);
        assert_eq!(Install::WindowsZip.asset("1.0.0").as_deref(), Some("lootingglass-1.0.0-windows-x64.zip"));
    }

    fn release(tag: &str, files: &[&str]) -> Release {
        let assets = files
            .iter()
            .map(|n| Asset { name: n.to_string(), browser_download_url: format!("https://x/{n}") })
            .collect();
        Release { tag_name: tag.into(), html_url: String::new(), body: String::new(), assets }
    }

    #[test]
    fn offers_a_newer_release_only_with_its_downloads() {
        let zip = "lootingglass-0.9.4-windows-x64.zip";
        let full = release("v0.9.4", &[zip, "lootingglass-0.9.4-windows-x64.zip.sha256"]);
        assert_eq!(offer(&full, "0.9.3", Install::WindowsZip), Offer::Available);
        assert_eq!(offer(&full, "0.9.4", Install::WindowsZip), Offer::UpToDate);
        assert_eq!(offer(&full, "0.10.0", Install::WindowsZip), Offer::UpToDate);
        // Published before the builds attached anything.
        let bare = release("v0.9.4", &[]);
        assert_eq!(offer(&bare, "0.9.3", Install::WindowsZip), Offer::NotReady);
        assert_eq!(offer(&bare, "0.9.3", Install::Package), Offer::NotReady);
        // The file without its checksum (upload still running) isn't enough either.
        assert_eq!(offer(&release("v0.9.4", &[zip]), "0.9.3", Install::WindowsZip), Offer::NotReady);
        // Another platform's files only: the Linux build failed, say.
        assert_eq!(offer(&full, "0.9.3", Install::LinuxUser), Offer::NotReady);
        // Installs that are only told about it need some download, not a specific one.
        assert_eq!(offer(&full, "0.9.3", Install::Package), Offer::Available);
        assert_eq!(offer(&full, "0.9.3", Install::Manual), Offer::Available);
    }

    #[test]
    fn checks_again_soon_while_a_release_isnt_ready() {
        assert_eq!(next_check(Some(&Checked::NotReady("0.9.4".into()))), Duration::from_secs(900));
        assert_eq!(next_check(Some(&Checked::UpToDate)), Duration::from_secs(86400));
        assert_eq!(next_check(Some(&Checked::Failed("offline".into()))), Duration::from_secs(86400));
        assert_eq!(next_check(None), Duration::from_secs(86400), "checks turned off");
    }

    #[test]
    fn announces_each_new_version_once() {
        assert!(should_announce("0.9.4", None, true), "first find this run");
        assert!(!should_announce("0.9.4", Some("0.9.4"), true), "the next day's check");
        assert!(should_announce("0.9.5", Some("0.9.4"), true), "a newer one since");
        assert!(!should_announce("0.9.4", None, false), "checks turned off meanwhile");
    }

    #[test]
    fn mock_release_parses() {
        let v = crate::mock::response("GET", LATEST).unwrap();
        let r: Release = serde_json::from_value(v).unwrap();
        assert!(parse_version(&r.tag_name).is_some());
        // Complete, like a published release: every install kind could update from it.
        for kind in [Install::WindowsInstaller, Install::WindowsZip, Install::LinuxUser] {
            assert_eq!(offer(&r, "0.0.1", kind), Offer::Available, "{kind:?}");
        }
    }

    #[test]
    fn swap_replaces_files_and_cleans_up() {
        let tmp = std::env::temp_dir().join(format!("lg-swap-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let (staged, app) = (tmp.join("staged"), tmp.join("app"));
        std::fs::create_dir_all(staged.join("plugins")).unwrap();
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(staged.join("lootingglass.exe"), "new").unwrap();
        std::fs::write(staged.join("plugins/new.dll"), "added").unwrap();
        std::fs::write(app.join("lootingglass.exe"), "old").unwrap();
        std::fs::write(app.join("config.txt"), "kept").unwrap();

        swap_in(&staged, &app).unwrap();
        assert_eq!(std::fs::read_to_string(app.join("lootingglass.exe")).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(app.join("plugins/new.dll")).unwrap(), "added");
        assert_eq!(std::fs::read_to_string(app.join("lootingglass.exe.update-old")).unwrap(), "old");

        remove_old_files(&app);
        assert!(!app.join("lootingglass.exe.update-old").exists());
        assert_eq!(std::fs::read_to_string(app.join("config.txt")).unwrap(), "kept");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
