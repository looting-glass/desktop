# Desktops

## Supported desktops

<!-- The table planned in issue #64 (step 4), generated from the platform providers'
     requirements, replaces this list. -->

| Desktop | Status |
|---|---|
| KDE Plasma 6 (Wayland) | Supported and tested. |
| Hyprland | **Experimental**: written against Hyprland's documented `hyprctl` dispatchers and portal, but not yet run with the game. |
| Windows 10 and 11 | **Experimental**: built and packaged in CI, but not yet run with the game. |
| Sway | Not yet: the overlay and the clipboard work, but there's no GlobalShortcuts portal for the hotkeys, and Looting Glass can't send Ctrl+C to the game there yet. |
| Umbriel | Not yet: as on Sway, the overlay and the clipboard work, but the hotkeys and sending Ctrl+C to the game don't yet. |
| GNOME | Not yet: Mutter has no layer-shell for the overlay and doesn't let apps read the clipboard in the background. GNOME 48 and later have the GlobalShortcuts portal. |

Looting Glass picks how it does each job (hotkeys, copying the item, reading the clipboard,
refocusing the game) by what the desktop can do rather than by its name. On a desktop that can't
do one of them, it says which one is missing and suggests the manual check.

More desktops are planned in [issue #64](https://github.com/looting-glass/desktop/issues/64):
Sway and Umbriel through the virtual-keyboard protocol and a `lootingglass trigger` command that
any window manager can bind to a key, and GNOME after that. A desktop stays experimental until
someone has run it with the game.

## How it works on Wayland

Electron-based tools struggle on Wayland because they rely on X11 tricks for hotkeys, key
injection and always-on-top windows. Looting Glass uses the native equivalents instead, with no
XWayland:

| Need | How Looting Glass does it |
|---|---|
| Hotkeys while the game is focused | XDG **GlobalShortcuts** portal |
| Make the game copy the hovered item | KDE: XDG **RemoteDesktop** portal, keyboard only (you approve it once). Hyprland: `hyprctl dispatch sendshortcut` |
| Read the clipboard without focus | Wayland **data-control** protocol |
| Overlay above a fullscreen game | **wlr-layer-shell**, through KDE's LayerShellQt |
| Give focus back to the game | KDE: a one-shot KWin script. Hyprland: `hyprctl dispatch focuswindow` |
| Tray icon | StatusNotifierItem |

On Hyprland, copying and refocusing go through `hyprctl`, so no permission dialog appears.

On Windows the same jobs use the Win32 API: `RegisterHotKey` for hotkeys, `SendInput` to copy
the item, the Win32 clipboard, and a top-most window for the overlay.
