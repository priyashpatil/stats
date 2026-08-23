# Stats

Stats is a lightweight dashboard for your Mac's system health, Amp usage, and Codex usage. Use it as a native menu bar app or run it directly in a terminal.

<p align="center">
  <img src="https://cdn.priyashpatil.com/products/stats-app-22-08-26.gif" alt="Stats dashboard demo">
</p>

## What it shows

- CPU, RAM, GPU, storage, and network metrics
- Amp subscription usage
- Codex weekly quota and token activity
- Four customizable world clocks

Stats reads usage through the installed Amp and Codex CLIs. It does not read or store their credentials.

## Install

Stats requires macOS 11 or newer and currently provides prebuilt releases for Apple Silicon Macs. [Amp](https://ampcode.com/) and the [Codex CLI](https://github.com/openai/codex) must also be installed and signed in.

### Homebrew (recommended)

Copy and paste this command into Terminal:

```sh
brew install --cask priyashpatil/tap/stats &&
  xattr -dr com.apple.quarantine /Applications/Stats.app
```

The second command allows macOS to open the app after Homebrew has downloaded and verified it.

### Download

1. Open the [latest release](https://github.com/priyashpatil/stats/releases/latest).
2. Download the `Stats-...-macOS-arm64.zip` file for Apple Silicon.
3. Unzip `Stats.app`, move it to `/Applications`, and open it.

The downloaded app is self-contained and does not require Rust or Swift. Releases are ad-hoc signed but not Apple-notarized, so macOS may ask you to confirm the first launch in **System Settings → Privacy & Security**.

## Getting started

1. Open Stats from your Applications folder. The dashboard appears and a Stats icon is added to the menu bar.
2. Use the menu bar icon to show or hide the dashboard, open **Settings**, or quit Stats.
3. In **Settings → General**, choose whether Stats should launch when you sign in.
4. In **Settings → Clocks**, search for a city or time zone for each of the four clocks.

You can move and resize the dashboard. Stats remembers its position and restores it on the primary desktop the next time it opens.

## Terminal use

If you prefer a terminal, install the CLI with Cargo:

```sh
cargo install --git https://github.com/priyashpatil/stats --locked
stats
```

Press `q` or Escape to quit. For a single, script-friendly snapshot, run:

```sh
stats --once
```

Run `stats --help` to see all CLI options. The AI usage sections require both `amp` and `codex` to be available in `PATH`.

GPU utilization uses the `ioreg` metrics provided by macOS and requires no elevated permissions.

## Privacy and security

- Amp and Codex requests are made through their installed CLIs; credentials are not copied into Stats.
- The Codex app server listens only on `127.0.0.1` while Stats is running.
- Parsed usage responses are cached in the operating system's user cache directory (`~/Library/Caches/stats` on macOS). On Unix systems, Stats restricts the directory to the current user (`0700`) and cache files to `0600`.

To report a security issue, see [SECURITY.md](SECURITY.md).

## Contributing

Want to build Stats from source or help improve it? See [CONTRIBUTING.md](CONTRIBUTING.md) for setup, testing, and release details.

## License

[MIT](LICENSE)
