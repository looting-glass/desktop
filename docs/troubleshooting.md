# Troubleshooting

## The overlay has a title bar or isn't on top

On Linux, Looting Glass has to run as a native Wayland client to use layer-shell for the overlay.
It overrides `QT_QPA_PLATFORM=xcb` for itself, unless you start it with
`LOOTINGGLASS_ALLOW_X11=1`.

On Windows, switch the game to **windowed fullscreen** (borderless). Windows can't draw the
overlay above a game in exclusive fullscreen.

## "The game didn't copy anything"

Make sure the cursor is over an item. On KDE, also check that you allowed keyboard control when
Plasma asked: Looting Glass needs it to press Ctrl+C in the game. To ask again, delete
`~/.local/state/lootingglass/remote-desktop.token` and restart Looting Glass.

## "Rate limit: waiting …" or "Trade site busy"

GGG limits how often an IP address may search the trade site (about 5 searches per 10 seconds,
with longer limits on top), and locks you out for longer each time you go over. Looting Glass
reads those limits from every reply and paces its requests to stay one under them:

- When only a couple of searches are left, the overlay's footer says so.
- A short wait is sat out.
- A longer one shows "Trade site busy, try again in Ns" instead of sending.

Other trade tools on the same connection count towards the same limits.

## Windows says "Windows protected your PC"

The downloads aren't code-signed yet. Click *More info → Run anyway*.

## Something else

Run Looting Glass from a terminal with debug output:

```bash
RUST_LOG=lootingglass=debug QT_FORCE_STDERR_LOGGING=1 lootingglass
```

Then [open an issue](https://github.com/looting-glass/desktop/issues) with what it prints, your
desktop and how you installed Looting Glass.
