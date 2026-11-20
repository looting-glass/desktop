#!/usr/bin/env bash
# Install Looting Glass for the current user.
# From a release tarball this installs the bundled binary; from a source checkout it builds first.
set -euo pipefail
cd "$(dirname "$0")"

if [ -x ./lootingglass ] && [ ! -f Cargo.toml ]; then
    bin=./lootingglass
else
    command -v cargo >/dev/null || export PATH="$(echo "$HOME"/.rustup/toolchains/stable-*/bin):$PATH"
    cargo build --release
    bin=target/release/lootingglass
fi

install -Dm755 "$bin" "$HOME/.local/bin/lootingglass"
install -Dm644 data/dev.lootingglass.LootingGlass.desktop "$HOME/.local/share/applications/dev.lootingglass.LootingGlass.desktop"
install -Dm644 data/icons/dev.lootingglass.LootingGlass.svg "$HOME/.local/share/icons/hicolor/scalable/apps/dev.lootingglass.LootingGlass.svg"
sed -i "s|^Exec=.*|Exec=$HOME/.local/bin/lootingglass|" "$HOME/.local/share/applications/dev.lootingglass.LootingGlass.desktop"
update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
# Remove installs from before the renames (Loot Lens, Glimmer). Settings carry over on first start.
rm -f "$HOME/.local/bin/lootlens" "$HOME/.local/share/applications/dev.lootlens.LootLens.desktop" \
      "$HOME/.local/share/icons/hicolor/scalable/apps/dev.lootlens.LootLens.svg" \
      "$HOME/.local/bin/glimmer" "$HOME/.local/share/applications/dev.glimmer.Glimmer.desktop"
echo "Installed. Start 'Looting Glass' from the launcher (or run ~/.local/bin/lootingglass)."
