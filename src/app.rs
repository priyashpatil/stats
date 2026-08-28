use std::env;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use crate::cache::prime_usage_caches;
use crate::cli::{command_exists, parse_args};
use crate::model::{Action, AppState, Args, Mode};
use crate::providers::amp::{spawn_refresh_amp, spawn_refresh_amp_activity};
use crate::providers::antigravity::{
    executable as antigravity_executable, spawn_refresh_antigravity,
};
use crate::providers::claude::spawn_refresh_claude;
use crate::providers::codex::{
    pick_port, run_codex_usage_status, shutdown_server, spawn_codex_client, start_codex_server,
    wait_ready,
};
use crate::providers::cursor::{executable as cursor_executable, spawn_refresh_cursor};
use crate::providers::grok::{executable as grok_executable, spawn_refresh_grok};
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
    match args.mode {
        Mode::Stats => {
            let amp_needed = args.section_display.amp_ai_needed(&args.sections)
                || args.section_display.amp_activity_needed(&args.sections);
            let codex_needed = args.section_display.codex_ai_needed(&args.sections)
                || args.section_display.codex_activity_needed(&args.sections);
            let claude_needed = args.section_display.claude_ai_needed(&args.sections);
            let antigravity_needed = args.section_display.antigravity_ai_needed(&args.sections);
            let cursor_needed = args.section_display.cursor_ai_needed(&args.sections);
            let grok_needed = args.section_display.grok_ai_needed(&args.sections);
            if amp_needed && !command_exists("amp") {
                return Err("amp not found in PATH".into());
            }
            if codex_needed && !command_exists("codex") {
                return Err("codex not found in PATH".into());
            }
            if claude_needed && !command_exists("claude") {
                return Err("claude not found in PATH".into());
            }
            if antigravity_needed && antigravity_executable().is_none() {
                return Err("agy not found".into());
            }
            if cursor_needed && cursor_executable().is_none() {
                return Err("Cursor agent not found".into());
            }
            if grok_needed && grok_executable().is_none() {
                return Err("grok not found".into());
            }
            run_stats(args)
        }
        Mode::CodexUsageStatus => {
            if !command_exists("codex") {
                return Err("codex not found in PATH".into());
            }
            run_codex_usage_status().map(|()| AppOutcome::Done)
        }
    }
}

fn run_stats(args: Args) -> Result<AppOutcome, String> {
    let system_needed = args.section_display.system_needed(&args.sections);
    let amp_ai_needed = args.section_display.amp_ai_needed(&args.sections);
    let codex_ai_needed = args.section_display.codex_ai_needed(&args.sections);
    let claude_ai_needed = args.section_display.claude_ai_needed(&args.sections);
    let antigravity_ai_needed = args.section_display.antigravity_ai_needed(&args.sections);
    let cursor_ai_needed = args.section_display.cursor_ai_needed(&args.sections);
    let grok_ai_needed = args.section_display.grok_ai_needed(&args.sections);
    let amp_activity_needed = args.section_display.amp_activity_needed(&args.sections);
    let codex_activity_needed = args.section_display.codex_activity_needed(&args.sections);
    let codex_enabled = codex_ai_needed || codex_activity_needed;
    let port = codex_enabled.then(pick_port).transpose()?;
    let mut codex_proc = port.map(start_codex_server).transpose()?;
    let stop = Arc::new(AtomicBool::new(false));
    let state = Arc::new(Mutex::new(AppState::default()));
    prime_usage_caches(&state);

    let result = (|| {
        if let (Some(port), Some(codex_proc)) = (port, codex_proc.as_mut()) {
            wait_ready(port, codex_proc)?;
        }
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
        if claude_ai_needed {
            spawn_refresh_claude(&state, &stop, args.claude_interval);
        }
        if antigravity_ai_needed {
            spawn_refresh_antigravity(&state, &stop, args.quota_interval);
        }
        if cursor_ai_needed {
            spawn_refresh_cursor(&state, &stop, args.quota_interval);
        }
        if grok_ai_needed {
            spawn_refresh_grok(&state, &stop, args.quota_interval);
        }
        if let Some(port) = port {
            spawn_codex_client(
                &state,
                &stop,
                port,
                args.interval,
                codex_ai_needed,
                codex_activity_needed,
            );
        }

        if args.once {
            print_once(&state, &args.sections, &args.section_display);
            Ok(AppOutcome::Done)
        } else {
            run_tui(
                &state,
                &stop,
                &args.clocks,
                &args.sections,
                &args.section_display,
                args.show_scrollbar,
                &args.config_path,
            )
            .map(|reload| {
                if reload {
                    AppOutcome::Reload
                } else {
                    AppOutcome::Done
                }
            })
        }
    })();

    stop.store(true, Ordering::Relaxed);
    if let Some(codex_proc) = codex_proc.as_mut() {
        shutdown_server(codex_proc);
    }
    result
}
