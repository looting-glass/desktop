# Development

Looting Glass is written in Rust with a Qt 6 / QML interface, joined by
[cxx-qt](https://github.com/KDAB/cxx-qt). [Install](install.md#from-source) lists what a build
needs. Contributions follow the conventions in [`AGENTS.md`](../AGENTS.md).

## Build and test

```bash
cargo build
cargo test
```

Each module's unit tests sit in its `mod tests`, on real item text from `dev/*.txt` and canned
API replies from `src/mock.rs`. The release automation's scripts have their own tests:

```bash
python3 -m unittest discover --start-directory .github/scripts
```

## Running without the game

These environment variables let you work on Looting Glass without Path of Exile running, and
without hitting the trade API's rate limits on every run:

| Variable | Effect |
|---|---|
| `LOOTINGGLASS_DEMO=dev/boots.txt` | Price-checks an item text file at start-up instead of waiting for the hotkey. |
| `LOOTINGGLASS_GAME=poe2` | Forces the game for this run, such as for `dev/boots-poe2.txt`. |
| `LOOTINGGLASS_MOCK=1` | Uses canned API replies from `src/mock.rs`: no network. |
| `LOOTINGGLASS_MOCK_UPDATE=1` | With `LOOTINGGLASS_MOCK`, also fakes a newer release, to work on the update UI. |
| `LOOTINGGLASS_HTTP_CACHE=<dir>` | Records real API replies once and replays them on later runs. |
| `LOOTINGGLASS_SNAPSHOT=<dir>` | Saves screenshots of the overlay and the settings, the source of `docs/screenshots`. |

For example:

```bash
LOOTINGGLASS_DEMO=dev/boots.txt cargo run
LOOTINGGLASS_GAME=poe2 LOOTINGGLASS_DEMO=dev/boots-poe2.txt cargo run
LOOTINGGLASS_MOCK=1 LOOTINGGLASS_DEMO=dev/boots.txt cargo run
LOOTINGGLASS_HTTP_CACHE=/tmp/lg-cache LOOTINGGLASS_DEMO=dev/boots.txt cargo run
LOOTINGGLASS_SNAPSHOT=/tmp/shots LOOTINGGLASS_DEMO=dev/boots.txt cargo run
```

## Source layout

| Path | What |
|---|---|
| `src/item.rs` | Parser for the game's Ctrl+C item text |
| `src/trade.rs` | Trade API client (`/api/trade` and `/api/trade2`), stat matching, filters |
| `src/ninja.rs` | poe.ninja prices for stackables |
| `src/update.rs` | Update check (GitHub releases) and self-update per install type |
| `src/platform/` | Desktop integration: hotkeys, copying the item, focus, tray |
| `src/platform/linux/` | Linux backend: portals, data-control clipboard, virtual keyboard, KWin, Hyprland, Sway and Umbriel IPC, SNI tray |
| `src/platform/windows.rs` | Windows backend (experimental): RegisterHotKey, SendInput, Win32 clipboard |
| `src/theme/` | Themes: presets, palettes worked out from three colours, theme codes |
| `src/bridge.rs` | cxx-qt `Backend` object the QML binds to |
| `qml/` | Overlay, manual check and settings views |
| `cpp/main.cpp` | Small Qt bootstrap: Breeze or Fusion style, layer-shell setup |
| `docs/` | This documentation, also published on the website |

## Porting

Everything outside `src/platform/` and the platform `#ifdef`s in `cpp/main.cpp` is
platform-independent, and the QML uses only Qt Quick Controls (no KDE libraries) with bundled
fallback icons. A new OS needs a module implementing the functions documented in
`src/platform/mod.rs`; `src/platform/windows.rs` is the smallest example. New Linux desktops are
covered by [Desktops](desktops.md).

Windows code can't be built on Linux. To build any branch for both platforms, run the Build
workflow by hand and download the packages from the run's artifacts:

```bash
gh workflow run build.yml --ref <branch>
```

## Making a release

Releases are automated with [release-please](https://github.com/googleapis/release-please) and
planned with milestones.

1. Changes land on `main` as squash-merged PRs with
   [Conventional Commit](https://www.conventionalcommits.org) titles.
2. After each merge, release-please keeps a release PR up to date: it bumps the version in
   `Cargo.toml` (`fix:` → patch, `feat:` → minor) and adds the changes to `CHANGELOG.md`.
3. The issues that should ship together go into a milestone. When its last open issue is
   closed (as completed or not planned), the
   [Milestone release workflow](../.github/workflows/milestone-release.yml) merges the release
   PR, starts the [Release workflow](../.github/workflows/release-please.yml) and closes the
   milestone. The version comes from the commits, not the milestone's name. If there is nothing
   to release (only `docs:`, `ci:` or `chore:` changes), it only closes the milestone. Put
   issues in milestones, not PRs: GitHub counts an open PR as open work in its milestone, but
   closing a PR doesn't start the workflow.
4. The Release workflow tags `vX.Y.Z`, creates the GitHub release as a draft and runs the
   [Build workflow](../.github/workflows/build.yml).
5. The Linux tarball and the Windows installer and zip, each with a SHA-256 checksum, are
   attached to the draft, and only then is it published. The update check and the website never
   see a release without its downloads.

If a build fails, the release stays a draft; re-run the failed jobs to finish it. If the
Milestone release run fails, the milestone stays open: fix the cause and run the workflow by
hand for that milestone, first as a dry run, which only logs what it would do:

```bash
gh workflow run milestone-release.yml -f milestone=<number> -f dry-run=true
```

Merging the release PR by hand still releases right away, without a milestone.
