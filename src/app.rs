use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::cache::prime_usage_caches;
use crate::cli::{command_exists, parse_args};
use crate::model::{Action, AppState, Args, Mode};
use crate::providers::amp::{spawn_refresh_amp, spawn_refresh_amp_activity};
use crate::providers::codex::{
    pick_port, run_codex_usage_status, shutdown_server, spawn_codex_client, start_codex_server,
    wait_ready,
};
use crate::system::{prime_system, spawn_refresh_system};
use crate::ui::{print_once, run_tui};

pub(crate) fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    if args.action == Action::ConfigPath {
        println!("{}", args.config_path.display());
        return Ok(());
    }
    match args.mode {
        Mode::Stats => {
            for command in ["amp", "codex"] {
                if !command_exists(command) {
                    return Err(format!("{command} not found in PATH"));
                }
            }
            run_stats(args)
        }
        Mode::CodexUsageStatus => {
            if !command_exists("codex") {
                return Err("codex not found in PATH".into());
            }
            run_codex_usage_status()
        }
    }
}

fn run_stats(args: Args) -> Result<(), String> {
    let port = pick_port()?;
    let mut codex_proc = start_codex_server(port)?;
    let stop = Arc::new(AtomicBool::new(false));
    let state = Arc::new(Mutex::new(AppState::default()));
    prime_usage_caches(&state);

    let result = (|| {
        wait_ready(port, &mut codex_proc)?;
        prime_system(&state);

        spawn_refresh_system(&state, &stop, args.storage_interval);
        spawn_refresh_amp(&state, &stop, args.amp_interval);
        spawn_refresh_amp_activity(&state, &stop, args.amp_interval);
        spawn_codex_client(&state, &stop, port, args.interval);

        if args.once {
            print_once(&state);
            Ok(())
        } else {
            run_tui(&state, &stop, &args.clocks)
        }
    })();

    stop.store(true, Ordering::Relaxed);
    shutdown_server(&mut codex_proc);
    result
}
