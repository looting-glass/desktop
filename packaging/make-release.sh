#!/usr/bin/env bash
# Build Looting Glass and package it as a prebuilt release tarball.
# Usage: packaging/make-release.sh [version]   (default: version from Cargo.toml)
# Output: dist/lootingglass-<version>-x86_64.tar.gz and a .sha256 next to it.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v cargo >/dev/null || export PATH="$(echo "$HOME"/.rustup/toolchains/stable-*/bin):$PATH"

version="${1:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)}"
version="${version#v}"
name="lootingglass-${version}-$(uname -m)"

cargo build --release --locked

rm -rf "dist/$name"
mkdir -p "dist/$name/data/icons"
install -m755 target/release/lootingglass "dist/$name/lootingglass"
install -m755 install.sh "dist/$name/install.sh"
install -m644 data/dev.lootingglass.LootingGlass.desktop "dist/$name/data/"
install -m644 data/icons/dev.lootingglass.LootingGlass.svg "dist/$name/data/icons/"
install -m644 LICENSE.md README.md "dist/$name/"
strip "dist/$name/lootingglass" 2>/dev/null || true

tar -C dist -czf "dist/$name.tar.gz" "$name"
(cd dist && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256")
echo "dist/$name.tar.gz"
