use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Local;
use serde_json::{Value, json};

use crate::cache::{cli_working_dir, load_cached_grok, write_usage_cache};
use crate::model::{AppState, GrokUsage};
use crate::worker::sleep_stop;

pub(crate) fn spawn_refresh_grok(
    state: &Arc<Mutex<AppState>>,
    stop: &Arc<AtomicBool>,
    interval: u64,
) {
    let state = Arc::clone(state);
    let stop = Arc::clone(stop);
    thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            match read_usage() {
                Ok(result) => {
                    write_usage_cache("grok", &result);
                    let mut state = state.lock().unwrap();
                    state.grok.result = Some(result);
                    state.grok.error = None;
                    state.grok.updated_at = Some(Local::now());
                    state.grok.stale = false;
                }
                Err(error) => load_cached_grok(&state, error),
            }
            sleep_stop(&stop, Duration::from_secs(interval));
        }
    });
}

fn read_usage() -> Result<GrokUsage, String> {
    let mut child = Command::new("grok")
        .args(["--no-auto-update", "agent", "--no-leader", "stdio"])
        .current_dir(cli_working_dir("grok")?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("could not start grok: {error}"))?;
    let stdout = child.stdout.take().expect("piped stdout");
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let result = (|| {
        let stdin = child.stdin.as_mut().expect("piped stdin");
        send_rpc(
            stdin,
            1,
            "initialize",
            json!({
                "protocolVersion": 1,
                "clientCapabilities": {}
            }),
        )?;
        receive_rpc(&receiver, 1)?;
        send_rpc(stdin, 2, "_x.ai/billing", json!({}))?;
        extract_usage(&receive_rpc(&receiver, 2)?)
    })();
    drop(child.stdin.take());
    let _ = child.kill();
    let _ = child.wait();
    drop(receiver);
    let _ = reader.join();
    result
}

fn send_rpc(stdin: &mut impl Write, id: u64, method: &str, params: Value) -> Result<(), String> {
    let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    writeln!(stdin, "{request}")
        .and_then(|()| stdin.flush())
        .map_err(|error| error.to_string())
}

fn receive_rpc(receiver: &mpsc::Receiver<String>, id: u64) -> Result<Value, String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let line = receiver
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|error| format!("grok RPC request {id}: {error}"))?;
        let value: Value = serde_json::from_str(&line)
            .map_err(|error| format!("invalid grok RPC response: {error}"))?;
        if value.get("id").and_then(Value::as_u64) != Some(id) {
            continue;
        }
        if let Some(error) = value.get("error") {
            return Err(format!(
                "grok: {}",
                error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("RPC failed")
            ));
        }
        return value
            .get("result")
            .cloned()
            .ok_or_else(|| "grok RPC response did not include result".into());
    }
}

fn extract_usage(result: &Value) -> Result<GrokUsage, String> {
    let config = result
        .get("config")
        .filter(|config| config.is_object())
        .ok_or_else(|| "Grok billing response did not include config".to_string())?;
    let number = |value: &Value| value.as_f64().or_else(|| value.as_str()?.parse().ok());
    let used_percent = config
        .get("creditUsagePercent")
        .and_then(number)
        .or_else(|| {
            let limit = config.pointer("/monthlyLimit/val").and_then(number)?;
            if limit <= 0.0 {
                return None;
            }
            let used = config
                .pointer("/used/val")
                .or_else(|| config.pointer("/usage/totalUsed/val"))
                .and_then(number)
                .unwrap_or(0.0);
            Some(used / limit * 100.0)
        })
        .unwrap_or(0.0); // Match Grok's UI fallback; missing percentage is not an explicit server zero.
    let period = config
        .pointer("/currentPeriod/type")
        .and_then(Value::as_str)
        .unwrap_or("");
    let label = if period.contains("WEEKLY") {
        "Grok 7d"
    } else if period.contains("MONTHLY") {
        "Grok monthly"
    } else {
        "Grok"
    };
    let reset = config
        .pointer("/currentPeriod/end")
        .or_else(|| config.get("billingPeriodEnd"))
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(GrokUsage {
        label: label.into(),
        used_percent: used_percent.clamp(0.0, 100.0),
        reset,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_percentage_before_legacy_usage_and_current_reset() {
        let usage = extract_usage(&json!({"config": {
            "creditUsagePercent": 27.5, "monthlyLimit": {"val": 1000}, "used": {"val": 800},
            "currentPeriod": {"type": "USAGE_PERIOD_TYPE_WEEKLY", "end": "2026-10-03T06:30:50Z"},
            "billingPeriodEnd": "2026-11-01T00:00:00Z"
        }}))
        .unwrap();
        assert_eq!(usage.used_percent, 27.5);
        assert_eq!(usage.label, "Grok 7d");
        assert_eq!(usage.reset.as_deref(), Some("2026-10-03T06:30:50Z"));
    }

    #[test]
    fn matches_cli_zero_fallback_for_observed_unified_billing() {
        let usage = extract_usage(&json!({"config": {
            "isUnifiedBillingUser": true,
            "currentPeriod": {"type": "USAGE_PERIOD_TYPE_WEEKLY", "end": "2026-10-03T06:30:50Z"},
            "prepaidBalance": {"val": 0}
        }}))
        .unwrap();
        assert_eq!(usage.used_percent, 0.0);
        assert!(extract_usage(&json!({})).is_err());
    }

    #[test]
    fn reads_legacy_ratio_and_ignores_rpc_notifications() {
        let usage = extract_usage(&json!({"config": {
            "monthlyLimit": {"val": "1000"}, "used": {"val": "425"}
        }}))
        .unwrap();
        assert_eq!(usage.used_percent, 42.5);
        let (sender, receiver) = mpsc::channel();
        sender
            .send(json!({"method": "notification"}).to_string())
            .unwrap();
        sender
            .send(json!({"id": 1, "result": {"wrong": true}}).to_string())
            .unwrap();
        sender
            .send(json!({"id": 2, "result": {"config": {}}}).to_string())
            .unwrap();
        assert_eq!(receive_rpc(&receiver, 2).unwrap(), json!({"config": {}}));
    }

    #[test]
    fn clamps_usage_and_reports_rpc_errors() {
        for (used, expected) in [(-5.0, 0.0), (115.0, 100.0)] {
            assert_eq!(
                extract_usage(&json!({"config": {"creditUsagePercent": used}}))
                    .unwrap()
                    .used_percent,
                expected
            );
        }
        let (sender, receiver) = mpsc::channel();
        sender
            .send(
                json!({"id": 2, "error": {"code": -32000, "message": "Sign in required"}})
                    .to_string(),
            )
            .unwrap();
        assert_eq!(
            receive_rpc(&receiver, 2).unwrap_err(),
            "grok: Sign in required"
        );
    }
}
