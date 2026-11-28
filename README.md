<h1 align="center">
  <img src="data/icons/dev.lootingglass.LootingGlass.svg" alt="" width="40" height="40" align="top">
  Looting Glass
</h1>

**A Wayland-native price checker for Path of Exile 1 and Path of Exile 2.**

Hover an item in game, press a hotkey, and Looting Glass shows what it is worth: live listings from the
official trade site, poe.ninja prices for currency and other stackables, and filters you can tweak
on the spot. One app for both games, with a league per game.

Built for KDE Plasma on Wayland with Rust and Qt 6, with experimental Hyprland, Sway, Umbriel and
Windows support. No Electron, no browser, no XWayland. Global hotkeys, the overlay and clipboard access
all use the desktop's native mechanisms.

**Website and documentation:** [looting-glass.github.io](https://looting-glass.github.io/), with an
[Android version](https://looting-glass.github.io/android.html) for console players.

> Looting Glass is a fan-made tool and is not affiliated with or endorsed by Grinding Gear Games.

<table align="center">
  <tr><th>Path of Exile</th><th>Path of Exile 2</th></tr>
  <tr>
    <td valign="top"><img src="docs/screenshots/poe1-rare.png" alt="PoE 1 rare item price check with editable stat filters" width="300"></td>
    <td valign="top"><img src="docs/screenshots/poe2-rare.png" alt="PoE 2 rare item price check with editable stat filters" width="300"></td>
  </tr>
</table>

## Features

- **Price check any item:** uniques by name, rares and magic items by their mods, plus gems,
  maps, waystones, currency and other stackables.
- **Editable filters** pre-filled from your rolls, with pseudo totals, mod tiers, sockets,
  influences and weapon DPS. The search re-runs as you edit.
- **poe.ninja prices** and the cheapest **bulk exchange** offers for currency and stackables.
- **Map check** for PoE 1: mark the map mods you can't run, and see them in red on the next map.
- **Manual check:** paste item text from a friend or a forum post.
- **A proper overlay** on top of the game, plus a tray icon, native settings and update checks.

[Usage](docs/usage.md) describes them all.

## Install

- **Arch Linux and CachyOS:** build the `lootingglass-bin` package with `makepkg` (AUR package on
  its way).
- **Other Linux:** download the tarball from the
  [latest release](https://github.com/looting-glass/desktop/releases/latest) and run its
  `install.sh`, or build from source.
- **Windows (experimental):** run the installer from the
  [latest release](https://github.com/looting-glass/desktop/releases/latest), or use the
  portable zip.

[Install](docs/install.md) has the requirements and step-by-step instructions.

## Documentation

The full guide is in [`docs/`](docs/README.md) and on the
[website](https://looting-glass.github.io/docs/): [install](docs/install.md),
[usage](docs/usage.md), [shortcuts](docs/shortcuts.md), [configuration](docs/configuration.md),
[updates](docs/updates.md), [troubleshooting](docs/troubleshooting.md),
[desktops](docs/desktops.md) and [development](docs/development.md).

## Development

```bash
cargo build
cargo test
LOOTINGGLASS_MOCK=1 LOOTINGGLASS_DEMO=dev/boots.txt cargo run   # price-check a sample, no game or network
```

[Development](docs/development.md) covers the dev environment variables, the source layout,
releases and porting. [`AGENTS.md`](AGENTS.md) has the project's conventions.

## Thanks & inspiration

Looting Glass stands on the shoulders of the tools the PoE community already loves:

- [**Awakened PoE Trade**](https://github.com/SnosMe/awakened-poe-trade): the overlay that set the
  standard for in-game price checking, and the model for Looting Glass's filter layout.
- [**Exiled Exchange 2**](https://github.com/Kvan7/Exiled-Exchange-2): carried that experience over
  to Path of Exile 2.
- [**Sidekick**](https://github.com/Sidekick-Poe/Sidekick): showed how nice it is to have one app for
  both games, with the league picked per game.

Huge thanks to their authors and contributors. Looting Glass exists because they showed what a great
price checker looks like; it just brings that to a native Wayland desktop.

Prices come from [poe.ninja](https://poe.ninja) and the official
[Path of Exile trade site](https://www.pathofexile.com/trade). Thank you to poe.ninja for keeping
the economy data open, and to Grinding Gear Games for the trade API that makes all of this
possible.

## License

Looting Glass is free software, licensed under the [GNU General Public License v3.0 or later](LICENSE.md).
