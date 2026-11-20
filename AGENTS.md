# Loot Lens: notes for agents

Loot Lens is a native (Rust + Qt 6/QML) price checker for Path of Exile 1 and 2. `README.md`
covers features, layout and the dev commands; this file holds the conventions that aren't
written down anywhere else.

## Git

- Branches follow **git-flow**: `main` holds released versions (tagged `vX.Y.Z`, which
  triggers the release workflow), `develop` collects finished work. New work goes on
  `feature/<name>` from `develop`; `release/<version>` and `hotfix/<version>` merge into
  `main` and back into `develop`.
- Feature PRs into `develop` are **squash-merged**. Release and hotfix branches use a regular
  merge so `main` and `develop` keep a shared history.
- Commit messages and PR titles use **Conventional Commits** (`feat(windows): …`, `fix: …`,
  `ci: …`, `docs: …`). The PR title becomes the squash commit, so it carries the same weight.
- Commit and push when the maintainer asks; leave changes uncommitted otherwise. Already-pushed
  history stays as it is.

## Two games, one app

PoE 1 and PoE 2 differ in item text, stat ids and trade API filter groups (`src/item.rs`,
`src/trade.rs`). Scope every parsing or query change to the game it was observed in (gate on
`Game`), and confirm the other game still behaves as before with its `dev/*.txt` samples.

## Platforms

- KDE Plasma (Wayland) is the tested platform; Hyprland and Windows are **experimental**.
  Keep that label in docs and code comments until someone has run them with the game.
- Desktop-specific code lives in `src/platform/` (contract in `src/platform/mod.rs`) and the
  `#ifdef`s of `cpp/main.cpp`. QML uses only Qt Quick Controls, with bundled fallback icons in
  `qml/icons/`, so it runs on any platform.
- Windows code can't be built locally; the release workflow's Windows job (manual run from
  the Actions tab) is the build check.

## Verifying changes

- `cargo test` for the parser and hotkey logic.
- `LOOTLENS_DEMO=dev/<item>.txt cargo run` price-checks a sample without the game; add a
  `dev/` sample for any new item type you handle.
- UI changes: compare before/after with `LOOTLENS_SNAPSHOT=<dir>` (writes the README
  screenshots).

## Screenshots

README screenshots come from `LOOTLENS_SNAPSHOT`, cropped to the app with seller names
blurred. Show new screenshots to the maintainer before committing them. Desktop screenshots
taken while debugging are cropped to the app window and deleted after use: the desktop can show
private chats.
