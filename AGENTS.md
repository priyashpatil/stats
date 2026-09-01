# Stats agent workflow

For every coding task in this repository, load the `developing-stats` skill before making changes.

Use `.agents/skills/developing-stats/scripts/local-app.sh` for local builds, tests, app runs, and status checks. After changing Rust or Swift code, run it with `run` before reporting completion. The `run` command calls `./develop.sh`, which builds both release targets, reinstalls the source app, and verifies the installed app and LaunchAgent. Do not replace it with direct `.build` launches.

Preserve unrelated worktree changes. Never remove a conflicting app from `/Applications` without the user's explicit approval.
