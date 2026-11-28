# Install

Looting Glass runs on KDE Plasma 6 on Wayland, which is where it's tested. Hyprland and Windows
support is **experimental**: it's written and built, but nobody has run it with the game yet.
[Desktops](desktops.md) has the details, including why GNOME, Sway and Umbriel don't work yet.

## Linux

All Linux installs need Qt 6 with Qt Quick, plus `layer-shell-qt`, `kirigami` and
`qqc2-desktop-style`. Most Plasma 6 desktops already have them. On Hyprland you also need
`xdg-desktop-portal-hyprland` and a tray, such as Waybar's tray module, for the icon.

Once it's installed, start **Looting Glass** from the application launcher. It lives in the
system tray.

### Arch Linux and CachyOS package

A `lootingglass-bin` package for the AUR is on its way. Until it's published, build the same
package from this repository with `makepkg`:

```bash
git clone https://github.com/looting-glass/desktop.git looting-glass
cd looting-glass/packaging/aur/lootingglass-bin
makepkg -si
```

This installs the prebuilt release binary system-wide through pacman, with its dependencies,
launcher entry and icon. `pacman -R lootingglass-bin` removes it cleanly.

### Prebuilt release

No Rust needed. Install the runtime libraries first; on Arch and CachyOS:

```bash
sudo pacman -S --needed qt6-declarative layer-shell-qt kirigami qqc2-desktop-style
```

Then download `lootingglass-<version>-x86_64.tar.gz` from the
[latest release](https://github.com/looting-glass/desktop/releases/latest) and run its installer:

```bash
tar xzf lootingglass-*-x86_64.tar.gz
cd lootingglass-*-x86_64
./install.sh
```

Release binaries are built on Arch Linux. They need a Qt at least as new as the one they were
built with, and keep working as your Qt updates, so a rolling distribution (Arch, CachyOS,
EndeavourOS) is the safe bet. On other distributions, build from source.

### From source

Building needs a Rust toolchain (edition 2024) and a C++ compiler, plus the Qt packages above and
`qt6-base`. On Arch and CachyOS:

```bash
sudo pacman -S --needed rustup qt6-base qt6-declarative layer-shell-qt kirigami qqc2-desktop-style
git clone https://github.com/looting-glass/desktop.git looting-glass
cd looting-glass
./install.sh
```

### What install.sh does

From a release or a source checkout, `install.sh` installs for your user only:

- the program, `~/.local/bin/lootingglass`;
- the launcher entry, `~/.local/share/applications/dev.lootingglass.LootingGlass.desktop`;
- the icon, `~/.local/share/icons/hicolor/scalable/apps/dev.lootingglass.LootingGlass.svg`.

The launcher entry matters: the desktop portals use it to identify the app
(`dev.lootingglass.LootingGlass`). To uninstall, delete those three files.

## Windows (experimental)

> Windows support is experimental: the build compiles and packages in CI, but nobody has run it
> with the game yet. Bug reports and fixes are very welcome.

Windows 10 or 11. Download `lootingglass-<version>-windows-x64-setup.exe` from the
[latest release](https://github.com/looting-glass/desktop/releases/latest) and run it. It
installs for your user only (no admin rights), adds a Start menu entry, can start Looting Glass
with Windows, and uninstalls from *Settings → Apps*.

Prefer a portable app? Extract `lootingglass-<version>-windows-x64.zip` anywhere and run
`lootingglass.exe`. Either way, Looting Glass lives in the system tray.

The downloads aren't code-signed yet, so Windows SmartScreen may say "Windows protected your PC".
Click *More info → Run anyway*.

Run Path of Exile in **windowed fullscreen** (borderless): Windows can't draw the overlay above a
game in exclusive fullscreen.
