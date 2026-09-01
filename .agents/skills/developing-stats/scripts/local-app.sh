#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../../.." && pwd)"
PRODUCTION_APP="/Applications/Stats.app"
PRODUCTION_EXECUTABLE="$PRODUCTION_APP/Contents/MacOS/Stats"
SOURCE_APP="$HOME/Applications/Stats.app"
SOURCE_EXECUTABLE="$SOURCE_APP/Contents/MacOS/Stats"
SOURCE_SERVICE="gui/$(id -u)/com.priyashpatil.stats"

build() {
    cargo build --manifest-path "$ROOT/Cargo.toml" --release --locked
    swift build --package-path "$ROOT/macos" -c release
}

test_all() {
    cargo test --manifest-path "$ROOT/Cargo.toml" --locked
    swift test --package-path "$ROOT/macos"
}

print_process() {
    local executable="$1"
    pgrep -afil "^${executable}$" || printf '%s\n' 'not running'
}

status() {
    printf '%s\n' 'Production:'
    print_process "$PRODUCTION_EXECUTABLE"

    printf '%s\n' 'Source development:'
    print_process "$SOURCE_EXECUTABLE"

    printf '%s' 'Source LaunchAgent: '
    if launchctl print "$SOURCE_SERVICE" >/dev/null 2>&1; then
        printf '%s\n' 'running'
    else
        printf '%s\n' 'not loaded'
    fi
}

run_source() {
    "$ROOT/develop.sh"
    status
}

case "${1:-}" in
    build) build ;;
    test) test_all ;;
    run) run_source ;;
    status) status ;;
    *)
        printf 'Usage: %s {build|test|run|status}\n' "$0" >&2
        exit 2
        ;;
esac
