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

for resource_bundle in "$bin_dir"/*.bundle; do
    [ -e "$resource_bundle" ] || continue
    cp -R "$resource_bundle" "$app/Contents/Resources/"
done

codesign --force --deep --sign - "$app"
plutil -lint "$app/Contents/Info.plist"
