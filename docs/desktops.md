# Desktops

## Supported desktops

<!-- Generated from src/platform/linux/desktops.rs by `LOOTINGGLASS_UPDATE_DOCS=1 cargo test desktops_table` -->
| Desktop | Overlay | Shortcuts | Copy | Clipboard | Focus | Status |
|---|---|---|---|---|---|---|
| KDE Plasma 6 | layer-shell | [KDE shortcuts](shortcuts.md#kde-plasma) | remote desktop portal | data-control | KWin script | Supported and tested. |
| Hyprland | layer-shell | [hyprland.conf](shortcuts.md#hyprland-experimental) | hyprctl | data-control | hyprctl | **Experimental**: not yet run with the game. |
| Sway | layer-shell | [command](shortcuts.md#sway-experimental) | virtual keyboard | data-control | Sway IPC | **Experimental**: not yet run with the game. |
| Umbriel | layer-shell | [command](shortcuts.md#umbriel-experimental) | virtual keyboard | data-control | Umbriel IPC | **Experimental**: not yet run with the game. |
| river, Wayfire | layer-shell | [command](shortcuts.md#other-desktops) | virtual keyboard | data-control | none | **Experimental**: not yet run with the game. |
| GNOME | **missing** | portal | remote desktop portal | **missing** | none | **Not supported**: GNOME doesn't let apps show prices over the game, read what the game copied or switch you back to it. [Why GNOME isn't supported](#why-gnome-isnt-supported) explains. |
<!-- End of the generated table -->

Windows 10 and 11 are **experimental**: built and packaged in CI, but not yet run with the game.
There, Looting Glass registers the hotkeys itself ([Shortcuts](shortcuts.md#windows-experimental)).

Looting Glass picks how it does each job (showing the overlay over the game, shortcuts, copying
the item, reading the clipboard, refocusing the game) by what the desktop can do rather than by
its name. The table shows what it picks on each desktop, in the words *Settings → Shortcuts* uses.
**missing** means the desktop doesn't allow that job, and Settings says so too. Focus *none* means
Looting Glass can't switch you back to the game, so you click it yourself.

A desktop stays experimental until someone has run it with the game. If you play on one of
them, a report of how it went, good or bad, is very welcome.

## Why GNOME isn't supported

A price check takes three steps, and GNOME doesn't let an app do any of them:

- **Showing the prices on top of the game.** The prices have to appear over the game, even when
  it fills the whole screen. GNOME doesn't let an app keep its window on top like that, so the
  prices would end up hidden behind the game.
- **Reading the item the game copied.** When you press the hotkey, the game copies the item's
  details and Looting Glass reads them. GNOME only lets the app you're using read what was
  copied, and while you play, that's the game, not Looting Glass.
- **Putting you back in the game.** When the prices close, you should be able to carry on
  playing without clicking the game first. GNOME doesn't let one app switch you to another.

GNOME leaves these out on purpose, so apps can't snoop on what you copy or cover up other
windows. KDE Plasma, Hyprland, Sway and Umbriel let apps do them.

Other price checkers work around this through an older compatibility layer that GNOME still
includes. In our experience that workaround is what makes copying items unreliable in those
tools, so Looting Glass doesn't use it. If GNOME ever allows these three things, Looting Glass
will support it.

If you play on GNOME, KDE Plasma is the best-tested option. Many Linux systems let you install it
alongside GNOME and choose between them on the login screen.

## How it works on Wayland

Electron-based tools struggle on Wayland because they rely on X11 tricks for hotkeys, key
injection and always-on-top windows. Looting Glass uses the native equivalents instead, with no
XWayland:

| Need | How Looting Glass does it |
|---|---|
| Hotkeys while the game is focused | XDG **GlobalShortcuts** portal. Without one (Sway, Umbriel): keys bound to `lootingglass trigger <id>` in the window manager's config |
| Make the game copy the hovered item | KDE: XDG **RemoteDesktop** portal, keyboard only (you approve it once). Hyprland: `hyprctl dispatch sendshortcut`. Sway, Umbriel: a Wayland **virtual keyboard** |
| Read the clipboard without focus | Wayland **data-control** protocol |
| Overlay above a fullscreen game | **wlr-layer-shell**, through KDE's LayerShellQt |
| Give focus back to the game | KDE: a one-shot KWin script. Hyprland: `hyprctl dispatch focuswindow`. Sway: `swaymsg focus`. Umbriel: `umbriel msg window-focus` |
| Tray icon | StatusNotifierItem |

On Hyprland, Sway and Umbriel no permission dialog appears: copying and refocusing go through the
compositor's IPC or its virtual keyboard.

On Windows the same jobs use the Win32 API: `RegisterHotKey` for hotkeys, `SendInput` to copy
the item, the Win32 clipboard, and a top-most window for the overlay.
