# Contributing to Stats

Thanks for helping improve Stats. This guide covers the technical setup; the [README](README.md) is the user-facing overview.

## Project structure

- `src/` contains the Rust terminal dashboard and usage providers.
- `macos/` contains the native Swift wrapper, shared-config editor, and app tests.
- `develop.sh` builds, reinstalls, and verifies the complete development app.
- `install.sh` is the lower-level installer used by `develop.sh`.
- `.github/workflows/` contains CI and release automation.

## Prerequisites

- Rust 1.85 or newer with Cargo, rustfmt, and Clippy
- Swift 6.1 or newer for the native app
- macOS for building and running the native app
- Amp, Claude Code, and Codex installed and signed in for exercising their usage integrations

## Set up the repository

```sh
git clone https://github.com/priyashpatil/stats.git
cd stats
cargo build --locked
swift build --package-path macos
```

Run the terminal dashboard during development with:

```sh
cargo run --locked
```

Use `cargo run --locked -- --once` when you only need one rendered snapshot.

## Build and install the macOS app

Use the repository's canonical development workflow rather than running separate builds or launching the Swift build output directly:

```sh
./develop.sh
```

This command:

- builds the Rust and Swift release targets
- installs `stats`, `codex-usage`, and `codex-usage-status` in `~/.cargo/bin`
- builds and ad-hoc signs `~/Applications/Stats.app`
- installs its user LaunchAgent and starts the app
- verifies the app signature, service state, and running executable

The development app at `~/Applications/Stats.app` and the Homebrew release at `/Applications/Stats.app` cannot be installed together because they share a bundle identifier. If the release is installed, remove it first with:

```sh
brew uninstall --cask priyashpatil/tap/stats
```

To build only the terminal app from a local checkout, including on other supported Unix platforms, run:

```sh
cargo install --path . --locked
```

## Run the checks

Run the same checks used by CI before opening a pull request:

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
swift test --package-path macos
```

After changing Rust or Swift code, run `./develop.sh` to build both components, reinstall the app, and verify the installed copy.

## Pull requests

- Keep each change focused and explain the user-visible outcome.
- Add or update tests when behavior changes.
- Keep the Rust CLI and native wrapper behavior consistent where they share functionality.
- Update user or contributor documentation when setup, commands, or behavior changes.

## Releases

Stats uses Calendar Versioning in the form `YYYY.M.PATCH`. The first release in a month uses patch `0`; subsequent fixes increment it. For example, `2026.8.1` is the first patch to the August 2026 release.

Maintainers publish a release by running the **Release** workflow from the repository's default branch. The workflow calculates the next version, updates and commits the Cargo package and app bundle versions, creates the matching tag, builds the Apple Silicon app and CLI archive, and publishes them to GitHub Releases with SHA-256 checksums.

Pushing a tag manually is also supported. In that case, the tag, Cargo package, and app bundle versions must match.
