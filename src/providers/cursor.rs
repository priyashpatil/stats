use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::model::{AppState, QuotaLimit, QuotaUsage};
use crate::providers::spawn_quota_refresh;

pub(crate) fn executable() -> Option<PathBuf> {
    let candidates = ["agent", "cursor-agent"];
    if let Some(path) = std::env::var_os("CURSOR_AGENT_PATH")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
    {
        return Some(path);
    }
    std::env::var_os("PATH").and_then(|paths| {
        candidates.into_iter().find_map(|name| {
            std::env::split_paths(&paths)
                .map(|path| path.join(name))
                .find(|path| path.is_file())
        })
    })
}

pub(crate) fn spawn_refresh_cursor(
    state: &Arc<Mutex<AppState>>,
    stop: &Arc<AtomicBool>,
    interval: u64,
) {
    spawn_quota_refresh(
        state,
        stop,
        interval,
        "cursor",
        |state| &mut state.cursor,
        read_usage,
    );
}

fn read_usage() -> Result<QuotaUsage, String> {
    let executable = executable().ok_or_else(|| "Cursor agent not found".to_string())?;
    let mut child = Command::new(executable)
        .args(["status", "--format", "json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("could not read Cursor agent status: {error}"))?;
    let deadline = Instant::now() + Duration::from_secs(10);
    let exit_status = loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Cursor agent status timed out".into());
        }
        thread::sleep(Duration::from_millis(25));
    };
    if !exit_status.success() {
        return Err(format!("Cursor agent status exited with {exit_status}"));
    }
    let mut stdout = Vec::new();
    child
        .stdout
        .take()
        .ok_or_else(|| "could not read Cursor agent status output".to_string())?
        .read_to_end(&mut stdout)
        .map_err(|error| error.to_string())?;
    let status: Value = serde_json::from_slice(&stdout)
        .map_err(|error| format!("invalid Cursor agent status: {error}"))?;
    if status.get("authenticated").and_then(Value::as_bool) == Some(false) {
        return Err("Cursor agent is not authenticated".into());
    }
    let token = find_string(&status, &["accessToken", "apiKey"])
        .ok_or_else(|| "Cursor agent status did not include authentication".to_string())?;
    let endpoint = status
        .get("endpoint")
        .and_then(Value::as_str)
        .unwrap_or("https://api2.cursor.sh");
    let url = format!(
        "{}/aiserver.v1.DashboardService/GetCurrentPeriodUsage",
        endpoint.trim_end_matches('/')
    );
    let response = ureq::post(&url)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
        .set("Accept", "application/json")
        .set("Connect-Protocol-Version", "1")
        .timeout(Duration::from_secs(15))
        .send_string("{}")
        .map_err(|error| format!("could not read Cursor usage: {error}"))?;
    let value: Value = serde_json::from_reader(response.into_reader())
        .map_err(|error| format!("invalid Cursor usage response: {error}"))?;
    extract_usage(&value)
}

fn find_string<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    match value {
        Value::Object(values) => keys
            .iter()
            .find_map(|key| values.get(*key).and_then(Value::as_str))
            .or_else(|| values.values().find_map(|value| find_string(value, keys))),
        Value::Array(values) => values.iter().find_map(|value| find_string(value, keys)),
        _ => None,
    }
}

fn extract_usage(value: &Value) -> Result<QuotaUsage, String> {
    let plan = value
        .get("planUsage")
        .or_else(|| value.pointer("/individualUsage/plan"))
        .ok_or_else(|| "Cursor usage response did not include plan usage".to_string())?;
    let used_percent = number(plan.get("totalPercentUsed"))
        .or_else(|| ratio_percent(plan.get("used"), plan.get("limit")))
        .ok_or_else(|| "Cursor usage response did not include a plan limit".to_string())?;
    let reset = value
        .get("billingCycleEnd")
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(QuotaUsage {
        limits: vec![QuotaLimit {
            label: "Cursor".into(),
            used_percent: used_percent.clamp(0.0, 100.0),
            reset,
        }],
    })
}

fn number(value: Option<&Value>) -> Option<f64> {
    value?.as_f64().or_else(|| value?.as_str()?.parse().ok())
}

fn ratio_percent(used: Option<&Value>, limit: Option<&Value>) -> Option<f64> {
    let limit = number(limit)?;
    (limit > 0.0)
        .then(|| number(used).map(|used| used / limit * 100.0))
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_nested_cursor_token() {
        let status = json!({"authenticated": true, "auth": {"accessToken": "secret"}});
        assert_eq!(
            find_string(&status, &["accessToken", "apiKey"]),
            Some("secret")
        );
    }

    #[test]
    fn extracts_cursor_plan_usage() {
        let usage = extract_usage(&json!({
            "billingCycleEnd": "2026-09-23T00:00:00Z",
            "planUsage": {"used": 500, "limit": 2000}
        }))
        .unwrap();
        assert_eq!(usage.limits[0].used_percent, 25.0);
    }
}
