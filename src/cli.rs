use std::env;
use std::fs::{self};

use crate::model::{Args, Mode};

pub(crate) fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        mode: mode_from_argv0(),
        interval: env_u64("CODEX_USAGE_WATCH_INTERVAL", 60),
        once: false,
        amp_interval: env_u64("AMP_USAGE_WATCH_INTERVAL", 300),
        storage_interval: env_u64("CODEX_USAGE_STORAGE_INTERVAL", 300),
    };

    let mut iter = env::args().skip(1).peekable();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--codex-usage-status" => args.mode = Mode::CodexUsageStatus,
            "--once" => args.once = true,
            "-i" | "--interval" => args.interval = parse_next_u64(&mut iter, &arg)?,
            "--amp-interval" => args.amp_interval = parse_next_u64(&mut iter, &arg)?,
            "--storage-interval" => args.storage_interval = parse_next_u64(&mut iter, &arg)?,
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }

    if args.interval < 5 {
        return Err("interval must be an integer >= 5 seconds".into());
    }
    if args.amp_interval < 60 {
        return Err("amp interval must be an integer >= 60 seconds".into());
    }
    args.storage_interval = args.storage_interval.max(60);
    Ok(args)
}

fn mode_from_argv0() -> Mode {
    let command = env::args()
        .next()
        .and_then(|arg| {
            std::path::Path::new(&arg)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .unwrap_or_default();
    match command.as_str() {
        "codex-usage-status" => Mode::CodexUsageStatus,
        _ => Mode::Stats,
    }
}

pub(crate) fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn parse_next_u64<I>(iter: &mut std::iter::Peekable<I>, flag: &str) -> Result<u64, String>
where
    I: Iterator<Item = String>,
{
    iter.next()
        .ok_or_else(|| format!("{flag} requires a value"))?
        .parse::<u64>()
        .map_err(|_| format!("{flag} requires an integer value"))
}

fn print_help() {
    println!("Amp, Codex, and system stats");
    println!();
    println!("Options:");
    println!("      --codex-usage-status");
    println!("  -i, --interval <seconds>");
    println!("      --once");
    println!("      --amp-interval <seconds>");
    println!("      --storage-interval <seconds>");
    println!("  -h, --help");
}

pub(crate) fn command_exists(name: &str) -> bool {
    env::var_os("PATH").is_some_and(|paths| {
        env::split_paths(&paths).any(|path| {
            let candidate = path.join(name);
            candidate.is_file() && is_executable(&candidate)
        })
    })
}

#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &std::path::Path) -> bool {
    path.exists()
}
