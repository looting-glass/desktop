#!/usr/bin/env bash
# Build Loot Lens and package it as a prebuilt release tarball.
# Usage: packaging/make-release.sh [version]   (default: version from Cargo.toml)
# Output: dist/lootlens-<version>-x86_64.tar.gz and a .sha256 next to it.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v cargo >/dev/null || export PATH="$(echo "$HOME"/.rustup/toolchains/stable-*/bin):$PATH"

version="${1:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)}"
version="${version#v}"
name="lootlens-${version}-$(uname -m)"

cargo build --release --locked

rm -rf "dist/$name"
mkdir -p "dist/$name/data/icons"
install -m755 target/release/lootlens "dist/$name/lootlens"
install -m755 install.sh "dist/$name/install.sh"
install -m644 data/dev.lootlens.LootLens.desktop "dist/$name/data/"
install -m644 data/icons/dev.lootlens.LootLens.svg "dist/$name/data/icons/"
install -m644 LICENSE.md README.md "dist/$name/"
strip "dist/$name/lootlens" 2>/dev/null || true

tar -C dist -czf "dist/$name.tar.gz" "$name"
(cd dist && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256")
echo "dist/$name.tar.gz"
