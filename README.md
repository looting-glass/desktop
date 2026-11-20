# Loot Lens

A Wayland-native Path of Exile 1 **and** 2 price checker for KDE Plasma, in the spirit of
Awakened PoE Trade / Exiled Exchange 2 — without Electron.

> Loot Lens is a fan-made tool and is not affiliated with or endorsed by Grinding Gear Games.

- **Rust** core, **Qt 6 / QML** UI (Breeze style via `org.kde.desktop`, Kirigami settings form)
- **Global hotkeys** through the XDG `GlobalShortcuts` portal — rebind them in
  *System Settings → Shortcuts → Loot Lens*
- **Ctrl+C injection** through the `RemoteDesktop` portal (keyboard only). KDE asks once; the grant is
  remembered via a restore token in `~/.local/state/lootlens/`
- **Clipboard** read via the data-control protocol (no focus stealing)
- **Overlay** is a layer-shell surface (KDE LayerShellQt) on the overlay layer, so it shows above the game
- One app for both games: pick game + league per game in settings, the tray menu, or the
  "Switch between PoE 1 and PoE 2" shortcut
- Settings in TOML: `~/.config/lootlens/config.toml`

## Requirements (Arch/CachyOS)

`qt6-base qt6-declarative layer-shell-qt kirigami qqc2-desktop-style` and a Rust toolchain.

## Install / run

```bash
./install.sh
```

The `.desktop` file must be installed for the portals to identify the app (app id `dev.lootlens.LootLens`).

## Default shortcuts

| Action | Default |
|---|---|
| Price check hovered item | Ctrl+D |
| Switch PoE 1 / PoE 2 | unassigned |
| Open settings | unassigned |
| Hide overlay | unassigned (Esc works when the overlay has focus) |

## Layout

| Path | What |
|---|---|
| `src/item.rs` | Parser for the game's Ctrl+C item text (+ tests) |
| `src/trade.rs` | Official trade API client (`/api/trade` and `/api/trade2`), stat matching |
| `src/ninja.rs` | poe.ninja prices for stackables |
| `src/platform/mod.rs` | Platform layer contract: hotkeys, copy item, tray |
| `src/platform/linux/` | KDE/Wayland backend: portals, data-control clipboard, SNI tray |
| `src/platform/windows.rs` | Windows backend stub, with the planned approach |
| `src/bridge.rs` | cxx-qt `Backend` QObject the QML binds to |
| `cpp/main.cpp` | Tiny Qt bootstrap; layer-shell overlay on Linux, frameless top-most window elsewhere |
| `qml/` | Overlay and settings views |

## Porting

Everything outside `src/platform/` and the `LOOTLENS_LAYER_SHELL` parts of `cpp/main.cpp` is
platform-independent. A new OS needs a module implementing the functions documented in
`src/platform/mod.rs`; Linux-only crates are under a `cfg(target_os = "linux")` dependency table.
