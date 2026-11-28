# Shortcuts

| Action | Default | Id |
|---|---|---|
| Price check hovered item | Ctrl+D | `price-check` |
| Switch between PoE 1 and PoE 2 | unassigned | `toggle-game` |
| Open settings | unassigned | `settings` |
| Open manual price check | unassigned | `manual-check` |
| Hide overlay | unassigned | `hide-overlay` |
| Open hovered item on poedb or the wiki | unassigned | `item-info` |

On Linux the shortcuts belong to the desktop, which registers them for Looting Glass through the
GlobalShortcuts portal. How you change them depends on the desktop. The id is the name the desktop
knows the action by.

## KDE Plasma

Change them right in *Settings → Shortcuts*: click a shortcut, press the keys you want (Esc
cancels, Backspace clears) and Save. A key another app already uses is shown with that app's
name, and Save moves it over.

These are the same shortcuts as in *System Settings → Shortcuts → Looting Glass*, so you can
change them in either place.

## Hyprland (experimental)

> Hyprland support is experimental: it was written against Hyprland's documented `hyprctl`
> dispatchers and portal, but nobody has run it on Hyprland yet. Bug reports and fixes are very
> welcome.

Hyprland doesn't let apps choose their own keys, so bind them in `hyprland.conf`, using the ids
from the table above:

```ini
bind = CTRL, D, global, dev.lootingglass.LootingGlass:price-check
bind = , F7, global, dev.lootingglass.LootingGlass:toggle-game     # optional
```

If a bind does nothing, run `hyprctl globalshortcuts` to see the exact names your portal
registered.

## Windows (experimental)

Looting Glass registers the shortcuts itself. Type them into *Settings → Shortcuts*, such as
`Ctrl+D`, `Ctrl+Shift+F7` or `Alt+W`, and leave a field empty to unassign it. They're stored in
the [`[shortcuts]` section](configuration.md#shortcuts-windows) of `config.toml`.

## Other desktops

Sway, Umbriel and GNOME aren't supported yet; [Desktops](desktops.md) says why and what's
planned. Each will get its own section here when it is.
