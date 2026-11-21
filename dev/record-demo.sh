#!/bin/sh
# Record the README demo: runs the scripted LOOTINGGLASS_RECORD flow and encodes its frames to
# docs/screenshots/demo.gif (README) and demo.mp4 (website). Needs ffmpeg and ImageMagick.
#
#   dev/record-demo.sh [item.txt] [manual-item.txt]
#
# Trade replies are kept in target/demo-cache (LOOTINGGLASS_HTTP_CACHE): the first run records
# real listings, later runs replay them. Delete it for fresh prices.
set -eu
item=${1:-dev/boots.txt}
manual=${2:-dev/ring.txt}
out=docs/screenshots
bg='#2b2d31'
frames=$(mktemp -d)
trap 'rm -rf "$frames"' EXIT

LOOTINGGLASS_GAME=${LOOTINGGLASS_GAME:-poe2} LOOTINGGLASS_HTTP_CACHE=${LOOTINGGLASS_HTTP_CACHE:-target/demo-cache} \
LOOTINGGLASS_RECORD="$frames" LOOTINGGLASS_RECORD_MANUAL="$manual" LOOTINGGLASS_DEMO="$item" \
    cargo run --quiet

# Frames differ in size (overlay grows, manual window is wider): centre each on one canvas.
size=$(magick identify -format '%w %h\n' "$frames"/frame-*.png | awk '
    $1 > w { w = $1 } $2 > h { h = $2 } END { print w + 32 "x" h + 32 }')
for f in "$frames"/frame-*.png; do
    magick "$f" -background "$bg" -gravity north -splice 0x16 -extent "$size" -flatten "$f"
done

fps=12
ffmpeg -loglevel error -y -framerate $fps -i "$frames/frame-%05d.png" \
    -vf "split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=none:diff_mode=rectangle" \
    "$out/demo.gif"
ffmpeg -loglevel error -y -framerate $fps -i "$frames/frame-%05d.png" \
    -vf "scale=trunc(iw/2)*2:trunc(ih/2)*2" -c:v libx264 -pix_fmt yuv420p -crf 26 -movflags +faststart \
    "$out/demo.mp4"
ls -lh "$out/demo.gif" "$out/demo.mp4"
