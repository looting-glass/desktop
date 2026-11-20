# Looting Glass: notes for agents

Looting Glass is a native (Rust + Qt 6/QML) price checker for Path of Exile 1 and 2. `README.md`
covers features, layout and the dev commands; this file holds the conventions that aren't
written down anywhere else.

## Git

- **GitHub Flow**: `main` is always releasable; work happens on short-lived branches named
  after the change type (`feat/…`, `fix/…`, `ci/…`, `docs/…`) and lands through a PR that is
  **squash-merged**.
- Commit messages and PR titles use **Conventional Commits** (`feat(windows): …`, `fix: …`,
  `ci: …`, `docs: …`). The PR title becomes the squash commit on `main` and drives releases.
- Releases come from **release-please**: after each merge it updates a release PR that bumps
  the version in `Cargo.toml` and writes `CHANGELOG.md` (`fix` → patch, `feat` → minor,
  `docs`/`ci`/`chore` → no release). Merging that PR tags `vX.Y.Z`, publishes the GitHub
  release and builds the Linux and Windows downloads. Leave the version and changelog to it.
- Commit and push when the maintainer asks; leave changes uncommitted otherwise. Already-pushed
  history stays as it is.

## Two games, one app

PoE 1 and PoE 2 differ in item text, stat ids and trade API filter groups (`src/item.rs`,
`src/trade.rs`). Scope every parsing or query change to the game it was observed in (gate on
`Game`), and confirm the other game still behaves as before with its `dev/*.txt` samples.

## Platforms

- KDE Plasma (Wayland) is the tested platform; Hyprland and Windows are **experimental**.
  Keep that label in docs and code comments until someone has run them with the game.
- Looks: on KDE the app follows the user's colour scheme (Breeze). Everywhere else it uses Qt
  Quick's Fusion style with fixed colours copied from the KDE build (`qml/KdeLook.qml`), in both
  light and dark system mode, so it matches the README screenshots.
- Desktop-specific code lives in `src/platform/` (contract in `src/platform/mod.rs`) and the
  `#ifdef`s of `cpp/main.cpp`. QML uses only Qt Quick Controls, with bundled fallback icons in
  `qml/icons/`, so it runs on any platform.
- Every PR runs the CI workflow (build + tests on Linux and Windows); keep it green.
- Windows UI: CI's Windows job renders the overlay, manual check and settings on a real Windows
  runner in light and dark mode (artifact `ui-snapshots`; `gh workflow run ci.yml --ref <branch>`
  for any branch). Check it before calling a UI change done on Windows.
- Windows code can't be built locally. Build any branch for both platforms with
  `gh workflow run build.yml --ref <branch>`; the packages appear as run artifacts.

## Verifying changes

- `cargo test` for the parser and hotkey logic.
- `LOOTINGGLASS_DEMO=dev/<item>.txt cargo run` price-checks a sample without the game; add a
  `dev/` sample for any new item type you handle.
- UI changes: compare before/after with `LOOTINGGLASS_SNAPSHOT=<dir>` (writes the README
  screenshots).
- Repeated runs: the trade API rate-limits quickly. For UI work use `LOOTINGGLASS_MOCK=1` (canned
  responses, no network; extend `src/mock.rs` for new items); to replay real data, set
  `LOOTINGGLASS_HTTP_CACHE=<dir>` so replies are recorded once.

## Screenshots

README screenshots come from `LOOTINGGLASS_SNAPSHOT`, cropped to the app with seller names
blurred. Show new screenshots to the maintainer before committing them. Desktop screenshots
taken while debugging are cropped to the app window and deleted after use: the desktop can show
private chats.
