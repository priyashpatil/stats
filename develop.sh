#!/bin/bash

set -euo pipefail

if [ "$(uname -s)" != "Darwin" ]; then
    echo "develop.sh builds and reinstalls the macOS app and requires macOS" >&2
    exit 1
fi

root="$(cd "$(dirname "$0")" && pwd)"
app="$HOME/Applications/Stats.app"
service="gui/$(id -u)/com.priyashpatil.stats"
expected_executable="$app/Contents/MacOS/Stats"

echo "Building Rust release target..."
cargo build --manifest-path "$root/Cargo.toml" --release --locked

echo "Building Swift release target..."
swift build --package-path "$root/macos" -c release

echo "Reinstalling Stats..."
"$root/install.sh"

echo "Verifying installed app..."
version="$(plutil -extract CFBundleShortVersionString raw "$app/Contents/Info.plist")"
codesign --verify --deep --strict "$app"
launchctl print "$service" >/dev/null

executable=""
pids=""
for _ in {1..50}; do
    pids="$(pgrep -x Stats || true)"
    if [ "$(printf '%s\n' "$pids" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1 ]; then
        executable="$(lsof -a -p "$pids" -d txt -Fn 2>/dev/null | sed -n 's/^n//p' | head -1)"
        [ "$executable" = "$expected_executable" ] && break
    fi
    sleep 0.1
done

if [ "$executable" != "$expected_executable" ]; then
    echo "Expected exactly one Stats process at: $expected_executable" >&2
    echo "Found Stats process IDs: ${pids:-none}" >&2
    [ -n "$executable" ] && echo "Found executable: $executable" >&2
    exit 1
fi

echo "Installed version: $version"
echo "Signature: valid"
echo "LaunchAgent: running"
echo "Executable: $executable"
