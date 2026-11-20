#!/usr/bin/env bash
# Build and install Loot Lens for the current user.
set -euo pipefail
cd "$(dirname "$0")"
command -v cargo >/dev/null || export PATH="$(echo "$HOME"/.rustup/toolchains/stable-*/bin):$PATH"
cargo build --release
install -Dm755 target/release/lootlens "$HOME/.local/bin/lootlens"
install -Dm644 data/dev.lootlens.LootLens.desktop "$HOME/.local/share/applications/dev.lootlens.LootLens.desktop"
install -Dm644 data/icons/dev.lootlens.LootLens.svg "$HOME/.local/share/icons/hicolor/scalable/apps/dev.lootlens.LootLens.svg"
sed -i "s|^Exec=.*|Exec=$HOME/.local/bin/lootlens|" "$HOME/.local/share/applications/dev.lootlens.LootLens.desktop"
update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
# Remove the install from before the rename (Glimmer).
rm -f "$HOME/.local/bin/glimmer" "$HOME/.local/share/applications/dev.glimmer.Glimmer.desktop"
echo "Installed. Start 'Loot Lens' from the launcher (or run ~/.local/bin/lootlens)."
