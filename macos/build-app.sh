#!/bin/bash

set -euo pipefail

if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <output-app> <stats-executable>" >&2
    exit 1
fi

root="$(cd "$(dirname "$0")" && pwd)"
app="$1"
stats="$2"

if [ ! -x "$stats" ]; then
    echo "Stats executable is missing or not executable: $stats" >&2
    exit 1
fi

swift build --package-path "$root" -c release
bin_dir="$(swift build --package-path "$root" -c release --show-bin-path)"

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin_dir/Stats" "$app/Contents/MacOS/Stats"
cp "$stats" "$app/Contents/Resources/stats"
cp "$root/Info.plist" "$app/Contents/Info.plist"

icon_work="$(mktemp -d)"
trap 'rm -rf "$icon_work"' EXIT
iconset="$icon_work/Stats.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    sips -s format png -z "$size" "$size" "$root/AppIcon.svg" \
        --out "$iconset/icon_${size}x${size}.png" >/dev/null
    double_size=$((size * 2))
    sips -s format png -z "$double_size" "$double_size" "$root/AppIcon.svg" \
        --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/Stats.icns"

for resource_bundle in "$bin_dir"/*.bundle; do
    [ -e "$resource_bundle" ] || continue
    cp -R "$resource_bundle" "$app/Contents/Resources/"
done

codesign --force --deep --sign - "$app"
plutil -lint "$app/Contents/Info.plist"
