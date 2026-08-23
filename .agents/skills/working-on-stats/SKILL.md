---
name: working-on-stats
description: Builds and reinstalls Stats from source. Use for every coding task in the Stats repository.
compatibility: Requires macOS, Rust, Swift, and the command-line tools checked by develop.sh.
---

# Working on Stats

Use the canonical workflow for every code change so both the Rust CLI and Swift app are built, installed, and verified together.

## Required workflow

1. Before editing, inspect `git status --short` and preserve unrelated worktree changes.
2. Make the requested change and run its focused tests while iterating.
3. Before reporting completion, run this command from the repository root:

   ```sh
   ./develop.sh
   ```

   Do not substitute separate Cargo or Swift commands for this final check. `develop.sh` builds both release targets, calls the installer, and verifies the installed app, signature, LaunchAgent, and process path.

## Installation contract

- Treat `./develop.sh` as the only complete development build, reinstall, and verification entry point. `install.sh` is its lower-level installer.
- The development app belongs at `~/Applications/Stats.app`.
- Its login item is `~/Library/LaunchAgents/com.priyashpatil.stats.plist` with service name `com.priyashpatil.stats`.
- Do not install or copy a development build to `/Applications` and do not launch a binary from `.build` directly.
- The Homebrew release cask at `/Applications/Stats.app` and the source-development app are mutually exclusive because both use bundle identifier `com.priyashpatil.stats`.

## Conflicting release app

`develop.sh` stops if `/Applications/Stats.app` uses this project's bundle identifier. To diagnose it, inspect the bundle:

```sh
if [ -f /Applications/Stats.app/Contents/Info.plist ]; then
  plutil -extract CFBundleIdentifier raw /Applications/Stats.app/Contents/Info.plist
fi
```

If the identifier is `com.priyashpatil.stats`, inspect the Homebrew receipt before changing anything:

```sh
receipt=/opt/homebrew/Caskroom/stats/.metadata/INSTALL_RECEIPT.json
[ -f "$receipt" ] && plutil -extract source.tap raw "$receipt"
```

If it reports `priyashpatil/tap`, the conflicting copy is this project's release cask. Remove it with `brew uninstall --cask priyashpatil/tap/stats` only when the user requested cleanup or approved removing the release install. Do not use the ambiguous unqualified `stats` cask name because Homebrew also has an unrelated cask with that token. If the receipt has another source or is absent, stop and report the conflicting path rather than deleting it.

## Verification

Treat a successful `./develop.sh` exit as the required verification. Report its installed version, signature result, service state, and executable path. If it reports a duplicate or unexpected process, diagnose its launcher and installation source instead of killing arbitrary processes or deleting unknown apps.
