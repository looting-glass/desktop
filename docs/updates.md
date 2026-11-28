# Updates

Looting Glass asks GitHub for the latest release shortly after it starts and once a day. That's
one anonymous request each time. Turn it off under *Settings → About*, or check by hand there or
with the tray menu's *Check for updates…*.

## When a new version is out

The new version shows next to the version number in the overlay, in the tray menu and on the
About page, with its release notes. Nothing installs until you click *Install update*, and what
happens then depends on how you installed Looting Glass:

| Installed with | Install update |
|---|---|
| Windows installer | Updates silently and restarts the app. |
| Windows zip | Replaces the files after checking the download's SHA-256, then offers a restart. |
| Linux `install.sh` | Replaces the files after checking the download's SHA-256, then offers a restart. |
| A package manager (AUR) | Not offered: you're told about the new version, and the package manager installs it. |

## Notifications on Linux

On Linux, Looting Glass also uses desktop notifications:

- A new version shows up as a notification, once per version.
- *Check for updates…* in the tray answers with one: "up to date" or "… is available". Click it
  to open the About page.
- After an update, the new version says it was updated.
