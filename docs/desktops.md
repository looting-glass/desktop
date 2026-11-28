# Desktops

## Supported desktops

<!-- The table planned in issue #64 (step 4), generated from the platform providers'
     requirements, replaces this list. -->

| Desktop | Status |
|---|---|
| KDE Plasma 6 (Wayland) | Supported and tested. |
| Hyprland | **Experimental**: written against Hyprland's documented `hyprctl` dispatchers and portal, but not yet run with the game. |
| Sway | **Experimental**: written against Sway's IPC and the virtual-keyboard protocol, but not yet run with the game. Hotkeys are keys you bind to `lootingglass trigger` ([Shortcuts](shortcuts.md#sway-experimental)). |
| Umbriel | **Experimental**: written against Umbriel's documented IPC and the virtual-keyboard protocol, but not yet run with the game. Hotkeys are keys you bind to `lootingglass trigger` ([Shortcuts](shortcuts.md#umbriel-experimental)). |
| Windows 10 and 11 | **Experimental**: built and packaged in CI, but not yet run with the game. |
| GNOME | Not yet: Mutter has no layer-shell for the overlay and doesn't let apps read the clipboard in the background. GNOME 48 and later have the GlobalShortcuts portal. |

Looting Glass picks how it does each job (hotkeys, copying the item, reading the clipboard,
refocusing the game) by what the desktop can do rather than by its name. On a desktop that can't
do one of them, it says which one is missing and suggests the manual check.

Other wlroots compositors, such as river and Wayfire, get the same virtual keyboard and
`lootingglass trigger` hotkeys, but can't give focus back to the game: click it after the overlay
closes. GNOME is planned next in [issue #64](https://github.com/looting-glass/desktop/issues/64).
A desktop stays experimental until someone has run it with the game.

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
