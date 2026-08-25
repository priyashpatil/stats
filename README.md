# Stats

Stats is a lightweight dashboard for your Mac's system health, Amp usage, and Codex usage. Use it as a native menu bar app or run it directly in a terminal.

<p align="center">
  <img src="https://cdn.priyashpatil.com/products/stats-app-22-08-26.gif" alt="Stats dashboard demo">
</p>

## What it shows

- CPU, RAM, GPU, storage, and network metrics
- Amp subscription and Orb usage
- Codex weekly quota and token activity
- Four customizable world clocks

Stats reads usage through the installed Amp and Codex CLIs. It does not read or store their credentials.

## Install

Stats requires macOS 11 or newer and currently provides prebuilt releases for Apple Silicon Macs. [Amp](https://ampcode.com/) and the [Codex CLI](https://github.com/openai/codex) must also be installed and signed in.

### Homebrew (recommended)

Choose the package that matches how you want to run Stats. Although both packages are named `stats`, the `--cask` option selects the desktop app while the command without `--cask` selects the CLI formula.

#### Desktop app

This installs `Stats.app` in `/Applications`:

```sh
brew install --cask priyashpatil/tap/stats &&
  xattr -dr com.apple.quarantine /Applications/Stats.app
```

The `brew install --cask` command installs the desktop app. The separate `xattr` command allows macOS to open that app after Homebrew has downloaded and verified it.

#### CLI only

This installs the `stats` command in Homebrew's binary directory. It does not install `Stats.app`:

```sh
brew install priyashpatil/tap/stats
```

### Download

1. Open the [latest release](https://github.com/priyashpatil/stats/releases/latest).
2. Download the `Stats-...-macOS-arm64.zip` file for Apple Silicon.
3. Unzip `Stats.app`, move it to `/Applications`, and open it.

The downloaded app is self-contained and does not require Rust or Swift. Releases are ad-hoc signed but not Apple-notarized, so macOS may ask you to confirm the first launch in **System Settings → Privacy & Security**.

For a CLI-only installation, download `stats-...-macOS-arm64.tar.gz` from the same release, extract `stats`, and place it in a directory in your `PATH`, such as `~/.local/bin`.

## Getting started

1. Open Stats from your Applications folder. The dashboard appears and a Stats icon is added to the menu bar.
2. Use the menu bar icon to show or hide the dashboard, open **Settings**, or quit Stats.
3. In **Settings → General**, choose whether Stats should launch when you sign in.
4. In **Settings → Clocks**, search for a city or time zone for each of the four clocks.

You can move and resize the dashboard. Stats remembers its position and restores it on the primary desktop the next time it opens.

## Terminal use

If you prefer a terminal and did not install the Homebrew formula above, install the CLI with Cargo:

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

## Configuration

The terminal dashboard and macOS app share one human-editable TOML configuration file:

```text
~/.config/stats/config.toml
```

Stats respects `XDG_CONFIG_HOME` when it is set, so the complete default is `${XDG_CONFIG_HOME:-$HOME/.config}/stats/config.toml`. Print the active path with:

```sh
stats config path
```

Use a different file for one invocation with `stats --config /path/to/config.toml`. Command-line options override environment variables, which override values from the file. A missing file uses the built-in defaults; the macOS app creates it when you change a shared setting or choose **Settings → General → Open Configuration File**.

Restart a running dashboard after editing the file directly so it reloads the new values.

```toml
version = 1

[[clocks]]
label = "Mumbai"
timezone = "Asia/Kolkata"

[[clocks]]
label = "Paris"
timezone = "Europe/Paris"

[[clocks]]
label = "Sydney"
timezone = "Australia/Sydney"

[[clocks]]
label = "Seattle"
timezone = "America/Los_Angeles"

[refresh]
codex_seconds = 60
amp_seconds = 300
storage_seconds = 300

[desktop]
font_size = 15
```

The config requires four clocks with valid IANA time zone identifiers. `desktop.font_size` controls the embedded terminal in the macOS app and accepts values from 10 through 24. Launch-at-login and window placement remain native macOS settings.

## Privacy and security

- Amp and Codex requests are made through their installed CLIs; credentials are not copied into Stats.
- The Codex app server listens only on `127.0.0.1` while Stats is running.
- Parsed usage responses are cached in the operating system's user cache directory (`~/Library/Caches/stats` on macOS). On Unix systems, Stats restricts the directory to the current user (`0700`) and cache files to `0600`.

To report a security issue, see [SECURITY.md](SECURITY.md).

## Contributing

Want to build Stats from source or help improve it? See [CONTRIBUTING.md](CONTRIBUTING.md) for setup, testing, and release details.

## License

[MIT](LICENSE)
