use std::env;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use crate::cache::prime_usage_caches;
use crate::cli::{command_exists, parse_args};
use crate::model::{Action, AppState, Args};
use crate::providers::amp::{spawn_refresh_amp, spawn_refresh_amp_activity};
use crate::system::{prime_system, spawn_refresh_system};
use crate::ui::{print_once, run_tui};

enum AppOutcome {
    Done,
    Reload,
}

pub(crate) fn main() {
    match run() {
        Ok(AppOutcome::Done) => {}
        Ok(AppOutcome::Reload) => reload_process(),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

#[cfg(unix)]
fn reload_process() -> ! {
    let executable = env::current_exe().unwrap_or_else(|err| {
        eprintln!("could not reload Stats: {err}");
        std::process::exit(1);
    });
    let error = Command::new(executable).args(env::args_os().skip(1)).exec();
    eprintln!("could not reload Stats: {error}");
    std::process::exit(1);
}

#[cfg(not(unix))]
fn reload_process() -> ! {
    eprintln!("automatic config reload is not supported on this platform");
    std::process::exit(1);
}

fn run() -> Result<AppOutcome, String> {
    let args = parse_args()?;
    if args.action == Action::ConfigPath {
        println!("{}", args.config_path.display());
        return Ok(AppOutcome::Done);
    }
    let amp_needed = args.section_display.amp_ai_needed(&args.sections)
        || args.section_display.amp_activity_needed(&args.sections);
    if amp_needed && !command_exists("amp") {
        return Err("amp not found in PATH".into());
    }
    run_stats(args)
}

fn run_stats(args: Args) -> Result<AppOutcome, String> {
    let system_needed = args.section_display.system_needed(&args.sections);
    let amp_ai_needed = args.section_display.amp_ai_needed(&args.sections);
    let amp_activity_needed = args.section_display.amp_activity_needed(&args.sections);
    let stop = Arc::new(AtomicBool::new(false));
    let state = Arc::new(Mutex::new(AppState::default()));
    prime_usage_caches(&state);

    let result = {
        if system_needed {
            prime_system(&state);
            spawn_refresh_system(&state, &stop, args.storage_interval);
        }

        if amp_ai_needed {
            spawn_refresh_amp(&state, &stop, args.amp_interval);
        }
        if amp_activity_needed {
            spawn_refresh_amp_activity(&state, &stop, args.amp_interval);
        }

        if args.once {
            print_once(&state, &args.sections, &args.section_display);
            Ok(AppOutcome::Done)
        } else {
            run_tui(&state, &stop, &args).map(|reload| {
                if reload {
                    AppOutcome::Reload
                } else {
                    AppOutcome::Done
                }
            })
        }
    };

    stop.store(true, Ordering::Relaxed);
    result
}
