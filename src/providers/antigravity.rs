use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde_json::Value;

use crate::model::{AppState, QuotaLimit, QuotaUsage};
use crate::providers::spawn_quota_refresh;

const QUOTA_PATH: &str = "/exa.language_server_pb.LanguageServerService/RetrieveUserQuotaSummary";

pub(crate) fn executable() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ANTIGRAVITY_CLI_PATH")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
    {
        return Some(path);
    }
    if let Some(path) = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|path| path.join("agy"))
            .find(|path| path.is_file())
    }) {
        return Some(path);
    }
    let home = dirs::home_dir()?;
    [
        home.join(".local/bin/agy"),
        PathBuf::from("/opt/homebrew/bin/agy"),
        PathBuf::from("/usr/local/bin/agy"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

pub(crate) fn spawn_refresh_antigravity(
    state: &Arc<Mutex<AppState>>,
    stop: &Arc<AtomicBool>,
    interval: u64,
) {
    spawn_quota_refresh(
        state,
        stop,
        interval,
        "antigravity",
        |state| &mut state.antigravity,
        read_usage,
    );
}

fn read_usage() -> Result<QuotaUsage, String> {
    if let Some(usage) = warm_process_ids()
        .into_iter()
        .find_map(|pid| fetch_from_pid(pid).ok())
    {
        return Ok(usage);
    }
    let executable = executable().ok_or_else(|| "agy not found".to_string())?;
    let process = ManagedAgy::start(&executable)?;
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut last_error = "agy did not expose a quota service".to_string();
    while Instant::now() < deadline {
        match fetch_from_pid(process.pid) {
            Ok(usage) => return Ok(usage),
            Err(error) => last_error = error,
        }
        thread::sleep(Duration::from_millis(200));
    }
    Err(last_error)
}

struct ManagedAgy {
    pid: u32,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    _master: Box<dyn MasterPty + Send>,
}

impl ManagedAgy {
    fn start(executable: &Path) -> Result<Self, String> {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 50,
                cols: 160,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("could not open agy terminal: {error}"))?;
        let mut command = CommandBuilder::new(executable);
        if let Some(home) = dirs::home_dir() {
            command.cwd(&home);
            command.env("PWD", home);
        }
        command.env("TERM", "xterm-256color");
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("could not start agy: {error}"))?;
        let pid = child
            .process_id()
            .ok_or_else(|| "could not determine agy process ID".to_string())?;
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| error.to_string())?;
        thread::spawn(move || {
            let mut sink = std::io::sink();
            let _ = std::io::copy(&mut reader, &mut sink);
        });
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            pid,
            child,
            writer,
            _master: pair.master,
        })
    }

    fn stop(&mut self) {
        let _ = self.writer.write_all(b"/exit\r");
        let _ = self.writer.flush();
        thread::sleep(Duration::from_millis(100));
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for ManagedAgy {
    fn drop(&mut self) {
        self.stop();
    }
}

fn warm_process_ids() -> Vec<u32> {
    ["agy", "antigravity-cli", "antigravity_cli"]
        .into_iter()
        .flat_map(|name| {
            Command::new("pgrep")
                .args(["-x", name])
                .output()
                .ok()
                .into_iter()
                .flat_map(|output| {
                    String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .filter_map(|line| line.trim().parse().ok())
                        .collect::<Vec<_>>()
                })
        })
        .collect()
}

fn fetch_from_pid(pid: u32) -> Result<QuotaUsage, String> {
    let ports = listening_ports(pid);
    if ports.is_empty() {
        return Err(format!("agy process {pid} has no listening quota service"));
    }
    let mut last_error = String::new();
    for port in ports {
        for scheme in ["https", "http"] {
            match fetch_quota(scheme, port) {
                Ok(usage) => return Ok(usage),
                Err(error) => last_error = error,
            }
        }
    }
    Err(last_error)
}

fn listening_ports(pid: u32) -> Vec<u16> {
    let output = Command::new("lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-a", "-p", &pid.to_string()])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_whitespace().find_map(port_from_address))
        .collect()
}

fn port_from_address(field: &str) -> Option<u16> {
    field.rsplit_once(':')?.1.parse().ok()
}

fn fetch_quota(scheme: &str, port: u16) -> Result<QuotaUsage, String> {
    let url = format!("{scheme}://127.0.0.1:{port}{QUOTA_PATH}");
    let output = Command::new("curl")
        .args([
            "-ksS",
            "--max-time",
            "2",
            "-H",
            "Content-Type: application/json",
            "-H",
            "Connect-Protocol-Version: 1",
            "--data",
            "{\"forceRefresh\":true}",
            &url,
        ])
        .output()
        .map_err(|error| format!("could not query agy: {error}"))?;
    if !output.status.success() {
        return Err(format!("agy quota request failed on port {port}"));
    }
    let response: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid agy quota response: {error}"))?;
    extract_usage(&response)
}

fn extract_usage(value: &Value) -> Result<QuotaUsage, String> {
    let groups = value
        .pointer("/response/groups")
        .or_else(|| value.get("groups"))
        .and_then(Value::as_array)
        .ok_or_else(|| "agy quota response did not include groups".to_string())?;
    let mut limits = Vec::new();
    for group in groups {
        let group_name = group
            .get("displayName")
            .and_then(Value::as_str)
            .unwrap_or("Antigravity");
        let Some(buckets) = group.get("buckets").and_then(Value::as_array) else {
            continue;
        };
        for bucket in buckets {
            if bucket.get("disabled").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            let remaining = remaining_fraction(bucket.get("remaining"));
            let Some(remaining) = remaining else { continue };
            let window = bucket
                .get("displayName")
                .and_then(Value::as_str)
                .unwrap_or("Quota");
            limits.push(QuotaLimit {
                label: format!("Agy {} {}", short_group(group_name), short_window(window)),
                used_percent: ((1.0 - remaining) * 100.0).clamp(0.0, 100.0),
                reset: bucket
                    .get("resetTime")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            });
        }
    }
    if limits.is_empty() {
        Err("agy quota response did not include usable limits".into())
    } else {
        Ok(QuotaUsage { limits })
    }
}

fn remaining_fraction(value: Option<&Value>) -> Option<f64> {
    let value = value?;
    value
        .get("remainingFraction")
        .and_then(Value::as_f64)
        .or_else(|| {
            (value.get("case").and_then(Value::as_str) == Some("remainingFraction"))
                .then(|| value.get("value").and_then(Value::as_f64))
                .flatten()
        })
}

fn short_group(value: &str) -> &str {
    if value.to_ascii_lowercase().contains("gemini") {
        "Gemini"
    } else {
        "Claude/GPT"
    }
}

fn short_window(value: &str) -> &str {
    let lower = value.to_ascii_lowercase();
    if lower.contains("week") {
        "7d"
    } else if lower.contains("five") || lower.contains("5") {
        "5h"
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_antigravity_quota_groups() {
        let usage = extract_usage(&json!({"response": {"groups": [{
            "displayName": "Gemini Models",
            "buckets": [{"displayName": "Weekly Limit", "remaining": {"remainingFraction": 0.82}, "resetTime": "2026-09-01T00:00:00Z"}]
        }]}})).unwrap();
        assert_eq!(usage.limits[0].label, "Agy Gemini 7d");
        assert!((usage.limits[0].used_percent - 18.0).abs() < 0.001);
    }

    #[test]
    fn extracts_lsof_listening_port() {
        assert_eq!(port_from_address("127.0.0.1:42123"), Some(42123));
        assert_eq!(port_from_address("TCP"), None);
    }
}
