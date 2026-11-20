<h1 align="center">
  <img src="data/icons/dev.lootlens.LootLens.svg" alt="" width="48" align="center">
  Loot Lens
</h1>

**A Wayland-native price checker for Path of Exile 1 and Path of Exile 2.**

Hover an item in game, press a hotkey, and Loot Lens shows what it is worth: live listings from the
official trade site, poe.ninja prices for currency and other stackables, and filters you can tweak
on the spot. One app for both games, with a league per game.

Built for KDE Plasma on Wayland with Rust and Qt 6, with experimental Hyprland support. No Electron, no browser, no XWayland. Global
hotkeys, the overlay and clipboard access all use the desktop's native mechanisms.

> Loot Lens is a fan-made tool and is not affiliated with or endorsed by Grinding Gear Games.

<p align="center">
  <img src="docs/screenshots/rare-item.png" alt="Price check of a rare item with editable stat filters" width="331">
  &nbsp;&nbsp;
  <img src="docs/screenshots/currency.png" alt="Currency price check with poe.ninja value" width="331">
</p>

## Features

- **Price check any item.** Uniques by name, rares and magic items by their mods, plus gems,
  currency, fragments, divination cards and more.
- **Editable filters, like Awakened PoE Trade / Exiled Exchange 2.** Every matched mod has a
  checkbox and min/max values, pre-filled from your roll minus a configurable tolerance. Sensible
  mods (life, resistances, movement speed, …) are enabled by default, and the search re-runs as you
  edit.
- **Quality-adjusted values.** Armour, evasion, energy shield and physical DPS are compared at 20%
  quality, the same way the trade site does.
- **Weapon DPS filters.** Physical, elemental and total DPS, attack speed and critical chance.
- **poe.ninja prices** for currency, fragments, essences, scarabs, divination cards, runes, omens
  and other stackables, including the 7-day trend and the value of the whole stack.
- **PoE 1 and PoE 2 in one app.** Switch game from the tray, the settings or a hotkey; each game
  remembers its own league.
- **Results at a glance.** Price, item level, listing age and seller, with an online/any toggle and
  a one-click jump to the full search on pathofexile.com.
- **A proper overlay.** Always on top of the game, on the game's screen, without window
  decorations. Drag the ⠿ handle to move it and its free edges to resize it. Close it with Esc,
  Space or by clicking back into the game.
- **Native settings** in a KDE-style window, stored as TOML.

<p align="center">
  <img src="docs/screenshots/settings.png" alt="Settings window with category sidebar" width="600">
</p>

## How it works on Wayland

Electron-based tools struggle on Wayland because they rely on X11 tricks for hotkeys, key
injection and always-on-top windows. Loot Lens uses the native equivalents instead:

| Need | How Loot Lens does it |
|---|---|
| Hotkeys while the game is focused | XDG **GlobalShortcuts** portal (bound in System Settings) |
| Make the game copy the hovered item | KDE: XDG **RemoteDesktop** portal, keyboard only (you approve it once). Hyprland: `hyprctl dispatch sendshortcut` |
| Read the clipboard without focus | Wayland **data-control** protocol |
| Overlay above a fullscreen game | **wlr-layer-shell** via KDE's LayerShellQt |
| Give focus back to the game | KDE: a one-shot KWin script. Hyprland: `hyprctl dispatch focuswindow` |
| Tray icon | StatusNotifierItem |

## Requirements

- KDE Plasma 6 on Wayland (tested). Hyprland support is **experimental and untested**; see below. GNOME isn't supported: it lacks layer-shell and
  background clipboard access. Sway lacks the GlobalShortcuts portal.
- On Hyprland: `xdg-desktop-portal-hyprland` and a tray (e.g. Waybar's tray module) for the icon
- Qt 6 with Qt Quick, plus `layer-shell-qt`, `kirigami` and `qqc2-desktop-style`
- A Rust toolchain (edition 2024) and a C++ compiler

On Arch / CachyOS:

```bash
sudo pacman -S --needed rustup qt6-base qt6-declarative layer-shell-qt kirigami qqc2-desktop-style
```

## Install

```bash
git clone https://github.com/rockymontana/loot-lens.git
cd loot-lens
./install.sh
```

`install.sh` builds a release binary, installs it to `~/.local/bin/lootlens` and adds a launcher
entry. The `.desktop` file matters: the portals use it to identify the app
(`dev.lootlens.LootLens`).

Start **Loot Lens** from the application launcher. It lives in the system tray.

## Usage

1. In the game, hover an item and press **Ctrl+D**.
2. The first time, KDE asks you to confirm the shortcuts and to allow Loot Lens to control the
   keyboard. That permission is needed so Loot Lens can press Ctrl+C in the game. Both are
   remembered.
3. Adjust the filters if you like. The search updates automatically.
4. Press **Esc** or **Space**, or click back into the game, to close the overlay.

### Shortcuts

| Action | Default |
|---|---|
| Price check hovered item | Ctrl+D |
| Switch between PoE 1 and PoE 2 | unassigned |
| Open settings | unassigned |
| Hide overlay | unassigned |

Change them in *System Settings → Shortcuts → Loot Lens*, or via *Settings → Shortcuts*.

#### Hyprland (experimental)

> ⚠️ Hyprland support is untested: it was written against Hyprland's documented `hyprctl`
> dispatchers and portal, but nobody has run it on Hyprland yet. Bug reports and fixes are
> very welcome.

Hyprland doesn't let apps choose their own keys, so bind them in `hyprland.conf`:

```ini
bind = CTRL, D, global, dev.lootlens.LootLens:price-check
bind = , F7, global, dev.lootlens.LootLens:toggle-game     # optional
```

The shortcut ids are `price-check`, `toggle-game`, `settings` and `hide-overlay`. If a bind does
nothing, run `hyprctl globalshortcuts` to see the exact names your portal registered.
Copying and refocusing go through `hyprctl`, so no permission dialog appears.

## Configuration

Settings live in `~/.config/lootlens/config.toml` and can be edited in the settings window
(click the tray icon) or by hand:

```toml
game = "poe1"            # or "poe2"
contact = ""             # optional, appended to the User-Agent sent to the trade API

[poe1]
league = "Standard"

[poe2]
league = "Standard"

[trade]
status = "online"        # "online", "any", or "securable" (PoE 2 instant buyout)
stat_tolerance = 10      # % below your roll used as the filter minimum
max_results = 20

[overlay]
side = "right"
margin_top = 80
margin_side = 24
width = 380
max_height = 0           # 0 = fit content
```

## Troubleshooting

- **The overlay has a title bar or isn't on top.** Loot Lens has to run as a native Wayland client.
  It overrides `QT_QPA_PLATFORM=xcb` for itself, but if you start it with `LOOTLENS_ALLOW_X11=1`
  the overlay can't use layer-shell.
- **"The game didn't copy anything".** Make sure the cursor is over an item and that you allowed
  keyboard control when KDE asked. To reset that permission, delete
  `~/.local/state/lootlens/remote-desktop.token` and restart Loot Lens.
- **Debug output.** Run `RUST_LOG=lootlens=debug QT_FORCE_STDERR_LOGGING=1 lootlens` in a terminal.

## Development

```bash
cargo build
cargo test
LOOTLENS_DEMO=dev/boots.txt cargo run         # price-check an item text file without the game
LOOTLENS_SNAPSHOT=/tmp/shots LOOTLENS_DEMO=dev/boots.txt cargo run   # also save README screenshots
```

| Path | What |
|---|---|
| `src/item.rs` | Parser for the game's Ctrl+C item text (with tests) |
| `src/trade.rs` | Trade API client (`/api/trade` and `/api/trade2`), stat matching, filters |
| `src/ninja.rs` | poe.ninja prices for stackables |
| `src/platform/` | Desktop integration: hotkeys, copying the item, focus, tray |
| `src/platform/linux/` | KDE/Wayland backend: portals, data-control clipboard, KWin, SNI tray |
| `src/platform/windows.rs` | Windows backend stub with the planned approach |
| `src/bridge.rs` | cxx-qt `Backend` object the QML binds to |
| `qml/` | Overlay and settings views |
| `cpp/main.cpp` | Small Qt bootstrap: QApplication for the Breeze style, layer-shell setup |

### Porting

Everything outside `src/platform/` and the `LOOTLENS_LAYER_SHELL` parts of `cpp/main.cpp` is
platform-independent. A new OS needs a module implementing the functions documented in
`src/platform/mod.rs`. A Windows port is planned; see `src/platform/windows.rs`.

## License

Loot Lens is free software, licensed under the [GNU General Public License v3.0 or later](LICENSE.md).
