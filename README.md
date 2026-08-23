# Stats

A terminal dashboard for macOS system metrics, Amp usage, and Codex usage. It can run directly in a terminal or inside the included native macOS wrapper.

<p align="center">
  <img src="https://cdn.priyashpatil.com/products/stats-app-22-08-26.gif" alt="Stats dashboard demo">
</p>

## Features

- CPU, RAM, GPU, storage, and network metrics
- Amp subscription usage
- Codex weekly quota and token activity
- World clocks
- Native macOS window powered by SwiftTerm
- `--once` output for scripts

Stats reads usage through the installed Amp and Codex CLIs. It does not read or store their credentials.

## Requirements

- macOS 11 or newer for the native app
- [Amp](https://ampcode.com/) installed and signed in
- [Codex CLI](https://github.com/openai/codex) installed and signed in

## Install

### Homebrew

Install the self-contained macOS app:

```sh
brew install --cask priyashpatil/tap/stats &&
  xattr -dr com.apple.quarantine /Applications/Stats.app
```

Releases are ad-hoc signed but not Apple-notarized. The second command explicitly
removes macOS quarantine after Homebrew downloads and verifies the cask.

### Download the macOS app

1. Open the [latest release](https://github.com/priyashpatil/stats/releases/latest).
2. Download the `Stats-...-macOS-arm64.zip` file for Apple Silicon.
3. Unzip `Stats.app`, move it to `/Applications`, and open it.

The downloaded app is self-contained and does not require Rust or Swift. Releases are ad-hoc signed but not Apple-notarized, so macOS may ask you to confirm the first launch in **System Settings → Privacy & Security**.

### Build from source

Install Rust with Cargo and Swift 6.1 or newer, then clone the repository and run:

```sh
./install.sh
```

The installer:

- installs `stats`, `codex-usage`, and `codex-usage-status` in `~/.cargo/bin`
- builds and installs `~/Applications/Stats.app`
- starts the app and enables it at login with a user LaunchAgent

To install only the terminal app, including on other Unix platforms:

```sh
cargo install --path . --locked
```

The AI usage sections require both `amp` and `codex` to be available in `PATH`.

## Releases and versioning

Stats uses Calendar Versioning in the form `YYYY.M.PATCH`. The first release in a month uses patch `0`; subsequent fixes increment it. For example, `2026.8.1` is the first patch to the August 2026 release.

To publish a release, run the **Release** workflow from the repository's default branch. It calculates the next CalVer version, updates and commits the Cargo package and app bundle versions, creates the matching tag, builds a self-contained macOS app and CLI archive for Apple Silicon, and publishes them to GitHub Releases with SHA-256 checksums.

Pushing a tag manually remains supported. In that case, the tag, Cargo package, and app bundle versions must match.

## Usage

```text
stats [OPTIONS]

Options:
      --codex-usage-status
  -i, --interval <seconds>
      --once
      --amp-interval <seconds>
      --storage-interval <seconds>
  -h, --help
```

Press `q` or Escape to quit the interactive dashboard.

GPU utilization uses the `ioreg` metrics provided by macOS and requires no elevated permissions.

## Privacy and security

- Amp and Codex requests are made through their installed CLIs; credentials are not copied into Stats.
- The Codex app server listens only on `127.0.0.1` while Stats is running.
- Parsed usage responses are cached in the operating system's user cache directory (`~/Library/Caches/stats` on macOS). On Unix systems, Stats restricts the directory to the current user (`0700`) and cache files to `0600`.

To report a security issue, see [SECURITY.md](SECURITY.md).

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
swift test --package-path macos
```

## License

[MIT](LICENSE)
