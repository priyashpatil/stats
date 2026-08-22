use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use regex::Regex;
use sysinfo::{Disks, Networks, System};

use crate::command::run_output;
use crate::model::{AppState, SystemMetrics};
use crate::worker::sleep_stop;

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

fn read_gpu_stats() -> Option<f64> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let Ok(output) = run_output(
        "ioreg",
        &["-r", "-c", "AGXAccelerator", "-d", "1"],
        Duration::from_secs(1),
    ) else {
        return None;
    };
    if let Some(percent) = regex_number(&output, r#""Device Utilization %"\s*=\s*([0-9.]+)"#) {
        return Some(percent);
    }
    [
        regex_number(&output, r#""Renderer Utilization %"\s*=\s*([0-9.]+)"#),
        regex_number(&output, r#""Tiler Utilization %"\s*=\s*([0-9.]+)"#),
    ]
    .into_iter()
    .flatten()
    .fold(None, |acc: Option<f64>, value| {
        Some(acc.map_or(value, |current| current.max(value)))
    })
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

pub(crate) fn spawn_refresh_system(
    state: &Arc<Mutex<AppState>>,
    stop: &Arc<AtomicBool>,
    storage_interval: u64,
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
        let mut gpu_percent = read_gpu_stats();
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

            if last_gpu_sample.elapsed() >= Duration::from_secs(2) {
                gpu_percent = read_gpu_stats();
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

pub(crate) fn prime_system(state: &Arc<Mutex<AppState>>) {
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
    let gpu_percent = read_gpu_stats();

    let mut guard = state.lock().unwrap();
    guard.system.cpu_percent = Some(if per_core.is_empty() {
        0.0
    } else {
        per_core.iter().sum::<f64>() / per_core.len() as f64
    });
    guard.system.load_avg = (load.one, load.five, load.fifteen);
    guard.system.gpu_percent = gpu_percent;
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
