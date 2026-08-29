use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

use crate::model::{AppState, QuotaLimit, QuotaUsage};
use crate::providers::spawn_quota_refresh;

pub(crate) fn executable() -> Option<PathBuf> {
    resolve(
        "GROK_CLI_PATH",
        "grok",
        &[".grok/bin/grok", ".local/bin/grok"],
    )
}

pub(crate) fn spawn_refresh_grok(
    state: &Arc<Mutex<AppState>>,
    stop: &Arc<AtomicBool>,
    interval: u64,
) {
    spawn_quota_refresh(
        state,
        stop,
        interval,
        "grok",
        |state| &mut state.grok,
        || {
            executable()
                .ok_or_else(|| "grok not found".to_string())
                .and_then(read_usage)
        },
    );
}

fn read_usage(executable: PathBuf) -> Result<QuotaUsage, String> {
    let mut child = Command::new(executable)
        .args(["agent", "stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("could not start grok agent stdio: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "could not read grok stdout".to_string())?;
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = sender.send(line);
        }
    });
    let result = (|| {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "could not write grok stdin".to_string())?;
        send_rpc(
            stdin,
            1,
            "initialize",
            json!({
                "protocolVersion": "1",
                "clientCapabilities": {
                    "fs": {"readTextFile": false, "writeTextFile": false},
                    "terminal": false
                }
            }),
        )?;
        receive_rpc(&receiver, 1, Duration::from_secs(4))?;
        send_rpc(stdin, 2, "x.ai/billing", json!({}))?;
        let response = receive_rpc(&receiver, 2, Duration::from_secs(3))?;
        extract_usage(&response)
    })();
    drop(child.stdin.take());
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn send_rpc(stdin: &mut impl Write, id: u64, method: &str, params: Value) -> Result<(), String> {
    let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    writeln!(stdin, "{request}")
        .and_then(|()| stdin.flush())
        .map_err(|error| error.to_string())
}

fn receive_rpc(
    receiver: &mpsc::Receiver<String>,
    id: u64,
    timeout: Duration,
) -> Result<Value, String> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let line = receiver
            .recv_timeout(remaining)
            .map_err(|_| format!("grok RPC timed out waiting for request {id}"))?;
        let value: Value = serde_json::from_str(&line)
            .map_err(|error| format!("invalid grok RPC response: {error}"))?;
        if value.get("id").and_then(Value::as_u64) != Some(id) {
            continue;
        }
        if let Some(message) = value.pointer("/error/message").and_then(Value::as_str) {
            return Err(format!("grok: {message}"));
        }
        return value
            .get("result")
            .cloned()
            .ok_or_else(|| "grok RPC response did not include result".into());
    }
}

fn extract_usage(result: &Value) -> Result<QuotaUsage, String> {
    let config = result.get("config").unwrap_or(result);
    let used_percent = config
        .get("creditUsagePercent")
        .and_then(Value::as_f64)
        .or_else(|| {
            ratio_percent(
                config.pointer("/usage/totalUsed/val"),
                config.pointer("/monthlyLimit/val"),
            )
        })
        .or_else(|| {
            ratio_percent(
                config.pointer("/used/val"),
                config.pointer("/monthlyLimit/val"),
            )
        })
        .ok_or_else(|| "Grok billing response did not include quota usage".to_string())?;
    let period_type = config
        .pointer("/currentPeriod/type")
        .and_then(Value::as_str);
    let label = match period_type {
        Some(value) if value.contains("WEEKLY") => "Grok weekly",
        Some(value) if value.contains("MONTHLY") => "Grok monthly",
        _ => "Grok",
    };
    let reset = config
        .pointer("/currentPeriod/end")
        .or_else(|| config.pointer("/billingCycle/billingPeriodEnd"))
        .or_else(|| config.get("billingPeriodEnd"))
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(QuotaUsage {
        limits: vec![QuotaLimit {
            label: label.into(),
            used_percent: used_percent.clamp(0.0, 100.0),
            reset,
        }],
    })
}

fn ratio_percent(used: Option<&Value>, limit: Option<&Value>) -> Option<f64> {
    let used = number(used?)?;
    let limit = number(limit?)?;
    (limit > 0.0).then_some(used / limit * 100.0)
}

fn number(value: &Value) -> Option<f64> {
    value.as_f64().or_else(|| value.as_str()?.parse().ok())
}

fn resolve(env_name: &str, command: &str, home_paths: &[&str]) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(env_name)
        .map(PathBuf::from)
        .filter(|path| path.is_file())
    {
        return Some(path);
    }
    if let Some(path) = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|path| path.join(command))
            .find(|path| path.is_file())
    }) {
        return Some(path);
    }
    let home = dirs::home_dir()?;
    home_paths
        .iter()
        .map(|path| home.join(path))
        .find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_current_grok_billing() {
        let usage = extract_usage(&json!({
            "billingCycle": {"billingPeriodEnd": "2026-09-01T00:00:00Z"},
            "monthlyLimit": {"val": 1000},
            "usage": {"totalUsed": {"val": 425}}
        }))
        .unwrap();
        assert_eq!(usage.limits[0].used_percent, 42.5);
        assert_eq!(
            usage.limits[0].reset.as_deref(),
            Some("2026-09-01T00:00:00Z")
        );
    }
}
