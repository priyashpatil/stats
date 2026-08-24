# Stats agent workflow

For every coding task in this repository, load the `working-on-stats` skill before making changes.

After changing Rust or Swift code, run `./develop.sh` before reporting completion. This is the canonical development workflow: it builds both release targets, reinstalls the source app, and verifies the installed app and LaunchAgent. Do not replace it with separate `cargo build`, `swift build`, or direct `.build` launches.

Preserve unrelated worktree changes. Never remove a conflicting app from `/Applications` without the user's explicit approval.
