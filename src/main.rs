use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Datelike, Days, Local, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use rand::Rng;
use ratatui::backend::CrosstermBackend;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, Terminal};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sysinfo::{Disks, Networks, System};
use tungstenite::{Message, client};

const CLOCKS: &[(&str, &str)] = &[
    ("Mumbai", "Asia/Kolkata"),
    ("Paris", "Europe/Paris"),
    ("Sydney", "Australia/Sydney"),
    ("Seattle", "America/Los_Angeles"),
];

const BAR_FILLED: &str = "━";
const BAR_EMPTY: &str = "·";

#[derive(Debug, Clone)]
struct Args {
    mode: Mode,
    interval: u64,
    once: bool,
    amp_interval: u64,
    privileged_gpu: bool,
    storage_interval: u64,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Mode {
    Stats,
    CodexUsageStatus,
}

#[derive(Debug, Clone, Default)]
struct ProviderState<T> {
    result: Option<T>,
    error: Option<String>,
    updated_at: Option<DateTime<Local>>,
    ready: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct AmpUsage {
    plan: Option<String>,
    other_percent_remaining: Option<f64>,
    reset: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct CodexActivityUsage {
    daily_usage_buckets: Option<Vec<CodexDailyUsageBucket>>,
    summary: Option<CodexActivitySummary>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct CodexDailyUsageBucket {
    start_date: String,
    tokens: u64,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct CodexActivitySummary {
    lifetime_tokens: Option<u64>,
    peak_daily_tokens: Option<u64>,
    longest_running_turn_sec: Option<u64>,
    current_streak_days: Option<u64>,
    longest_streak_days: Option<u64>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct DailyTokenUsage {
    date: NaiveDate,
    tokens: u64,
}

#[derive(Debug, Clone)]
struct SystemMetrics {
    cpu_percent: Option<f64>,
    load_avg: (f64, f64, f64),
    gpu_percent: Option<f64>,
    gpu_privileged: bool,
    ram_percent: f64,
    ram_used: u64,
    ram_total: u64,
    net_down_rate: Option<f64>,
    net_up_rate: Option<f64>,
    net_interface: String,
    storage_free: u64,
    storage_total: u64,
    storage_percent_free: f64,
    storage_updated_at: Instant,
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            cpu_percent: None,
            load_avg: (0.0, 0.0, 0.0),
            gpu_percent: None,
            gpu_privileged: false,
            ram_percent: 0.0,
            ram_used: 0,
            ram_total: 0,
            net_down_rate: None,
            net_up_rate: None,
            net_interface: "network".into(),
            storage_free: 0,
            storage_total: 0,
            storage_percent_free: 0.0,
            storage_updated_at: Instant::now(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct AppState {
    amp: ProviderState<AmpUsage>,
    codex: ProviderState<Value>,
    codex_activity: ProviderState<CodexActivityUsage>,
    system: SystemMetrics,
}

#[derive(Debug, Clone, Deserialize)]
struct CacheEnvelope<T> {
    cached_at: f64,
    result: T,
}

#[derive(Debug, Clone)]
struct AiQuotaRow {
    label: String,
    percent_left: f64,
    reset: Option<String>,
    suffix: Option<String>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
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

    let privileged_gpu = if args.privileged_gpu {
        if authorize_privileged_gpu() {
            true
        } else {
            eprintln!(
                "could not authorize sudo for privileged GPU sampling; falling back to ioreg"
            );
            false
        }
    } else {
        false
    };

    let result = (|| {
        wait_ready(port, &mut codex_proc)?;
        prime_system(&state, privileged_gpu);

        spawn_refresh_system(&state, &stop, args.storage_interval, privileged_gpu);
        spawn_refresh_amp(&state, &stop, args.amp_interval);
        spawn_codex_client(&state, &stop, port, args.interval);

        if args.once {
            print_once(&state);
            Ok(())
        } else {
            run_tui(&state, &stop)
        }
    })();

    stop.store(true, Ordering::Relaxed);
    shutdown_server(&mut codex_proc);
    result
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        mode: mode_from_argv0(),
        interval: env_u64("CODEX_USAGE_WATCH_INTERVAL", 60),
        once: false,
        amp_interval: env_u64("AMP_USAGE_WATCH_INTERVAL", 300),
        privileged_gpu: cfg!(target_os = "macos")
            && env::var("STATS_PRIVILEGED_GPU").is_ok_and(|value| value != "0"),
        storage_interval: env_u64("CODEX_USAGE_STORAGE_INTERVAL", 300),
    };

    let mut iter = env::args().skip(1).peekable();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--codex-usage-status" => args.mode = Mode::CodexUsageStatus,
            "--once" => args.once = true,
            "-i" | "--interval" => args.interval = parse_next_u64(&mut iter, &arg)?,
            "--amp-interval" => args.amp_interval = parse_next_u64(&mut iter, &arg)?,
            "--privileged-gpu" => args.privileged_gpu = true,
            "--no-privileged-gpu" => args.privileged_gpu = false,
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

fn env_u64(name: &str, default: u64) -> u64 {
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
    println!("      --privileged-gpu / --no-privileged-gpu");
    println!("      --storage-interval <seconds>");
    println!("  -h, --help");
}

fn command_exists(name: &str) -> bool {
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

fn pick_port() -> Result<u16, String> {
    let mut rng = rand::thread_rng();
    for _ in 0..20 {
        let port = 49152 + rng.gen_range(0..10_000);
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        if TcpListener::bind(addr).is_ok() {
            return Ok(port);
        }
    }
    Err("could not find a free local port".into())
}

fn start_codex_server(port: u16) -> Result<Child, String> {
    Command::new("codex")
        .args(["app-server", "--listen", &format!("ws://127.0.0.1:{port}")])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to start codex app-server: {err}"))
}

fn wait_ready(port: u16, proc: &mut Child) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{port}/readyz");
    for _ in 0..100 {
        if proc.try_wait().map_err(|err| err.to_string())?.is_some() {
            let mut err = String::new();
            if let Some(stderr) = proc.stderr.as_mut() {
                let _ = stderr.read_to_string(&mut err);
            }
            return Err(format!("codex app-server exited before ready\n{err}")
                .trim()
                .into());
        }
        let ready = ureq::get(&url)
            .timeout(Duration::from_millis(200))
            .call()
            .is_ok_and(|response| response.status() == 200);
        if ready {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err("codex app-server did not become ready".into())
}

fn shutdown_server(proc: &mut Child) {
    if proc.try_wait().ok().flatten().is_some() {
        return;
    }
    let _ = proc.kill();
    let _ = proc.wait();
}

fn run_output(command: &str, args: &[&str], _timeout: Duration) -> Result<String, String> {
    let output = Command::new(command)
        .args(args)
        .stderr(Stdio::null())
        .output()
        .map_err(|err| err.to_string())?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("stats")
}

fn cache_path(provider: &str) -> PathBuf {
    cache_dir().join(format!("{provider}-usage.json"))
}

fn read_usage_cache<T>(provider: &str, max_age: Option<u64>) -> Option<(T, DateTime<Local>)>
where
    T: for<'de> Deserialize<'de>,
{
    let payload = fs::read_to_string(cache_path(provider)).ok()?;
    let envelope: CacheEnvelope<T> = serde_json::from_str(&payload).ok()?;
    if let Some(max_age) = max_age {
        let now = epoch_seconds();
        if now - envelope.cached_at > max_age as f64 {
            return None;
        }
    }
    let updated_at = Local
        .timestamp_opt(envelope.cached_at as i64, 0)
        .single()
        .unwrap_or_else(Local::now);
    Some((envelope.result, updated_at))
}

fn write_usage_cache<T>(provider: &str, result: &T)
where
    T: Serialize,
{
    let directory = cache_dir();
    if fs::create_dir_all(&directory).is_err() {
        return;
    }
    set_private_directory_permissions(&directory);

    let path = cache_path(provider);
    let temp_path = directory.join(format!(
        ".{provider}-usage-{}-{}.tmp",
        std::process::id(),
        rand::thread_rng().r#gen::<u64>()
    ));
    let payload = json!({
        "cached_at": epoch_seconds(),
        "result": result,
    });
    if write_private_file(&temp_path, payload.to_string().as_bytes()).is_err() {
        let _ = fs::remove_file(temp_path);
        return;
    }
    if fs::rename(&temp_path, path).is_err() {
        let _ = fs::remove_file(temp_path);
    }
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) {}

fn write_private_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(contents)
}

fn epoch_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or_default()
}

fn prime_usage_caches(state: &Arc<Mutex<AppState>>) {
    if let Some((result, updated_at)) = read_usage_cache::<AmpUsage>("amp", None) {
        let mut state = state.lock().unwrap();
        state.amp.result = Some(result);
        state.amp.updated_at = Some(updated_at);
    }
    if let Some((result, updated_at)) = read_usage_cache::<Value>("codex", None) {
        let mut state = state.lock().unwrap();
        state.codex.result = Some(result);
        state.codex.updated_at = Some(updated_at);
    }
    if let Some((result, updated_at)) =
        read_usage_cache::<CodexActivityUsage>("codex-activity", None)
    {
        let mut state = state.lock().unwrap();
        state.codex_activity.result = Some(result);
        state.codex_activity.updated_at = Some(updated_at);
    }
}

fn load_cached_amp(state: &Arc<Mutex<AppState>>, error: String) {
    let cached = read_usage_cache::<AmpUsage>("amp", Some(env_u64("STATS_USAGE_CACHE_TTL", 600)))
        .or_else(|| read_usage_cache::<AmpUsage>("amp", None));
    let mut state = state.lock().unwrap();
    if let Some((result, updated_at)) = cached {
        state.amp.result = Some(result);
        state.amp.updated_at = Some(updated_at);
        state.amp.error = None;
    } else {
        state.amp.error = Some(error);
    }
}

fn load_cached_codex(state: &Arc<Mutex<AppState>>, error: String) {
    let cached = read_usage_cache::<Value>("codex", Some(env_u64("STATS_USAGE_CACHE_TTL", 600)))
        .or_else(|| read_usage_cache::<Value>("codex", None));
    let mut state = state.lock().unwrap();
    if let Some((result, updated_at)) = cached {
        state.codex.result = Some(result);
        state.codex.updated_at = Some(updated_at);
        state.codex.error = None;
    } else {
        state.codex.error = Some(error);
    }
}

fn load_cached_codex_activity(state: &Arc<Mutex<AppState>>, error: String) {
    let cached = read_usage_cache::<CodexActivityUsage>(
        "codex-activity",
        Some(env_u64("STATS_USAGE_CACHE_TTL", 600)),
    )
    .or_else(|| read_usage_cache::<CodexActivityUsage>("codex-activity", None));
    let mut state = state.lock().unwrap();
    if let Some((result, updated_at)) = cached {
        state.codex_activity.result = Some(result);
        state.codex_activity.updated_at = Some(updated_at);
        state.codex_activity.error = None;
    } else {
        state.codex_activity.error = Some(error);
    }
}

fn authorize_privileged_gpu() -> bool {
    if !cfg!(target_os = "macos") {
        return false;
    }
    Command::new("sudo")
        .arg("-v")
        .status()
        .is_ok_and(|status| status.success())
}

fn default_interface() -> String {
    if !cfg!(target_os = "macos") {
        return "network".into();
    }
    if let Ok(output) = run_output("route", &["-n", "get", "default"], Duration::from_secs(1)) {
        for line in output.lines().map(str::trim) {
            if let Some(value) = line.strip_prefix("interface:") {
                return value.trim().to_string();
            }
        }
    }
    "network".into()
}

fn read_gpu_stats_ioreg() -> (Option<f64>, bool) {
    if !cfg!(target_os = "macos") {
        return (None, false);
    }
    let Ok(output) = run_output(
        "ioreg",
        &["-r", "-c", "AGXAccelerator", "-d", "1"],
        Duration::from_secs(1),
    ) else {
        return (None, false);
    };
    if let Some(percent) = regex_number(&output, r#""Device Utilization %"\s*=\s*([0-9.]+)"#) {
        return (Some(percent), false);
    }
    let fallback = [
        regex_number(&output, r#""Renderer Utilization %"\s*=\s*([0-9.]+)"#),
        regex_number(&output, r#""Tiler Utilization %"\s*=\s*([0-9.]+)"#),
    ]
    .into_iter()
    .flatten()
    .fold(None, |acc: Option<f64>, value| {
        Some(acc.map_or(value, |current| current.max(value)))
    });
    (fallback, false)
}

fn read_gpu_stats_powermetrics() -> (Option<f64>, bool) {
    if !cfg!(target_os = "macos") {
        return (None, false);
    }
    let output = Command::new("sudo")
        .args([
            "-n",
            "powermetrics",
            "-n",
            "1",
            "-i",
            "1000",
            "-s",
            "gpu_power",
            "--show-process-gpu",
        ])
        .stderr(Stdio::null())
        .output();
    let Ok(output) = output else {
        return (None, false);
    };
    let text = String::from_utf8_lossy(&output.stdout);
    if let Some(percent) = regex_number(&text, r"GPU HW active residency:\s*([0-9.]+)%") {
        return (Some(percent), true);
    }
    if let Some(idle) = regex_number(&text, r"GPU idle residency:\s*([0-9.]+)%") {
        return (Some((100.0 - idle).clamp(0.0, 100.0)), true);
    }
    (None, true)
}

fn regex_number(text: &str, pattern: &str) -> Option<f64> {
    Regex::new(pattern)
        .ok()?
        .captures(text)?
        .get(1)?
        .as_str()
        .parse()
        .ok()
}

fn read_gpu_stats(privileged: bool) -> (Option<f64>, bool) {
    if privileged {
        let stats = read_gpu_stats_powermetrics();
        if stats.0.is_some() || stats.1 {
            return stats;
        }
    }
    read_gpu_stats_ioreg()
}

fn spawn_refresh_system(
    state: &Arc<Mutex<AppState>>,
    stop: &Arc<AtomicBool>,
    storage_interval: u64,
    privileged_gpu: bool,
) {
    let state = Arc::clone(state);
    let stop = Arc::clone(stop);
    thread::spawn(move || {
        let mut system = System::new_all();
        let mut networks = Networks::new_with_refreshed_list();
        let iface = default_interface();
        let mut previous_rx = 0;
        let mut previous_tx = 0;
        let mut previous_time = Instant::now();
        let (mut gpu_percent, mut gpu_privileged) = read_gpu_stats(privileged_gpu);
        let mut last_gpu_sample = Instant::now();

        while !stop.load(Ordering::Relaxed) {
            system.refresh_cpu_usage();
            system.refresh_memory();
            networks.refresh(true);

            let per_core = system
                .cpus()
                .iter()
                .map(|cpu| cpu.cpu_usage() as f64)
                .collect::<Vec<_>>();
            let cpu = if per_core.is_empty() {
                0.0
            } else {
                per_core.iter().sum::<f64>() / per_core.len() as f64
            };
            let load = System::load_average();
            let now = Instant::now();
            let (mut down_rate, mut up_rate) = (None, None);
            if let Some((_name, data)) = pick_network(&networks, &iface) {
                let rx = data.total_received();
                let tx = data.total_transmitted();
                let seconds = now.duration_since(previous_time).as_secs_f64();
                if seconds > 0.0 && (previous_rx > 0 || previous_tx > 0) {
                    down_rate = Some(rx.saturating_sub(previous_rx) as f64 / seconds);
                    up_rate = Some(tx.saturating_sub(previous_tx) as f64 / seconds);
                }
                previous_rx = rx;
                previous_tx = tx;
            }
            previous_time = now;

            if last_gpu_sample.elapsed() >= Duration::from_secs(if privileged_gpu { 5 } else { 2 })
            {
                (gpu_percent, gpu_privileged) = read_gpu_stats(privileged_gpu);
                last_gpu_sample = Instant::now();
            }

            let mut guard = state.lock().unwrap();
            if guard.system.storage_total == 0
                || guard.system.storage_updated_at.elapsed()
                    >= Duration::from_secs(storage_interval)
            {
                update_storage(&mut guard.system);
            }
            guard.system.cpu_percent = Some(cpu);
            guard.system.load_avg = (load.one, load.five, load.fifteen);
            guard.system.gpu_percent = gpu_percent;
            guard.system.gpu_privileged = gpu_privileged;
            guard.system.ram_total = system.total_memory();
            guard.system.ram_used = system.used_memory();
            guard.system.ram_percent = if guard.system.ram_total > 0 {
                guard.system.ram_used as f64 / guard.system.ram_total as f64 * 100.0
            } else {
                0.0
            };
            guard.system.net_down_rate = down_rate;
            guard.system.net_up_rate = up_rate;
            guard.system.net_interface = iface.clone();
            drop(guard);

            sleep_stop(&stop, Duration::from_secs(1));
        }
    });
}

fn pick_network<'a>(
    networks: &'a Networks,
    iface: &str,
) -> Option<(&'a String, &'a sysinfo::NetworkData)> {
    networks
        .iter()
        .find(|(name, _)| name.as_str() == iface)
        .or_else(|| networks.iter().next())
}

fn update_storage(system: &mut SystemMetrics) {
    let disks = Disks::new_with_refreshed_list();
    let root = disks
        .iter()
        .find(|disk| disk.mount_point().to_string_lossy() == "/")
        .or_else(|| disks.iter().next());
    if let Some(disk) = root {
        system.storage_free = disk.available_space();
        system.storage_total = disk.total_space();
        system.storage_percent_free = if system.storage_total > 0 {
            system.storage_free as f64 / system.storage_total as f64 * 100.0
        } else {
            0.0
        };
        system.storage_updated_at = Instant::now();
    }
}

fn prime_system(state: &Arc<Mutex<AppState>>, privileged_gpu: bool) {
    let mut system = System::new_all();
    system.refresh_cpu_usage();
    thread::sleep(Duration::from_millis(250));
    system.refresh_cpu_usage();
    system.refresh_memory();
    let per_core = system
        .cpus()
        .iter()
        .map(|cpu| cpu.cpu_usage() as f64)
        .collect::<Vec<_>>();
    let load = System::load_average();
    let (gpu_percent, gpu_privileged) = read_gpu_stats(privileged_gpu);

    let mut guard = state.lock().unwrap();
    guard.system.cpu_percent = Some(if per_core.is_empty() {
        0.0
    } else {
        per_core.iter().sum::<f64>() / per_core.len() as f64
    });
    guard.system.load_avg = (load.one, load.five, load.fifteen);
    guard.system.gpu_percent = gpu_percent;
    guard.system.gpu_privileged = gpu_privileged;
    guard.system.ram_total = system.total_memory();
    guard.system.ram_used = system.used_memory();
    guard.system.ram_percent = if guard.system.ram_total > 0 {
        guard.system.ram_used as f64 / guard.system.ram_total as f64 * 100.0
    } else {
        0.0
    };
    guard.system.net_interface = default_interface();
    update_storage(&mut guard.system);
}

fn spawn_codex_client(
    state: &Arc<Mutex<AppState>>,
    stop: &Arc<AtomicBool>,
    port: u16,
    interval: u64,
) {
    let state = Arc::clone(state);
    let stop = Arc::clone(stop);
    thread::spawn(move || {
        if let Err(err) = codex_client(&state, &stop, port, interval) {
            load_cached_codex(&state, err.clone());
            load_cached_codex_activity(&state, err);
        }
    });
}

fn codex_client(
    state: &Arc<Mutex<AppState>>,
    stop: &Arc<AtomicBool>,
    port: u16,
    interval: u64,
) -> Result<(), String> {
    let url = format!("ws://127.0.0.1:{port}");
    let stream = TcpStream::connect(("127.0.0.1", port)).map_err(|err| err.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_millis(250)))
        .map_err(|err| err.to_string())?;
    let (mut ws, _) = client(&url, stream).map_err(|err| err.to_string())?;

    let mut next_id = 1u64;
    let initialize_id = send_ws(
        &mut ws,
        &mut next_id,
        "initialize",
        Some(json!({
            "clientInfo": {"name": "stats", "title": null, "version": env!("CARGO_PKG_VERSION")},
            "capabilities": {"experimentalApi": true, "optOutNotificationMethods": []}
        })),
    )?;
    let mut initialized = false;
    let mut pending_rate_id = None;
    let mut pending_activity_id = None;
    let mut next_rate_refresh_at = Instant::now();
    let mut next_activity_refresh_at = Instant::now();

    while !stop.load(Ordering::Relaxed) {
        if initialized && pending_rate_id.is_none() && Instant::now() >= next_rate_refresh_at {
            pending_rate_id = Some(send_ws(
                &mut ws,
                &mut next_id,
                "account/rateLimits/read",
                None,
            )?);
        }
        if initialized
            && pending_activity_id.is_none()
            && Instant::now() >= next_activity_refresh_at
        {
            pending_activity_id = Some(send_ws(&mut ws, &mut next_id, "account/usage/read", None)?);
        }

        let raw = match ws.read() {
            Ok(Message::Text(text)) => text.to_string(),
            Ok(Message::Binary(data)) => String::from_utf8_lossy(&data).to_string(),
            Ok(_) => continue,
            Err(tungstenite::Error::Io(err))
                if err.kind() == io::ErrorKind::WouldBlock
                    || err.kind() == io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(err) => return Err(err.to_string()),
        };

        let message: Value = serde_json::from_str(&raw).map_err(|err| err.to_string())?;
        let message_id = message.get("id").and_then(Value::as_u64);
        if message_id == Some(initialize_id) {
            if let Some(error) = message.get("error") {
                return Err(error_message(error));
            }
            initialized = true;
            let mut guard = state.lock().unwrap();
            guard.codex.ready = true;
            guard.codex_activity.ready = true;
            next_rate_refresh_at = Instant::now();
            next_activity_refresh_at = Instant::now();
            continue;
        }
        if Some(message_id.unwrap_or_default()) == pending_rate_id {
            pending_rate_id = None;
            if let Some(error) = message.get("error") {
                load_cached_codex(state, error_message(error));
            } else if let Some(result) = message.get("result").cloned() {
                write_usage_cache("codex", &result);
                let mut guard = state.lock().unwrap();
                guard.codex.error = None;
                guard.codex.result = Some(result);
                guard.codex.updated_at = Some(Local::now());
            }
            next_rate_refresh_at = Instant::now() + Duration::from_secs(interval);
            continue;
        }
        if Some(message_id.unwrap_or_default()) == pending_activity_id {
            pending_activity_id = None;
            if let Some(error) = message.get("error") {
                load_cached_codex_activity(state, error_message(error));
            } else if let Some(result) = message.get("result").cloned() {
                match serde_json::from_value::<CodexActivityUsage>(result) {
                    Ok(result) => {
                        write_usage_cache("codex-activity", &result);
                        let mut guard = state.lock().unwrap();
                        guard.codex_activity.error = None;
                        guard.codex_activity.result = Some(result);
                        guard.codex_activity.updated_at = Some(Local::now());
                    }
                    Err(err) => load_cached_codex_activity(state, err.to_string()),
                }
            }
            next_activity_refresh_at = Instant::now() + Duration::from_secs(interval);
        }
    }
    let _ = ws.close(None);
    Ok(())
}

fn send_ws<S: Read + Write>(
    ws: &mut tungstenite::WebSocket<S>,
    next_id: &mut u64,
    method: &str,
    params: Option<Value>,
) -> Result<u64, String> {
    let id = *next_id;
    *next_id += 1;
    let request = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params
    });
    ws.send(Message::Text(request.to_string().into()))
        .map_err(|err| err.to_string())?;
    Ok(id)
}

fn error_message(error: &Value) -> String {
    error
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| error.to_string())
}

fn run_codex_usage_status() -> Result<(), String> {
    let port = pick_port()?;
    let mut proc = start_codex_server(port)?;
    let result = (|| {
        wait_ready(port, &mut proc)?;
        let usage = read_codex_rate_limits_once(port, Duration::from_secs(15))?;
        print_codex_usage_status(&usage);
        Ok(())
    })();
    shutdown_server(&mut proc);
    result
}

fn read_codex_rate_limits_once(port: u16, timeout: Duration) -> Result<Value, String> {
    let url = format!("ws://127.0.0.1:{port}");
    let stream = TcpStream::connect(("127.0.0.1", port)).map_err(|err| err.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_millis(250)))
        .map_err(|err| err.to_string())?;
    let (mut ws, _) = client(&url, stream).map_err(|err| err.to_string())?;
    let mut next_id = 1u64;
    let initialize_id = send_ws(
        &mut ws,
        &mut next_id,
        "initialize",
        Some(json!({
            "clientInfo": {"name": "codex-usage-status", "title": null, "version": env!("CARGO_PKG_VERSION")},
            "capabilities": {"experimentalApi": true, "optOutNotificationMethods": []}
        })),
    )?;
    let started_at = Instant::now();
    let mut rate_limits_id = None;
    while started_at.elapsed() < timeout {
        let raw = match ws.read() {
            Ok(Message::Text(text)) => text.to_string(),
            Ok(Message::Binary(data)) => String::from_utf8_lossy(&data).to_string(),
            Ok(_) => continue,
            Err(tungstenite::Error::Io(err))
                if err.kind() == io::ErrorKind::WouldBlock
                    || err.kind() == io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(err) => return Err(err.to_string()),
        };
        let message: Value = serde_json::from_str(&raw).map_err(|err| err.to_string())?;
        let message_id = message.get("id").and_then(Value::as_u64);
        if message_id == Some(initialize_id) {
            if let Some(error) = message.get("error") {
                return Err(error_message(error));
            }
            rate_limits_id = Some(send_ws(
                &mut ws,
                &mut next_id,
                "account/rateLimits/read",
                None,
            )?);
            continue;
        }
        if message_id == rate_limits_id {
            if let Some(error) = message.get("error") {
                return Err(error_message(error));
            }
            let _ = ws.close(None);
            return message
                .get("result")
                .cloned()
                .ok_or_else(|| "Codex usage response did not include result".into());
        }
    }
    Err("timed out reading Codex usage status".into())
}

fn print_codex_usage_status(result: &Value) {
    println!("Codex usage status");
    for snapshot in ordered_buckets(result) {
        println!();
        println!("{}:", codex_limit_title(snapshot));
        if let Some(window) = codex_weekly_window(snapshot) {
            print_codex_status_window(window);
        }
        if let Some(credits) = snapshot.get("credits") {
            print_codex_status_credits(credits);
        }
        if let Some(state) = snapshot.get("rateLimitReachedType").and_then(Value::as_str) {
            println!("  State: {state}");
        }
    }
}

fn codex_limit_title(snapshot: &Value) -> String {
    snapshot
        .get("limitName")
        .and_then(Value::as_str)
        .or_else(|| snapshot.get("planType").and_then(Value::as_str))
        .map(|value| {
            if value == "prolite" {
                "Codex".to_string()
            } else {
                value.to_string()
            }
        })
        .unwrap_or_else(|| "Codex".into())
}

fn print_codex_status_window(window: &Value) {
    println!(
        "  {}: {:.0}% left{}",
        window_label_status(window),
        left_percent(window),
        reset_label_status(window)
    );
}

fn print_codex_status_credits(credits: &Value) {
    if credits
        .get("unlimited")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        println!("  Credits: unlimited");
        return;
    }
    let balance = credits
        .get("balance")
        .and_then(Value::as_str)
        .unwrap_or("0");
    let has_credits = credits
        .get("hasCredits")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if has_credits || balance != "0" {
        println!("  Credits: {balance}");
    }
}

fn window_label_status(window: &Value) -> String {
    let minutes = window.get("windowDurationMins").and_then(Value::as_i64);
    match minutes {
        Some(300) => "5h limit".into(),
        Some(10080) => "Weekly limit".into(),
        Some(value) if value % 60 == 0 => format!("{}h limit", value / 60),
        Some(value) => format!("{value}m limit"),
        None => "Limit".into(),
    }
}

fn reset_label_status(window: &Value) -> String {
    let Some(epoch) = window.get("resetsAt").and_then(Value::as_i64) else {
        return String::new();
    };
    let reset = Local
        .timestamp_opt(epoch, 0)
        .single()
        .unwrap_or_else(Local::now);
    if reset.date_naive() == Local::now().date_naive() {
        format!(
            " (resets {})",
            reset.format("%I:%M %p").to_string().to_lowercase()
        )
    } else {
        format!(
            " (resets {} on {})",
            reset.format("%I:%M %p").to_string().to_lowercase(),
            reset.format("%-d %b")
        )
    }
}

fn spawn_refresh_amp(state: &Arc<Mutex<AppState>>, stop: &Arc<AtomicBool>, interval: u64) {
    let state = Arc::clone(state);
    let stop = Arc::clone(stop);
    thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            match read_amp_usage() {
                Ok(result) => {
                    write_usage_cache("amp", &result);
                    let mut guard = state.lock().unwrap();
                    guard.amp.result = Some(result);
                    guard.amp.error = None;
                    guard.amp.updated_at = Some(Local::now());
                }
                Err(err) => load_cached_amp(&state, err),
            }
            sleep_stop(&stop, Duration::from_secs(interval));
        }
    });
}

fn read_amp_usage() -> Result<AmpUsage, String> {
    let output = run_output("amp", &["usage", "--no-color"], Duration::from_secs(12))?;
    extract_amp_usage(&output).ok_or_else(|| "could not read Amp usage".into())
}

fn extract_amp_usage(output: &str) -> Option<AmpUsage> {
    let cleaned = strip_ansi(output).replace("**", "");
    let subscription = Regex::new(
        r"(?i)(?:Amp\s+([^:\r\n]+?)\s+Subscription|Subscription\s+([^:\r\n]+)):\s*([0-9]+(?:\.[0-9]+)?)%\s+other usage and\s+[0-9]+(?:\.[0-9]+)?%\s+orb usage remaining(?:\s+-\s+([^\r\n]+))?",
    )
    .ok()?
    .captures(&cleaned)?;
    Some(AmpUsage {
        plan: subscription
            .get(1)
            .or_else(|| subscription.get(2))
            .map(|value| value.as_str().trim().to_string()),
        other_percent_remaining: subscription
            .get(3)
            .and_then(|value| value.as_str().parse().ok()),
        reset: subscription
            .get(4)
            .map(|value| value.as_str().trim().to_string()),
    })
}

fn strip_ansi(value: &str) -> String {
    Regex::new(r"\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x07]*(?:\x07|\x1b\\)|[ -/]*[@-~])")
        .unwrap()
        .replace_all(value, "")
        .to_string()
}

fn sleep_stop(stop: &Arc<AtomicBool>, duration: Duration) {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline && !stop.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(100));
    }
}
fn run_tui(state: &Arc<Mutex<AppState>>, stop: &Arc<AtomicBool>) -> Result<(), String> {
    enable_raw_mode().map_err(|err| err.to_string())?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        Clear(ClearType::Purge),
        Clear(ClearType::All),
        MoveTo(0, 0),
        Hide,
        EnableMouseCapture
    )
    .map_err(|err| err.to_string())?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|err| err.to_string())?;
    let result = loop {
        terminal
            .draw(|frame| draw(frame, state))
            .map_err(|err| err.to_string())?;
        if event::poll(Duration::from_millis(200)).map_err(|err| err.to_string())?
            && let Event::Key(key) = event::read().map_err(|err| err.to_string())?
            && matches!(
                key.code,
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc
            )
        {
            stop.store(true, Ordering::Relaxed);
            break Ok(());
        }
        if stop.load(Ordering::Relaxed) {
            break Ok(());
        }
    };
    disable_raw_mode().map_err(|err| err.to_string())?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        Show,
        LeaveAlternateScreen
    )
    .map_err(|err| err.to_string())?;
    terminal.show_cursor().map_err(|err| err.to_string())?;
    result
}

fn draw(frame: &mut Frame, state: &Arc<Mutex<AppState>>) {
    let area = frame.area();
    let snapshot = state.lock().unwrap().clone();
    let lines = stats_lines(&snapshot, area.width as usize);
    let paragraph = Paragraph::new(Text::from(lines));
    frame.render_widget(paragraph, area);
}

fn stats_lines(state: &AppState, width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    render_clocks(&mut lines, width);
    render_system(&mut lines, &state.system, width);
    render_ai(
        &mut lines,
        &state.amp,
        &state.codex,
        &state.codex_activity,
        width,
    );
    lines
}

fn render_clocks(lines: &mut Vec<Line<'static>>, width: usize) {
    let gap = if width >= 72 { 4 } else { 2 };
    let card_widths =
        equal_column_widths(width.saturating_sub(gap * (CLOCKS.len() - 1)), CLOCKS.len());
    if card_widths.contains(&0) {
        return;
    }
    let mut city_spans = Vec::new();
    let mut time_spans = Vec::new();
    let now = Local::now();
    for (index, ((city, zone_name), card_width)) in CLOCKS.iter().zip(card_widths).enumerate() {
        let zone: Tz = zone_name.parse().unwrap_or(chrono_tz::UTC);
        let zoned = now.with_timezone(&zone);
        if index > 0 {
            city_spans.push(Span::raw(" ".repeat(gap)));
            time_spans.push(Span::raw(" ".repeat(gap)));
        }
        city_spans.push(span(
            fixed(&city.to_uppercase(), card_width),
            Color::Cyan,
            true,
        ));
        time_spans.push(Span::styled(
            fixed(
                &zoned.format("%I:%M %p").to_string().to_lowercase(),
                card_width,
            ),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
    }
    lines.push(Line::from(city_spans));
    lines.push(Line::from(time_spans));
    lines.push(Line::default());
}

fn render_system(lines: &mut Vec<Line<'static>>, system: &SystemMetrics, width: usize) {
    let mut rows = vec![
        metric_row("CPU", system.cpu_percent, "used", true, width),
        metric_row("RAM", Some(system.ram_percent), "used", true, width),
        metric_row("GPU", system.gpu_percent, "used", true, width),
    ];
    let used_percent = (100.0 - system.storage_percent_free).clamp(0.0, 100.0);
    let color = color_for_usage(used_percent);
    let mut storage = vec![dim(fixed("Storage", 8))];
    let storage_value = format!("{:>3}% free", system.storage_percent_free.round() as i64);
    storage.extend(bar_spans(
        used_percent,
        metric_bar_width(width, 8, storage_value.chars().count()),
        color,
    ));
    storage.extend([
        Span::raw("  "),
        span(
            storage_value,
            color_for_remaining(system.storage_percent_free),
            true,
        ),
    ]);
    rows.push(Line::from(storage));
    rows.push(Line::from(vec![
        dim(fixed("Network", 8)),
        span(
            format!("↓ {}", rate_label(system.net_down_rate)),
            Color::Green,
            true,
        ),
        Span::raw("  "),
        span(
            format!("↑ {}", rate_label(system.net_up_rate)),
            Color::Green,
            true,
        ),
    ]));
    section(lines, "System", "", width);
    lines.push(Line::default());
    lines.extend(rows);
    lines.push(Line::default());
}

fn render_ai(
    lines: &mut Vec<Line<'static>>,
    amp: &ProviderState<AmpUsage>,
    codex: &ProviderState<Value>,
    codex_activity: &ProviderState<CodexActivityUsage>,
    width: usize,
) {
    render_ai_at(
        lines,
        amp,
        codex,
        codex_activity,
        width,
        Utc::now().date_naive(),
    );
}

fn render_ai_at(
    lines: &mut Vec<Line<'static>>,
    amp: &ProviderState<AmpUsage>,
    codex: &ProviderState<Value>,
    codex_activity: &ProviderState<CodexActivityUsage>,
    width: usize,
    today: NaiveDate,
) {
    section(lines, "AI", "", width);
    lines.push(Line::default());

    let mut rows = Vec::new();
    let mut statuses = Vec::new();
    collect_amp_ai_rows(&mut rows, &mut statuses, amp);
    collect_codex_ai_rows(&mut rows, &mut statuses, codex);

    for status in statuses {
        lines.push(status);
    }

    if !rows.is_empty() {
        render_ai_quota_rows(lines, rows, width);
    }
    lines.push(Line::default());
    render_codex_activity(lines, codex_activity, width, today);

    lines.push(Line::default());
}

fn collect_amp_ai_rows(
    rows: &mut Vec<AiQuotaRow>,
    statuses: &mut Vec<Line<'static>>,
    amp: &ProviderState<AmpUsage>,
) {
    if let Some(error) = &amp.error {
        statuses.push(ai_status_row(
            "Megawatt",
            format!("Error: {error}"),
            Color::Red,
        ));
        return;
    }
    let Some(result) = &amp.result else {
        statuses.push(ai_status_row(
            "Megawatt",
            "Loading Amp usage...",
            Color::Yellow,
        ));
        return;
    };
    if let Some(percent_left) = result.other_percent_remaining {
        rows.push(AiQuotaRow {
            label: result.plan.clone().unwrap_or_else(|| "Megawatt".into()),
            percent_left,
            reset: result.reset.as_deref().map(amp_compact_reset_label),
            suffix: None,
        });
    }
}

fn amp_compact_reset_label(reset: &str) -> String {
    reset
        .strip_prefix("resets upon renewal ")
        .unwrap_or(reset)
        .to_string()
}

fn collect_codex_ai_rows(
    rows: &mut Vec<AiQuotaRow>,
    statuses: &mut Vec<Line<'static>>,
    codex: &ProviderState<Value>,
) {
    if let Some(error) = &codex.error {
        statuses.push(ai_status_row(
            "Codex",
            format!("Error: {error}"),
            Color::Red,
        ));
        return;
    }
    let Some(result) = &codex.result else {
        statuses.push(ai_status_row(
            "Codex",
            "Loading Codex usage status...",
            Color::Yellow,
        ));
        return;
    };
    for snapshot in ordered_buckets(result) {
        if let Some(window) = codex_weekly_window(snapshot) {
            rows.push(AiQuotaRow {
                label: "Codex".into(),
                percent_left: left_percent(window),
                reset: Some(codex_compact_reset_label(window)),
                suffix: None,
            });
        }
    }
}

fn render_ai_quota_rows(lines: &mut Vec<Line<'static>>, rows: Vec<AiQuotaRow>, width: usize) {
    const VALUE_WIDTH: usize = 9;
    const LABEL_GAP: usize = 1;
    const VALUE_GAP: usize = 2;
    const TAIL_GAP: usize = 2;

    let tails = rows.iter().map(ai_quota_tail).collect::<Vec<_>>();
    let tail_width = tails
        .iter()
        .map(|tail| tail.chars().count())
        .max()
        .unwrap_or_default();
    let fixed_width = CODEX_GUTTER_WIDTH + LABEL_GAP + VALUE_GAP + VALUE_WIDTH;
    let available = width.saturating_sub(fixed_width);
    let show_tail = tail_width > 0 && available >= 4 + TAIL_GAP + tail_width;
    let bar_width = if show_tail {
        available.saturating_sub(TAIL_GAP + tail_width)
    } else {
        available
    };

    for (row, tail) in rows.into_iter().zip(tails) {
        if width < fixed_width {
            let label_width = CODEX_GUTTER_WIDTH.min(width);
            let value_width = width.saturating_sub(label_width);
            let color = color_for_remaining(row.percent_left);
            let mut spans = vec![dim(fixed(&row.label, label_width))];
            let full_value = format!("{}% left", row.percent_left.round() as i64);
            let compact_value = format!("{}%", row.percent_left.round() as i64);
            if value_width >= full_value.chars().count() {
                spans.push(span(fixed(&full_value, value_width), color, true));
            } else if value_width >= compact_value.chars().count() {
                spans.push(span(fixed(&compact_value, value_width), color, true));
            }
            lines.push(Line::from(spans));
            continue;
        }
        let color = color_for_remaining(row.percent_left);
        let mut spans = vec![
            dim(fixed(&row.label, CODEX_GUTTER_WIDTH)),
            Span::raw(" ".repeat(LABEL_GAP)),
        ];
        if bar_width >= 4 {
            spans.extend(bar_spans(row.percent_left, bar_width, color));
        }
        spans.push(Span::raw(" ".repeat(VALUE_GAP)));
        spans.push(span(
            format!("{:>3}% left", row.percent_left.round() as i64),
            color,
            true,
        ));
        if show_tail {
            spans.push(Span::raw(" ".repeat(TAIL_GAP)));
            spans.push(dim(fixed(&tail, tail_width)));
        }
        lines.push(Line::from(spans));
    }
}

fn ai_quota_tail(row: &AiQuotaRow) -> String {
    [row.suffix.as_deref(), row.reset.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ")
}

#[derive(Debug, Clone)]
struct ActivityCalendar {
    start_week: NaiveDate,
    weeks: usize,
    utc_today: NaiveDate,
    latest_date: NaiveDate,
    tokens_by_date: BTreeMap<NaiveDate, u64>,
    quartiles: [u64; 3],
    summary: Option<CodexActivitySummary>,
}

const CODEX_GUTTER_WIDTH: usize = 8;

fn render_codex_activity(
    lines: &mut Vec<Line<'static>>,
    activity: &ProviderState<CodexActivityUsage>,
    width: usize,
    utc_today: NaiveDate,
) {
    if activity_week_capacity(width) == 0 {
        return;
    }
    if let Some(error) = &activity.error {
        lines.push(ai_status_row(
            "Activity",
            format!("Error: {error}"),
            Color::Red,
        ));
        return;
    }
    let Some(result) = &activity.result else {
        lines.push(ai_status_row(
            "Activity",
            "Loading token activity...",
            Color::Yellow,
        ));
        return;
    };
    let Some(calendar) = activity_calendar(result, width, utc_today) else {
        lines.push(Line::from(vec![
            dim(fixed("Activity", CODEX_GUTTER_WIDTH)),
            dim("unavailable"),
        ]));
        return;
    };

    lines.push(activity_month_labels(&calendar));
    for day_offset in 0..7 {
        let mut spans = vec![dim(fixed(
            ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][day_offset],
            CODEX_GUTTER_WIDTH,
        ))];
        for week in 0..calendar.weeks {
            if week > 0 {
                spans.push(Span::raw(" "));
            }
            let date = calendar
                .start_week
                .checked_add_days(Days::new((week * 7 + day_offset) as u64))
                .unwrap_or(calendar.utc_today);
            if date > calendar.utc_today {
                spans.push(Span::raw(" "));
                continue;
            }
            let tokens = calendar.tokens_by_date.get(&date).copied().unwrap_or(0);
            if tokens == 0 {
                spans.push(dim("·"));
            } else {
                spans.push(span(
                    "▪",
                    activity_green(activity_intensity(tokens, calendar.quartiles)),
                    true,
                ));
            }
        }
        lines.push(Line::from(spans));
    }
    lines.push(Line::default());
    lines.extend(activity_overview_rows(&calendar, width));
    lines.push(Line::default());
    lines.extend(activity_daily_rows(&calendar, width));
}

fn activity_overview_rows(calendar: &ActivityCalendar, width: usize) -> Vec<Line<'static>> {
    let mut stats = vec![
        (
            "7 days",
            compact_token_count(activity_period_tokens(calendar, 7)),
        ),
        (
            "30 days",
            compact_token_count(activity_period_tokens(calendar, 30)),
        ),
    ];
    if let Some(summary) = calendar.summary {
        stats.extend(
            [
                summary
                    .lifetime_tokens
                    .map(|tokens| ("Total", compact_token_count(tokens))),
                summary
                    .peak_daily_tokens
                    .map(|tokens| ("Peak", compact_token_count(tokens))),
                summary
                    .current_streak_days
                    .map(|days| ("Streak", format!("{days}d"))),
                summary
                    .longest_streak_days
                    .map(|days| ("Best", format!("{days}d"))),
            ]
            .into_iter()
            .flatten(),
        );
    }
    let column_widths = equal_column_widths(width, stats.len());
    let mut headings = Vec::with_capacity(stats.len());
    let mut values = Vec::with_capacity(stats.len());
    for ((heading, value), column_width) in stats.into_iter().zip(column_widths) {
        headings.push(dim(centered(heading, column_width)));
        values.push(span(centered(&value, column_width), Color::Green, true));
    }
    vec![Line::from(headings), Line::from(values)]
}

fn activity_period_tokens(calendar: &ActivityCalendar, days: u64) -> u64 {
    let start = calendar
        .latest_date
        .checked_sub_days(Days::new(days.saturating_sub(1)))
        .unwrap_or(calendar.latest_date);
    calendar
        .tokens_by_date
        .range(start..=calendar.latest_date)
        .fold(0, |total, (_, tokens)| total.saturating_add(*tokens))
}

fn equal_column_widths(width: usize, count: usize) -> Vec<usize> {
    let base = width / count;
    let remainder = width % count;
    (0..count)
        .map(|index| base + usize::from(index < remainder))
        .collect()
}

fn activity_daily_rows(calendar: &ActivityCalendar, width: usize) -> Vec<Line<'static>> {
    if width < 7 {
        return Vec::new();
    }
    let cell_widths = equal_column_widths(width, 7);
    let first_date = calendar
        .latest_date
        .checked_sub_days(Days::new(6))
        .unwrap_or(calendar.latest_date);
    let dates = (0..7)
        .map(|offset| {
            first_date
                .checked_add_days(Days::new(offset))
                .unwrap_or(first_date)
        })
        .collect::<Vec<_>>();
    let mut date_spans = Vec::with_capacity(7);
    let mut value_spans = Vec::with_capacity(7);
    for (date, cell_width) in dates.into_iter().zip(cell_widths) {
        let date_label = if cell_width >= 6 {
            date.format("%b %-d").to_string()
        } else {
            date.day().to_string()
        };
        date_spans.push(dim(centered(&date_label, cell_width)));
        let tokens = calendar.tokens_by_date.get(&date).copied().unwrap_or(0);
        if tokens == 0 {
            value_spans.push(dim(centered("·", cell_width)));
        } else {
            let color = activity_green(activity_intensity(tokens, calendar.quartiles));
            value_spans.push(span(
                centered(&compact_token_count(tokens), cell_width),
                color,
                true,
            ));
        }
    }
    vec![Line::from(date_spans), Line::from(value_spans)]
}

fn centered(value: &str, width: usize) -> String {
    let value = value.chars().take(width).collect::<String>();
    let padding = width.saturating_sub(value.chars().count());
    let left = padding / 2;
    let right = padding - left;
    format!("{}{}{}", " ".repeat(left), value, " ".repeat(right))
}

fn compact_token_count(tokens: u64) -> String {
    const UNITS: &[&str] = &["", "K", "M", "B", "T"];
    let mut value = tokens as f64;
    let mut unit = 0;
    while value >= 1_000.0 && unit < UNITS.len() - 1 {
        value /= 1_000.0;
        unit += 1;
    }

    loop {
        let decimals = if unit == 0 || value >= 100.0 || value.fract() < 0.05 {
            0
        } else {
            1
        };
        let scale = if decimals == 0 { 1.0 } else { 10.0 };
        let rounded = (value * scale).round() / scale;
        if rounded >= 1_000.0 && unit < UNITS.len() - 1 {
            value /= 1_000.0;
            unit += 1;
            continue;
        }
        let decimals = if unit == 0 || rounded >= 100.0 || rounded.fract() < 0.05 {
            0
        } else {
            1
        };
        return format!("{value:.decimals$}{}", UNITS[unit]);
    }
}

fn activity_calendar(
    usage: &CodexActivityUsage,
    width: usize,
    utc_today: NaiveDate,
) -> Option<ActivityCalendar> {
    let tokens_by_date = daily_token_usage(usage)?
        .into_iter()
        .map(|bucket| (bucket.date, bucket.tokens))
        .collect::<BTreeMap<_, _>>();
    let latest_date = *tokens_by_date.keys().next_back()?;
    let current_week = sunday_of_week(utc_today);
    let weeks = activity_week_capacity(width);
    if weeks == 0 {
        return None;
    }
    let start_week = current_week
        .checked_sub_days(Days::new(((weeks - 1) * 7) as u64))
        .unwrap_or(current_week);
    let visible_nonzero = tokens_by_date
        .range(start_week..=utc_today)
        .map(|(_, tokens)| *tokens)
        .filter(|tokens| *tokens > 0)
        .collect::<Vec<_>>();

    Some(ActivityCalendar {
        start_week,
        weeks,
        utc_today,
        latest_date,
        tokens_by_date,
        quartiles: activity_quartiles(&visible_nonzero),
        summary: usage.summary,
    })
}

fn daily_token_usage(usage: &CodexActivityUsage) -> Option<Vec<DailyTokenUsage>> {
    Some(
        usage
            .daily_usage_buckets
            .as_ref()?
            .iter()
            .filter_map(|bucket| {
                NaiveDate::parse_from_str(&bucket.start_date, "%Y-%m-%d")
                    .ok()
                    .map(|date| DailyTokenUsage {
                        date,
                        tokens: bucket.tokens,
                    })
            })
            .collect(),
    )
}

fn activity_week_capacity(width: usize) -> usize {
    width.saturating_sub(CODEX_GUTTER_WIDTH).div_ceil(2)
}

fn sunday_of_week(date: NaiveDate) -> NaiveDate {
    date.checked_sub_days(Days::new(date.weekday().num_days_from_sunday() as u64))
        .unwrap_or(date)
}

fn activity_quartiles(values: &[u64]) -> [u64; 3] {
    if values.is_empty() {
        return [0; 3];
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let last = sorted.len() - 1;
    [sorted[last / 4], sorted[last / 2], sorted[last * 3 / 4]]
}

fn activity_intensity(tokens: u64, quartiles: [u64; 3]) -> usize {
    if tokens <= quartiles[0] {
        1
    } else if tokens <= quartiles[1] {
        2
    } else if tokens <= quartiles[2] {
        3
    } else {
        4
    }
}

fn activity_green(level: usize) -> Color {
    match level {
        1 => Color::Rgb(14, 68, 41),
        2 => Color::Rgb(0, 109, 50),
        3 => Color::Rgb(38, 166, 65),
        _ => Color::Rgb(57, 211, 83),
    }
}

fn activity_month_labels(calendar: &ActivityCalendar) -> Line<'static> {
    let grid_width = calendar.weeks * 2 - 1;
    let mut text = vec![' '; grid_width];
    let mut last_label_end = None;
    for week in 0..calendar.weeks {
        let week_start = calendar
            .start_week
            .checked_add_days(Days::new((week * 7) as u64))
            .unwrap_or(calendar.start_week);
        let visible_start = week_start;
        let week_end = week_start
            .checked_add_days(Days::new(6))
            .unwrap_or(week_start)
            .min(calendar.utc_today);
        if visible_start > week_end {
            continue;
        }
        let label_date = if week == 0 {
            Some(visible_start)
        } else {
            (0..7)
                .filter_map(|offset| week_start.checked_add_days(Days::new(offset)))
                .find(|date| *date >= visible_start && *date <= week_end && date.day() == 1)
        };
        let Some(label_date) = label_date else {
            continue;
        };
        let x = week * 2;
        let label = label_date.format("%b").to_string();
        if x + label.chars().count() > text.len() || last_label_end.is_some_and(|end| x <= end) {
            continue;
        }
        for (offset, ch) in label.chars().enumerate() {
            if x + offset < text.len() {
                text[x + offset] = ch;
            }
        }
        last_label_end = Some(x + label.len());
    }
    Line::from(vec![
        dim(fixed("", CODEX_GUTTER_WIDTH)),
        dim(text.into_iter().collect::<String>()),
    ])
}

fn ai_status_row(label: &str, message: impl Into<String>, color: Color) -> Line<'static> {
    Line::from(vec![
        dim(fixed(label, CODEX_GUTTER_WIDTH)),
        span(message.into(), color, true),
    ])
}

fn section(lines: &mut Vec<Line<'static>>, title: &str, meta: &str, width: usize) {
    let heading = title.to_uppercase();
    let mut spans = vec![span(heading.clone(), Color::Cyan, true)];
    let mut used = heading.len();
    if !meta.is_empty() {
        spans.push(dim(" "));
        spans.push(dim(meta.to_string()));
        used += meta.len() + 1;
    }
    spans.push(dim("  "));
    spans.push(dim("─".repeat(width.saturating_sub(used + 2))));
    lines.push(Line::from(spans));
}

fn metric_row(
    label: &str,
    percent: Option<f64>,
    suffix: &str,
    usage: bool,
    width: usize,
) -> Line<'static> {
    if let Some(percent) = percent {
        let color = if usage {
            color_for_usage(percent)
        } else {
            color_for_remaining(percent)
        };
        let value = format!("{:>3}% {suffix}", percent.round() as i64);
        let mut row = vec![dim(fixed(label, 8))];
        row.extend(bar_spans(
            percent,
            metric_bar_width(width, 8, value.chars().count()),
            color,
        ));
        row.extend([Span::raw("  "), span(value, color, true)]);
        Line::from(row)
    } else {
        Line::from(vec![dim(fixed(label, 8)), dim("sampling")])
    }
}

fn metric_bar_width(width: usize, label_width: usize, value_width: usize) -> usize {
    width.saturating_sub(label_width + 2 + value_width)
}

fn bar_spans(percent: f64, width: usize, color: Color) -> Vec<Span<'static>> {
    let filled = ((percent.clamp(0.0, 100.0) / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    vec![
        Span::styled(BAR_FILLED.repeat(filled), Style::default().fg(color)),
        dim(BAR_EMPTY.repeat(empty)),
    ]
}

fn ordered_buckets(result: &Value) -> Vec<&Value> {
    let Some(buckets) = result.get("rateLimitsByLimitId") else {
        return result
            .get("rateLimits")
            .map(|value| vec![value])
            .unwrap_or_default();
    };
    let mut values = Vec::new();
    if let Some(codex) = buckets.get("codex") {
        values.push(codex);
    }
    values
}

fn codex_weekly_window(snapshot: &Value) -> Option<&Value> {
    ["primary", "secondary"]
        .into_iter()
        .filter_map(|key| snapshot.get(key))
        .find(|window| window.get("windowDurationMins").and_then(Value::as_i64) == Some(10080))
}

fn left_percent(window: &Value) -> f64 {
    let used = window
        .get("usedPercent")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    (100.0 - used.round()).clamp(0.0, 100.0)
}

fn window_label(window: &Value) -> String {
    let minutes = window.get("windowDurationMins").and_then(Value::as_i64);
    match minutes {
        Some(300) => "5h".into(),
        Some(10080) => "Weekly".into(),
        Some(value) if value % 60 == 0 => format!("{}h", value / 60),
        Some(value) => format!("{value}m"),
        None => "Limit".into(),
    }
}

fn codex_compact_reset_label(window: &Value) -> String {
    let Some(epoch) = window.get("resetsAt").and_then(Value::as_i64) else {
        return "unknown".into();
    };
    let reset = Local
        .timestamp_opt(epoch, 0)
        .single()
        .unwrap_or_else(Local::now);
    if reset.date_naive() == Local::now().date_naive() {
        reset.format("%I:%M%P").to_string()
    } else {
        format!("{} {}", reset.format("%I:%M%P"), reset.format("%-d %b"))
    }
}

fn reset_label(window: &Value) -> String {
    let Some(epoch) = window.get("resetsAt").and_then(Value::as_i64) else {
        return "reset unknown".into();
    };
    let reset = Local
        .timestamp_opt(epoch, 0)
        .single()
        .unwrap_or_else(Local::now);
    if reset.date_naive() == Local::now().date_naive() {
        format!(
            "resets {}",
            reset.format("%I:%M %p").to_string().to_lowercase()
        )
    } else {
        format!(
            "resets {} {}",
            reset.format("%I:%M %p").to_string().to_lowercase(),
            reset.format("%-d %b")
        )
    }
}

fn color_for_remaining(percent: f64) -> Color {
    if percent <= 15.0 {
        Color::Red
    } else if percent <= 35.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

fn color_for_usage(percent: f64) -> Color {
    color_for_remaining(100.0 - percent)
}

fn rate_label(value: Option<f64>) -> String {
    let Some(mut value) = value else {
        return "sampling".into();
    };
    let units = ["B/s", "KB/s", "MB/s", "GB/s"];
    let mut index = 0;
    while value >= 1024.0 && index < units.len() - 1 {
        value /= 1024.0;
        index += 1;
    }
    let decimals = if value >= 10.0 || index == 0 { 0 } else { 1 };
    format!("{value:.decimals$} {}", units[index])
}

fn fixed(value: &str, width: usize) -> String {
    let clipped = value.chars().take(width).collect::<String>();
    format!("{clipped:<width$}")
}

fn span<T: Into<String>>(value: T, color: Color, bold: bool) -> Span<'static> {
    let mut style = Style::default().fg(color);
    if bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    Span::styled(value.into(), style)
}

fn dim<T: Into<String>>(value: T) -> Span<'static> {
    Span::styled(
        value.into(),
        Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
    )
}

fn print_once(state: &Arc<Mutex<AppState>>) {
    let started_at = Local::now();
    let deadline = Instant::now() + Duration::from_secs(25);
    while Instant::now() < deadline {
        let ready = {
            let state = state.lock().unwrap();
            provider_ready_for_once(&state.amp, &started_at)
                && provider_ready_for_once(&state.codex, &started_at)
        };
        if ready {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    let state = state.lock().unwrap().clone();
    println!("Stats");
    let gpu = state
        .system
        .gpu_percent
        .map(|value| format!("{value:.0}%"))
        .unwrap_or_else(|| "sampling".into());
    let gpu_source = if state.system.gpu_privileged {
        "powermetrics"
    } else {
        "ioreg"
    };
    println!(
        "CPU {:.0}% RAM {:.0}% GPU {gpu} ({gpu_source})",
        state.system.cpu_percent.unwrap_or_default(),
        state.system.ram_percent
    );
    println!(
        "Network down {} up {}",
        rate_label(state.system.net_down_rate),
        rate_label(state.system.net_up_rate)
    );
    println!("Storage {:.0}% free", state.system.storage_percent_free);
    if let Some(error) = &state.amp.error {
        println!("Amp error: {error}");
    } else if let Some(amp) = &state.amp.result
        && let Some(percent) = amp.other_percent_remaining
    {
        let reset = amp
            .reset
            .as_deref()
            .map(amp_compact_reset_label)
            .map(|reset| format!(" {reset}"))
            .unwrap_or_default();
        println!(
            "{} {:.0}% left{reset}",
            amp.plan.as_deref().unwrap_or("Megawatt"),
            percent
        );
    }
    if let Some(error) = &state.codex.error {
        println!("Codex error: {error}");
    } else if let Some(result) = &state.codex.result {
        for snapshot in ordered_buckets(result) {
            let plan = snapshot
                .get("planType")
                .and_then(Value::as_str)
                .map(|plan| if plan == "prolite" { "Pro" } else { plan })
                .unwrap_or("");
            println!("Codex {plan}");
            if let Some(window) = codex_weekly_window(snapshot) {
                println!(
                    "{} {:.0}% left {}",
                    window_label(window),
                    left_percent(window),
                    reset_label(window)
                );
            }
        }
    }
}

fn provider_ready_for_once<T>(provider: &ProviderState<T>, started_at: &DateTime<Local>) -> bool {
    if provider.error.is_some() {
        return true;
    }
    provider
        .updated_at
        .as_ref()
        .is_some_and(|updated_at| updated_at.signed_duration_since(*started_at).num_seconds() >= -1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(value: &str) -> NaiveDate {
        NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn restricts_usage_cache_to_the_current_user() {
        use std::os::unix::fs::PermissionsExt;

        let directory = env::temp_dir().join(format!(
            "stats-cache-permissions-{}-{}",
            std::process::id(),
            rand::thread_rng().r#gen::<u64>()
        ));
        fs::create_dir(&directory).unwrap();
        set_private_directory_permissions(&directory);
        let file = directory.join("usage.json");
        write_private_file(&file, b"{}").unwrap();

        assert_eq!(
            fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );

        fs::remove_dir_all(directory).unwrap();
    }

    fn activity(buckets: &[(&str, u64)]) -> CodexActivityUsage {
        CodexActivityUsage {
            daily_usage_buckets: Some(
                buckets
                    .iter()
                    .map(|(start_date, tokens)| CodexDailyUsageBucket {
                        start_date: (*start_date).into(),
                        tokens: *tokens,
                    })
                    .collect(),
            ),
            summary: None,
        }
    }

    fn activity_with_summary(
        buckets: &[(&str, u64)],
        summary: CodexActivitySummary,
    ) -> CodexActivityUsage {
        CodexActivityUsage {
            summary: Some(summary),
            ..activity(buckets)
        }
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().fold(String::new(), |mut text, span| {
            text.push_str(span.content.as_ref());
            text
        })
    }

    #[test]
    fn extracts_current_amp_megawatt_usage() {
        let output = "Signed in as user@example.com\n**Amp Megawatt Subscription:** 82% other usage and 64.5% orb usage remaining - resets upon renewal in 1 month\n**Individual credits:** $1.01 remaining\n";

        let usage = extract_amp_usage(output).expect("usage");

        assert_eq!(usage.plan.as_deref(), Some("Megawatt"));
        assert_eq!(usage.other_percent_remaining, Some(82.0));
        assert_eq!(
            usage.reset.as_deref(),
            Some("resets upon renewal in 1 month")
        );
    }

    #[test]
    fn extracts_historical_amp_megawatt_usage() {
        let output = "Subscription Megawatt: 82% other usage and 64.5% orb usage remaining - resets upon renewal in 1 month\n";

        let usage = extract_amp_usage(output).expect("usage");

        assert_eq!(usage.plan.as_deref(), Some("Megawatt"));
        assert_eq!(usage.other_percent_remaining, Some(82.0));
    }

    #[test]
    fn renders_clocks_as_equal_high_contrast_columns() {
        let mut lines = Vec::new();

        render_clocks(&mut lines, 58);

        assert_eq!(lines.len(), 3);
        assert_eq!(line_text(&lines[0]).chars().count(), 58);
        assert_eq!(line_text(&lines[1]).chars().count(), 58);
        assert!(line_text(&lines[0]).contains("MUMBAI"));
        assert!(line_text(&lines[0]).contains("SEATTLE"));
        assert!(
            lines[0]
                .spans
                .iter()
                .filter(|span| span.style.fg == Some(Color::Cyan))
                .count()
                == 4
        );
        assert!(
            lines[1]
                .spans
                .iter()
                .filter(|span| span.style.add_modifier.contains(Modifier::BOLD))
                .count()
                == 4
        );
    }

    #[test]
    fn selects_only_the_codex_weekly_window() {
        let snapshot = json!({
            "primary": {
                "usedPercent": 25,
                "windowDurationMins": 300
            },
            "secondary": {
                "usedPercent": 4,
                "windowDurationMins": 10080
            }
        });

        let weekly = codex_weekly_window(&snapshot).expect("weekly window");

        assert_eq!(weekly["windowDurationMins"], 10080);
        assert_eq!(weekly["usedPercent"], 4);
    }

    #[test]
    fn renders_only_one_codex_weekly_row() {
        let codex = ProviderState {
            result: Some(json!({
                "rateLimitsByLimitId": {
                    "codex": {
                        "primary": {
                            "resetsAt": 1784696828_i64,
                            "usedPercent": 25,
                            "windowDurationMins": 300
                        },
                        "secondary": {
                            "resetsAt": 1784696828_i64,
                            "usedPercent": 4,
                            "windowDurationMins": 10080
                        }
                    }
                }
            })),
            ..ProviderState::default()
        };
        let mut rows = Vec::new();
        let mut statuses = Vec::new();

        collect_codex_ai_rows(&mut rows, &mut statuses, &codex);

        assert!(statuses.is_empty());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label, "Codex");
        assert_eq!(rows[0].percent_left, 96.0);
    }

    #[test]
    fn keeps_the_weekly_row_within_narrow_widths() {
        let row = AiQuotaRow {
            label: "Codex".into(),
            percent_left: 96.0,
            reset: Some("09:48am 8 Aug".into()),
            suffix: None,
        };

        for width in [9, 10, 11, 18, 19] {
            let mut lines = Vec::new();
            render_ai_quota_rows(&mut lines, vec![row.clone()], width);
            assert_eq!(lines.len(), 1);
            assert!(line_text(&lines[0]).chars().count() <= width);
            if width >= 18 {
                assert!(line_text(&lines[0]).contains("96% left"));
            } else if width >= 11 {
                assert!(line_text(&lines[0]).contains("96%"));
            } else {
                assert!(!line_text(&lines[0]).contains("96"));
            }
        }
    }

    #[test]
    fn aligns_ai_quota_tracks_values_and_reset_columns() {
        let rows = vec![
            AiQuotaRow {
                label: "Megawatt".into(),
                percent_left: 100.0,
                reset: Some("in 1 day".into()),
                suffix: None,
            },
            AiQuotaRow {
                label: "Codex".into(),
                percent_left: 95.0,
                reset: Some("09:02am 27 Aug".into()),
                suffix: None,
            },
        ];
        let mut lines = Vec::new();

        render_ai_quota_rows(&mut lines, rows, 58);

        assert_eq!(lines.len(), 2);
        assert!(
            lines
                .iter()
                .all(|line| line_text(line).chars().count() == 58)
        );
        assert_eq!(
            lines[0].spans[2].content.chars().count() + lines[0].spans[3].content.chars().count(),
            lines[1].spans[2].content.chars().count() + lines[1].spans[3].content.chars().count()
        );
        let span_start = |line: &Line<'_>, index: usize| {
            line.spans[..index]
                .iter()
                .map(|span| span.content.chars().count())
                .sum::<usize>()
        };
        assert_eq!(span_start(&lines[0], 5), span_start(&lines[1], 5));
        assert_eq!(span_start(&lines[0], 7), span_start(&lines[1], 7));
    }

    #[test]
    fn separates_activity_from_the_codex_quota() {
        let amp = ProviderState {
            result: Some(AmpUsage {
                plan: Some("Megawatt".into()),
                other_percent_remaining: Some(82.0),
                reset: Some("resets upon renewal in 1 month".into()),
            }),
            ..ProviderState::default()
        };
        let codex = ProviderState {
            result: Some(json!({
                "rateLimitsByLimitId": {
                    "codex": {
                        "primary": {
                            "resetsAt": 1784696828_i64,
                            "usedPercent": 4,
                            "windowDurationMins": 10080
                        }
                    }
                }
            })),
            ..ProviderState::default()
        };
        let activity = ProviderState {
            result: Some(activity(&[("2026-07-12", 1), ("2026-08-02", 2)])),
            ..ProviderState::default()
        };
        let mut lines = Vec::new();

        render_ai_at(&mut lines, &amp, &codex, &activity, 40, date("2026-08-02"));
        let text = lines.iter().map(line_text).collect::<Vec<_>>();
        let megawatt = text
            .iter()
            .position(|line| line.contains("Megawatt"))
            .unwrap();
        let quota = text.iter().position(|line| line.contains("Codex")).unwrap();

        assert!(text[0].starts_with("AI"));
        assert!(megawatt < quota);
        assert!(text[quota + 1].is_empty());
        assert!(text[quota + 2].contains("Jul"));
        assert!(text[quota + 3].starts_with("Sun"));
    }

    #[test]
    fn anchors_calendar_to_the_utc_week_and_uses_every_column_that_fits() {
        let usage = activity(&[("2026-08-01", 20)]);

        let calendar = activity_calendar(&usage, 30, date("2026-08-05")).unwrap();

        assert_eq!(calendar.weeks, 11);
        assert_eq!(calendar.start_week, date("2026-05-24"));
        assert_eq!(sunday_of_week(calendar.utc_today), date("2026-08-02"));
    }

    #[test]
    fn places_activity_in_calendar_rows_and_marks_missing_past_dates() {
        let state = ProviderState {
            result: Some(activity(&[("2026-07-26", 10), ("2026-08-01", 20)])),
            ..ProviderState::default()
        };
        let mut lines = Vec::new();

        render_codex_activity(&mut lines, &state, 30, date("2026-08-01"));

        assert_eq!(lines.len(), 14);
        assert!(line_text(&lines[1]).ends_with('▪'));
        assert!(line_text(&lines[7]).ends_with('▪'));
        assert!(
            lines[2..7]
                .iter()
                .all(|line| line_text(line).ends_with('·'))
        );
        assert!(line_text(&lines[8]).is_empty());
        assert!(line_text(&lines[9]).contains("7 days"));
        assert!(line_text(&lines[10]).contains("30"));
        assert!(line_text(&lines[11]).is_empty());
        assert!(line_text(&lines[12]).contains("26"));
        assert!(line_text(&lines[12]).contains('1'));
        assert!(line_text(&lines[13]).contains("10"));
        assert!(line_text(&lines[13]).contains("20"));
        assert!(lines[2].spans.iter().any(|span| {
            span.content.as_ref() == "·" && span.style.add_modifier.contains(Modifier::DIM)
        }));
    }

    #[test]
    fn leaves_current_week_future_days_blank() {
        let state = ProviderState {
            result: Some(activity(&[("2026-07-12", 1), ("2026-08-05", 5)])),
            ..ProviderState::default()
        };
        let mut lines = Vec::new();

        render_codex_activity(&mut lines, &state, 30, date("2026-08-05"));

        assert!(line_text(&lines[4]).ends_with('▪'));
        assert!(line_text(&lines[5]).ends_with(' '));
        assert!(line_text(&lines[6]).ends_with(' '));
        assert!(line_text(&lines[7]).ends_with(' '));
    }

    #[test]
    fn labels_months_above_their_first_visible_weeks() {
        let usage = activity(&[("2026-07-01", 1), ("2026-08-02", 2)]);
        let calendar = activity_calendar(&usage, 40, date("2026-08-02")).unwrap();

        let labels = line_text(&activity_month_labels(&calendar));

        assert!(labels.contains("Jul"));
        assert!(labels.contains("Aug"));
        assert!(labels.find("Jul").unwrap() < labels.find("Aug").unwrap());

        let edge_usage = activity(&[("2026-08-01", 1)]);
        let edge_calendar = activity_calendar(&edge_usage, 40, date("2026-08-01")).unwrap();
        let edge_labels = line_text(&activity_month_labels(&edge_calendar));
        assert!(!edge_labels.trim_end().ends_with('A'));
    }

    #[test]
    fn uses_full_width_even_when_returned_history_is_shorter() {
        let usage = activity(&[("2026-06-01", 1), ("2026-08-02", 2)]);

        let calendar = activity_calendar(&usage, 16, date("2026-08-02")).unwrap();

        assert_eq!(calendar.weeks, 4);
        assert_eq!(calendar.start_week, date("2026-07-12"));
    }

    #[test]
    fn assigns_four_levels_from_visible_quartiles() {
        let quartiles = activity_quartiles(&[10, 20, 30, 40, 50, 60, 70, 80]);

        assert_eq!(quartiles, [20, 40, 60]);
        assert_eq!(activity_intensity(10, quartiles), 1);
        assert_eq!(activity_intensity(30, quartiles), 2);
        assert_eq!(activity_intensity(50, quartiles), 3);
        assert_eq!(activity_intensity(70, quartiles), 4);
    }

    #[test]
    fn selects_the_latest_bucket_and_summarizes_anchored_calendar_days() {
        let usage = activity(&[
            ("2026-08-01", 47_250_000),
            ("2026-07-02", 99_000_000),
            ("2026-07-03", 10_000_000),
            ("2026-07-26", 200_000_000),
            ("2026-08-01", 100_000_000),
            ("2026-07-31", 47_250_000),
        ]);
        let calendar = activity_calendar(&usage, 100, date("2026-08-02")).unwrap();

        assert_eq!(calendar.latest_date, date("2026-08-01"));
        assert_eq!(activity_period_tokens(&calendar, 1), 100_000_000);
        assert_eq!(activity_period_tokens(&calendar, 7), 347_250_000);
        assert_eq!(activity_period_tokens(&calendar, 30), 357_250_000);
        let summary = activity_overview_rows(&calendar, 51);
        assert_eq!(summary.len(), 2);
        assert_eq!(summary[0].spans[0].content.trim(), "7 days");
        assert_eq!(summary[0].spans[1].content.trim(), "30 days");
        assert_eq!(summary[1].spans[0].content.trim(), "347M");
        assert_eq!(summary[1].spans[1].content.trim(), "357M");
    }

    #[test]
    fn compacts_large_token_counts_and_truncates_summary_by_width() {
        assert_eq!(compact_token_count(999), "999");
        assert_eq!(compact_token_count(1_250), "1.2K");
        assert_eq!(compact_token_count(999_499), "999K");
        assert_eq!(compact_token_count(999_500), "1M");
        assert_eq!(compact_token_count(999_999_999), "1B");
        assert_eq!(compact_token_count(2_100_000_000), "2.1B");
    }

    #[test]
    fn includes_account_metrics_in_the_overview_columns() {
        let usage = activity_with_summary(
            &[("2026-08-01", 2)],
            CodexActivitySummary {
                lifetime_tokens: Some(12_300_000_000),
                peak_daily_tokens: Some(420_000_000),
                longest_running_turn_sec: Some(9_000),
                current_streak_days: Some(5),
                longest_streak_days: Some(23),
            },
        );
        let calendar = activity_calendar(&usage, 100, date("2026-08-02")).unwrap();

        let overview = activity_overview_rows(&calendar, 51);
        assert_eq!(overview.len(), 2);
        assert_eq!(overview[0].spans.len(), 6);
        assert_eq!(overview[0].spans[2].content.trim(), "Total");
        assert_eq!(overview[0].spans[3].content.trim(), "Peak");
        assert_eq!(overview[0].spans[4].content.trim(), "Streak");
        assert_eq!(overview[0].spans[5].content.trim(), "Best");
        assert_eq!(overview[1].spans[2].content.trim(), "12.3B");
        assert_eq!(overview[1].spans[3].content.trim(), "420M");
        assert_eq!(overview[1].spans[4].content.trim(), "5d");
        assert_eq!(overview[1].spans[5].content.trim(), "23d");
        assert_eq!(
            calendar.summary.unwrap().longest_running_turn_sec,
            Some(9_000)
        );
    }

    #[test]
    fn reads_bucket_only_and_enriched_activity_caches() {
        let cached: CodexActivityUsage = serde_json::from_value(json!({
            "dailyUsageBuckets": [{"startDate": "2026-08-01", "tokens": 2}]
        }))
        .unwrap();
        assert!(cached.summary.is_none());

        let enriched: CodexActivityUsage = serde_json::from_value(json!({
            "dailyUsageBuckets": [],
            "summary": {
                "lifetimeTokens": 12,
                "peakDailyTokens": 8,
                "longestRunningTurnSec": 90,
                "currentStreakDays": 3,
                "longestStreakDays": 7
            }
        }))
        .unwrap();
        assert_eq!(
            enriched.summary,
            Some(CodexActivitySummary {
                lifetime_tokens: Some(12),
                peak_daily_tokens: Some(8),
                longest_running_turn_sec: Some(90),
                current_streak_days: Some(3),
                longest_streak_days: Some(7),
            })
        );
    }

    #[test]
    fn renders_daily_dates_and_token_values_without_bars() {
        let usage = activity(&[
            ("2026-07-26", 10),
            ("2026-07-27", 20),
            ("2026-07-28", 30),
            ("2026-07-29", 40),
            ("2026-07-30", 50),
            ("2026-07-31", 60),
            ("2026-08-01", 70),
        ]);
        let calendar = activity_calendar(&usage, 51, date("2026-08-02")).unwrap();
        let rows = activity_daily_rows(&calendar, 51);

        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| line_text(row).chars().count() == 51));
        assert_eq!(rows[0].spans.len(), 7);
        assert_eq!(rows[0].spans[0].content.trim(), "Jul 26");
        assert_eq!(rows[0].spans[6].content.trim(), "Aug 1");
        assert_eq!(rows[1].spans[0].content.trim(), "10");
        assert_eq!(rows[1].spans[6].content.trim(), "70");
        assert_eq!(rows[1].spans[0].style.fg, Some(activity_green(1)));
        assert_eq!(rows[1].spans[6].style.fg, Some(activity_green(4)));
    }

    #[test]
    fn treats_null_buckets_as_unavailable_and_skips_malformed_dates() {
        let null_usage: CodexActivityUsage =
            serde_json::from_value(json!({"dailyUsageBuckets": null})).unwrap();
        assert!(activity_calendar(&null_usage, 40, date("2026-08-02")).is_none());

        let usage = activity(&[("not-a-date", 99), ("2026-08-02", 2)]);
        let calendar = activity_calendar(&usage, 40, date("2026-08-02")).unwrap();
        assert_eq!(calendar.latest_date, date("2026-08-02"));
        assert_eq!(calendar.tokens_by_date.len(), 1);
    }

    #[test]
    fn renders_missing_daily_usage_as_dim_dots() {
        let empty = activity(&[("2026-08-02", 0)]);
        let empty_calendar = activity_calendar(&empty, 22, date("2026-08-02")).unwrap();
        let rows = activity_daily_rows(&empty_calendar, 22);

        assert_eq!(rows.len(), 2);
        assert!(rows[1].spans.iter().all(|span| {
            span.content.contains('·') && span.style.add_modifier.contains(Modifier::DIM)
        }));
        assert!(activity_daily_rows(&empty_calendar, 6).is_empty());
    }
}
