---
name: developing-stats
description: Builds, tests, reinstalls, and checks the Stats source app without changing a production copy. Use for every coding task in Stats, local app restarts, or checking source and production process state.
compatibility: Requires macOS, Rust, Swift, and the command-line tools checked by develop.sh.
---

# Developing Stats

Use the bundled `scripts/local-app.sh` for every local build, test, app run, or
status check. It keeps the repository workflow behind one command and never
operates on the production app in `/Applications`.

## Safety boundary

- Treat `/Applications/Stats.app` as production. Never quit, replace, open, or
  delete it during development.
- Treat `~/Applications/Stats.app`, `~/.cargo/bin/stats`, and
  `~/Library/LaunchAgents/com.priyashpatil.stats.plist` as the source-development
  installation managed by this repository.
- Production and source installations cannot coexist because they use the same
  bundle identifier. If production is installed, report the conflict and stop;
  never remove it without the user's explicit approval.
- Restarting the source app is allowed when the user asks to build and start or
  run it. Never infer permission to restart production.
- After starting the source app, report the source and production process state.

## Commands

From the repository root:

```sh
.agents/skills/developing-stats/scripts/local-app.sh build
.agents/skills/developing-stats/scripts/local-app.sh test
.agents/skills/developing-stats/scripts/local-app.sh run
.agents/skills/developing-stats/scripts/local-app.sh status
```

`run` calls the canonical `develop.sh` workflow. It builds both release targets,
reinstalls only the source-development app, and verifies its signature,
LaunchAgent, and executable path before reporting both installations' status.

Before editing, inspect `git status --short` and preserve unrelated worktree
changes. After changing Rust or Swift code, always run `local-app.sh run` before
reporting completion. Use `test` for the Cargo and Swift test suites while
iterating.
