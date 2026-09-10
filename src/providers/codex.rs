use std::env;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{Local, TimeZone};
use rand::Rng;
use serde_json::{Value, json};
use tungstenite::{Message, client};

use crate::cache::{load_cached_codex, load_cached_codex_activity, write_usage_cache};
use crate::model::{AppState, CodexActivityUsage};

pub(crate) fn pick_port() -> Result<u16, String> {
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

pub(crate) fn start_codex_server(port: u16) -> Result<Child, String> {
    Command::new("codex")
        .args(["app-server", "--listen", &format!("ws://127.0.0.1:{port}")])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to start codex app-server: {err}"))
}

pub(crate) fn wait_ready(port: u16, proc: &mut Child) -> Result<(), String> {
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

pub(crate) fn shutdown_server(proc: &mut Child) {
    if proc.try_wait().ok().flatten().is_some() {
        return;
    }
    let _ = proc.kill();
    let _ = proc.wait();
}

pub(crate) fn spawn_codex_client(
    state: &Arc<Mutex<AppState>>,
    stop: &Arc<AtomicBool>,
    port: u16,
    interval: u64,
    rate_limits_enabled: bool,
    activity_enabled: bool,
) {
    let state = Arc::clone(state);
    let stop = Arc::clone(stop);
    thread::spawn(move || {
        if let Err(err) = codex_client(
            &state,
            &stop,
            port,
            interval,
            rate_limits_enabled,
            activity_enabled,
        ) {
            if rate_limits_enabled {
                load_cached_codex(&state, err.clone());
            }
            if activity_enabled {
                load_cached_codex_activity(&state, err);
            }
        }
    });
}

fn codex_client(
    state: &Arc<Mutex<AppState>>,
    stop: &Arc<AtomicBool>,
    port: u16,
    interval: u64,
    rate_limits_enabled: bool,
    activity_enabled: bool,
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
        if rate_limits_enabled
            && initialized
            && pending_rate_id.is_none()
            && Instant::now() >= next_rate_refresh_at
        {
            pending_rate_id = Some(send_ws(
                &mut ws,
                &mut next_id,
                "account/rateLimits/read",
                None,
            )?);
        }
        if activity_enabled
            && initialized
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
            guard.codex.ready = rate_limits_enabled;
            guard.codex_activity.ready = activity_enabled;
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

pub(crate) fn ordered_buckets(result: &Value) -> Vec<&Value> {
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

pub(crate) fn codex_weekly_window(snapshot: &Value) -> Option<&Value> {
    ["primary", "secondary"]
        .into_iter()
        .filter_map(|key| snapshot.get(key))
        .find(|window| window.get("windowDurationMins").and_then(Value::as_i64) == Some(10080))
}

pub(crate) fn left_percent(window: &Value) -> f64 {
    let used = window
        .get("usedPercent")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    (100.0 - used.round()).clamp(0.0, 100.0)
}

pub(crate) fn run_codex_usage_status() -> Result<(), String> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
