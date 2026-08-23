---
name: developing-stats
description: Builds, installs, and verifies the Stats macOS app from source using its canonical development location. Use when developing Stats or asked to build, reinstall, launch, or diagnose duplicate Stats apps.
compatibility: Requires macOS, Rust, Swift, and the command-line tools checked by install.sh.
---

# Developing Stats

Use one source-development installation of Stats and verify that it is the only running copy.

## Installation contract

- Treat `./install.sh` as the only development build and install entry point.
- The development app belongs at `~/Applications/Stats.app`.
- Its login item is `~/Library/LaunchAgents/com.priyashpatil.stats.plist` with service name `com.priyashpatil.stats`.
- Do not install or copy a development build to `/Applications` and do not launch a binary from `.build` directly.
- The Homebrew release cask at `/Applications/Stats.app` and the source-development app are mutually exclusive because both use bundle identifier `com.priyashpatil.stats`.

## Rebuild and reinstall

1. Inspect `git status --short`; preserve unrelated worktree changes.
2. Check for a conflicting release app:

   ```sh
   if [ -f /Applications/Stats.app/Contents/Info.plist ]; then
     plutil -extract CFBundleIdentifier raw /Applications/Stats.app/Contents/Info.plist
   fi
   ```

3. If the identifier is `com.priyashpatil.stats`, inspect the Homebrew receipt before changing anything:

   ```sh
   receipt=/opt/homebrew/Caskroom/stats/.metadata/INSTALL_RECEIPT.json
   [ -f "$receipt" ] && plutil -extract source.tap raw "$receipt"
   ```

   If it reports `priyashpatil/tap`, the conflicting copy is this project's release cask. Remove it with `brew uninstall --cask priyashpatil/tap/stats` only when the user requested cleanup or approved removing the release install. Do not use the ambiguous unqualified `stats` cask name because Homebrew also has an unrelated cask with that token. If the receipt has another source or is absent, stop and report the conflicting path rather than deleting it.

4. Run the canonical installer from the repository root:

   ```sh
   ./install.sh
   ```

   This installs the Rust CLI, builds and signs the native wrapper, replaces `~/Applications/Stats.app`, rewrites its LaunchAgent, and restarts the service.

## Verification

Require all of these checks after installation:

```sh
plutil -extract CFBundleShortVersionString raw \
  "$HOME/Applications/Stats.app/Contents/Info.plist"
codesign --verify --deep --strict "$HOME/Applications/Stats.app"
launchctl print "gui/$(id -u)/com.priyashpatil.stats"
```

Then inspect every native wrapper process:

```sh
for pid in $(pgrep -x Stats || true); do
  lsof -a -p "$pid" -d txt -Fn 2>/dev/null | sed -n 's/^n//p' | head -1
done
```

There must be exactly one `Stats` wrapper process and its executable must be `~/Applications/Stats.app/Contents/MacOS/Stats`. Also verify that no app with bundle identifier `com.priyashpatil.stats` remains at `/Applications/Stats.app`.

Report the installed version, signature result, service state, and executable path. If more than one copy remains, diagnose its launcher and installation source instead of killing arbitrary processes or deleting unknown apps.
