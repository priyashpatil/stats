# Stats

Stats is a lightweight dashboard for your Mac's system health and AI coding usage. Use it as a native menu bar app or run it directly in a terminal.

<p align="center">
  <img src="https://cdn.priyashpatil.com/products/stats-app-22-08-26.gif" alt="Stats dashboard demo">
</p>

## What it shows

- CPU, RAM, GPU, storage, and network metrics
- Amp subscription, Orb usage/runtime, and individual credit balance
- Antigravity model-group quotas
- Claude five-hour, weekly, and model-specific subscription quotas
- Codex weekly quota and token activity
- Cursor plan quota
- Grok Build subscription quota
- Four customizable world clocks

Stats reads usage through installed, signed-in coding CLIs. It never stores provider credentials. Cursor's CLI supplies a short-lived access token in memory for its account usage request; the other integrations query their CLIs directly.

## Install

Stats requires macOS 11 or newer and currently provides prebuilt releases for Apple Silicon Macs. Enable only the providers whose signed-in CLIs are installed: [Amp](https://ampcode.com/), [Antigravity](https://antigravity.google/docs/cli/overview), [Claude Code](https://code.claude.com/docs/en/overview), [Codex](https://github.com/openai/codex), [Cursor](https://cursor.com/docs/cli/overview), and [Grok Build](https://docs.x.ai/build/overview).

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
4. In **Settings → Sections**, choose which dashboard sections to display.
5. In **Settings → Clocks**, search for a city or time zone for each of the four clocks.

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

Run `stats --help` to see all CLI options. Enabled provider controls require their corresponding CLIs to be installed and signed in.

Amp percentage meters show the remaining quota reported by `amp usage`. The non-Orb allowance is labeled with the subscription name, such as **Megawatt**, while **Amp Orbs** identifies the Orb allowance. Stats also shows renewal/reset text, Orb runtime, and the individual credit balance when Amp provides them. Orb runtime and credits are exact supplemental values rather than percentage charts because Amp does not report a corresponding limit for either value. Previously fetched Amp values show their last-updated time until a fresh CLI response is available.

Claude meters show the remaining quota derived from the plan limits reported by `claude -p "/usage"`. Stats displays the five-hour and seven-day limits plus any model-specific weekly limit Claude reports. The command runs in safe mode without model turns, user customizations, or session persistence. Claude's machine-readable response currently wraps human-formatted limit rows, so Stats tolerates optional rows and keeps its last parsed values if a later response cannot be read.

Antigravity quota uses the official `agy` CLI's embedded local quota service. Stats reuses a running `agy` process when possible; otherwise it briefly starts the CLI in a private terminal, reads the Gemini and Claude/GPT five-hour and weekly buckets, and exits it. Cursor quota uses `agent status --format json` for authentication and requests the current plan period from Cursor's dashboard service. Grok quota uses the official `grok agent stdio` protocol and its `x.ai/billing` extension. These three integrations are opt-in because their CLIs and account plans are not present on every machine. Their upstream quota protocols may change, so Stats preserves the last successful value when a refresh fails.

The separate **Amp Activity** panel uses UTC-day ranges supported by `amp usage --details` to build a token calendar. A table compares covered/paid recorded cost, Orb runtime, model token usage, and source token usage across the most recent 1, 7, and 30 UTC days from the available cache. Its grid always fills the available width. Stats requests only the days needed by that visible grid, caches completed UTC days permanently, and refreshes only the current partial day. A persistent rolling limiter caps Stats at 40 account/activity lookups and 24 historical lookups per hour, leaving at least 20 of Amp's shared hourly allowance for other consumers. If Amp still returns a rate limit, Stats keeps cached data visible and pauses all lookups until Amp's retry window or the next locally calculated rolling-window slot. Amp and Codex activity remain separate datasets and charts.

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

A running dashboard watches the config file and reloads automatically after settings changes or direct edits.

```toml
version = 2

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

[sections]
clocks = true
system = true
ai = true
amp_activity = true
codex_activity = true

[section_display.clocks]
heading = true
clock_1 = true
clock_2 = true
clock_3 = true
clock_4 = true

[section_display.system]
heading = true
cpu = true
ram = true
gpu = true
storage = true
network = true

[section_display.ai]
heading = true
amp_plan = true
amp_orbs = true
amp_credits = true
codex_quota = true
claude_quota = true
antigravity_quota = false
cursor_quota = false
grok_quota = false

[section_display.amp_activity]
heading = true
calendar = true
daily_activity = true
usage_summary = true
models = true
sources = true
sync_alerts = true

[section_display.codex_activity]
heading = true
calendar = true
overview = true
daily_activity = true

[refresh]
codex_seconds = 60
amp_seconds = 300
claude_seconds = 300
quota_seconds = 300
storage_seconds = 300

[desktop]
font_size = 15
show_scrollbar = false
```

Version 2 requires the complete configuration shown above; older versions and omitted fields are rejected, except that pre-Claude version 2 files may omit `claude_quota` and `claude_seconds`. Claude stays disabled for those files until enabled in Settings; its omitted refresh interval defaults to 300 seconds. The config requires four clocks with valid IANA time zone identifiers. The `[sections]` flags independently control the Clocks, System, AI quota, Amp Activity, and Codex Activity sections. The corresponding `[section_display.*]` tables control their headings and individual rows or charts. An enabled section must have at least one display option enabled; a disabled section may retain any display choices. Data providers are not refreshed when none of their visible controls require them. `desktop.font_size` controls the embedded terminal in the macOS app and accepts values from 10 through 24. `desktop.show_scrollbar` controls the dashboard scrollbar. Launch-at-login and window placement remain native macOS settings.

## Privacy and security

- Amp, Claude, and Codex requests are made through their installed CLIs; credentials are not copied into Stats.
- Stats parses and caches account-level and UTC-day aggregate usage only; it does not retain the signed-in identity, thread titles, thread IDs, or per-thread details printed by Amp.
- The Codex app server listens only on `127.0.0.1` while Stats is running.
- Parsed usage responses are cached in the operating system's user cache directory (`~/Library/Caches/stats` on macOS). On Unix systems, Stats restricts the directory to the current user (`0700`) and cache files to `0600`.

To report a security issue, see [SECURITY.md](SECURITY.md).

## Contributing

Want to build Stats from source or help improve it? See [CONTRIBUTING.md](CONTRIBUTING.md) for setup, testing, and release details.

## License

[MIT](LICENSE)
