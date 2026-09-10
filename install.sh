#!/bin/bash

set -euo pipefail

if [ "$(uname -s)" != "Darwin" ]; then
    echo "install.sh builds the macOS app and requires macOS" >&2
    echo "To install only the CLI, run: cargo install --path . --locked" >&2
    exit 1
fi

for command in cargo swift codesign iconutil launchctl plutil sips; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "Missing required command: $command" >&2
        exit 1
    fi
done

system_app="/Applications/Stats.app"
system_bundle_id=""
if [ -f "$system_app/Contents/Info.plist" ]; then
    system_bundle_id="$(
        plutil -extract CFBundleIdentifier raw "$system_app/Contents/Info.plist" 2>/dev/null || true
    )"
fi
if [ "$system_bundle_id" = "com.priyashpatil.stats" ]; then
    echo "A release copy of Stats is installed at $system_app" >&2
    echo "Source development uses ~/Applications/Stats.app; keeping both would run duplicate menu bar apps." >&2
    echo "Remove the release copy before installing from source." >&2
    exit 1
fi

root="$(cd "$(dirname "$0")" && pwd)"
macos="$root/macos"
app="$HOME/Applications/Stats.app"
agent="$HOME/Library/LaunchAgents/com.priyashpatil.stats.plist"
service="gui/$(id -u)/com.priyashpatil.stats"

cargo install --path "$root" --root "$HOME/.cargo" --force --locked
for command in codex-usage codex-usage-status; do
    ln -sfn "$HOME/.cargo/bin/stats" "$HOME/.cargo/bin/$command"
done

"$macos/build-app.sh" "$app" "$HOME/.cargo/bin/stats"

mkdir -p "$(dirname "$agent")"
cat >"$agent" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.priyashpatil.stats</string>
    <key>ProgramArguments</key>
    <array>
        <string>$app/Contents/MacOS/Stats</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>LimitLoadToSessionType</key>
    <string>Aqua</string>
</dict>
</plist>
EOF

plutil -lint "$app/Contents/Info.plist" "$agent"
launchctl bootout "$service" 2>/dev/null || true
for _ in {1..50}; do
    if ! launchctl print "$service" >/dev/null 2>&1; then
        break
    fi
    sleep 0.1
done
launchctl bootstrap "gui/$(id -u)" "$agent"

echo "Stats CLI installed in ~/.cargo/bin"
echo "Stats.app installed in ~/Applications and enabled at login"
