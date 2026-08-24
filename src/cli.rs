use std::env;
use std::fs::{self};
use std::path::PathBuf;

use crate::config::{self, Config};
use crate::model::{Action, Args, Clock, Mode};

pub(crate) fn parse_args() -> Result<Args, String> {
    let mut action = Action::Run;
    let mut mode = mode_from_argv0();
    let mut once = false;
    let mut config_path = config::default_path()?;
    let mut interval = None;
    let mut amp_interval = None;
    let mut storage_interval = None;

    let mut iter = env::args().skip(1).peekable();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "config" => {
                if iter.next().as_deref() != Some("path") {
                    return Err("expected `stats config path`".into());
                }
                action = Action::ConfigPath;
            }
            "--config" => {
                config_path = PathBuf::from(
                    iter.next()
                        .ok_or_else(|| "--config requires a path".to_string())?,
                );
            }
            "--codex-usage-status" => mode = Mode::CodexUsageStatus,
            "--once" => once = true,
            "-i" | "--interval" => interval = Some(parse_next_u64(&mut iter, &arg)?),
            "--amp-interval" => amp_interval = Some(parse_next_u64(&mut iter, &arg)?),
            "--storage-interval" => storage_interval = Some(parse_next_u64(&mut iter, &arg)?),
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }

    if action == Action::ConfigPath {
        return Ok(Args {
            action,
            mode,
            interval: 60,
            once,
            amp_interval: 300,
            storage_interval: 300,
            clocks: Config::default().clocks,
            config_path,
        });
    }

    let config = config::load(&config_path)?;
    let interval = interval
        .unwrap_or_else(|| env_u64("CODEX_USAGE_WATCH_INTERVAL", config.refresh.codex_seconds));
    let amp_interval = amp_interval
        .unwrap_or_else(|| env_u64("AMP_USAGE_WATCH_INTERVAL", config.refresh.amp_seconds));
    let storage_interval = storage_interval.unwrap_or_else(|| {
        env_u64(
            "CODEX_USAGE_STORAGE_INTERVAL",
            config.refresh.storage_seconds,
        )
    });
    let clocks = configured_clocks(config.clocks);

    if interval < 5 {
        return Err("interval must be an integer >= 5 seconds".into());
    }
    if amp_interval < 60 {
        return Err("amp interval must be an integer >= 60 seconds".into());
    }
    Ok(Args {
        action,
        mode,
        interval,
        once,
        amp_interval,
        storage_interval: storage_interval.max(60),
        clocks,
        config_path,
    })
}

fn configured_clocks(default: Vec<Clock>) -> Vec<Clock> {
    env::var("STATS_CLOCKS")
        .ok()
        .and_then(|value| serde_json::from_str::<Vec<Clock>>(&value).ok())
        .filter(|clocks| config::validate_clocks(clocks).is_ok())
        .unwrap_or(default)
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
    println!("Commands:");
    println!("      config path");
    println!();
    println!("Options:");
    println!("      --config <path>");
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
