# Looting Glass: notes for agents

Looting Glass is a native (Rust + Qt 6/QML) price checker for Path of Exile 1 and 2. `README.md`
covers features, layout and the dev commands; this file holds the conventions that aren't
written down anywhere else.

## Git

- **GitHub Flow**: `main` is always releasable; work happens on short-lived branches named
  after the change type (`feat/…`, `fix/…`, `ci/…`, `docs/…`) and lands through a PR that is
  **squash-merged**. Changes that don't touch the program or its builds (docs, `AGENTS.md`)
  are committed straight to `main` as a `docs:` commit instead.
- Commit messages and PR titles use **Conventional Commits** (`feat(windows): …`, `fix: …`,
  `ci: …`, `docs: …`). The PR title becomes the squash commit on `main` and drives releases.
- Releases come from **release-please**: after each merge it updates a release PR that bumps
  the version in `Cargo.toml` and writes `CHANGELOG.md` (`fix` → patch, `feat` → minor,
  `docs`/`ci`/`chore` → no release). Merging that PR tags `vX.Y.Z`, publishes the GitHub
  release and builds the Linux and Windows downloads. Leave the version and changelog to it.
- Releases are batched: the release PR stays open and collects several features and fixes.
  Merge it only when the maintainer says it's time for a release, not after every feature.
- The website shows the latest release's changelog and download links, and a daily deploy
  picks up new releases. The Release workflow deploys it right after attaching the downloads
  (needs the `SITE_DEPLOY_TOKEN` secret: a fine-grained token with Actions read/write on
  looting-glass.github.io). If that step was skipped or failed, start
  `gh workflow run Pages --repo looting-glass/looting-glass.github.io` by hand;
  the website's `AGENTS.md` says what to check.
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

## Code quality

Write idiomatic Rust on the stable toolchain, and make every change testable and robust. New and
changed code meets these; existing code is brought up to them when you touch it.

- **Testable by design.** Put each decision in a plain function that takes what it needs as
  arguments (text, headers, the current time, previous state) and returns a result, with no I/O,
  clock, globals or Qt inside; keep the networking, file and UI code around it thin. That function
  gets unit tests for the normal case and the edge cases (empty, malformed, boundary values).
- **Tests come with the change.** Every new behaviour or fixed bug gets a test in the module's
  `mod tests`, on real game text (`dev/*.txt`) or canned API replies (`src/mock.rs`). A change
  that can't be tested that way says why in its PR, and is checked by hand as *Verifying
  changes* describes.
- **Robust at runtime.** Input from the game, the clipboard, the network and files can always be
  wrong: handle it with `Result` and `?`, add `.context(…)` (anyhow) where the error would be
  unclear, and show users a plain-language message. `unwrap`/`expect` only where failure would be
  a bug in our own code (a fixed regex, a poisoned lock), never on outside data. Background tasks
  log errors and keep running.
- **Mindful of memory and resources.** Safe Rust prevents leaks of freed memory and data races;
  what it doesn't prevent is memory and work that pile up in a process running for days in the
  tray. Every cache has a bound or expiry (like poe.ninja's 15 minutes), every spawned task has an
  end (a timeout or a closed channel), and queues stay bounded. Share large data with `Arc` or
  references instead of cloning it. `Box::leak`/`mem::forget` only for true app-lifetime statics.
  Keep blocking work (file I/O, process waits, heavy parsing) off the async runtime and the Qt
  thread (`spawn_blocking`).
- **Stable Rust only**: no nightly features, `unsafe` only for platform FFI (the Windows API),
  each block with a `// SAFETY:` comment saying why it's sound. New dependencies only when they
  save real work, and well maintained.
- **Clean builds.** No new `cargo clippy` warnings and no compiler warnings; `cargo test` passes.

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

The website ([looting-glass/looting-glass.github.io](https://github.com/looting-glass/looting-glass.github.io))
uses the same `docs/screenshots`; it picks up changes daily or on a manual Pages run there.
README screenshots come from `LOOTINGGLASS_SNAPSHOT`, cropped to the app with seller names
blurred. Show new screenshots to the maintainer before committing them.

Any change to how the app looks (overlay, settings, manual check) regenerates the affected
`docs/screenshots` in the same PR, so the README's screenshot section always matches the app.
They show real listings: record the replies once with `LOOTINGGLASS_HTTP_CACHE` and retake from
the cache. After it merges, start the website's Pages run
(`gh workflow run Pages --repo looting-glass/looting-glass.github.io`) instead of
waiting for the daily pickup.

Desktop screenshots taken while debugging are cropped to the app window and deleted after use:
the desktop can show private chats.
