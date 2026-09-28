use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use chrono::Local;
use regex::Regex;
use serde_json::Value;

use crate::cache::{cli_working_dir, load_cached_claude, write_usage_cache};
use crate::model::{AppState, ClaudeLimit, ClaudeUsage};
use crate::worker::sleep_stop;

pub(crate) fn spawn_refresh_claude(
    state: &Arc<Mutex<AppState>>,
    stop: &Arc<AtomicBool>,
    interval: u64,
) {
    let state = Arc::clone(state);
    let stop = Arc::clone(stop);
    thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            match read_claude_usage() {
                Ok(result) => {
                    write_usage_cache("claude", &result);
                    let mut guard = state.lock().unwrap();
                    guard.claude.result = Some(result);
                    guard.claude.error = None;
                    guard.claude.updated_at = Some(Local::now());
                    guard.claude.stale = false;
                }
                Err(error) => load_cached_claude(&state, error),
            }
            sleep_stop(&stop, Duration::from_secs(interval));
        }
    });
}

fn read_claude_usage() -> Result<ClaudeUsage, String> {
    let working_dir = cli_working_dir("claude")?;
    let output = Command::new("claude")
        .args([
            "--safe-mode",
            "--strict-mcp-config",
            "--tools",
            "",
            "-p",
            "/usage",
            "--output-format",
            "json",
            "--no-session-persistence",
        ])
        .current_dir(working_dir)
        .output()
        .map_err(|error| error.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = [stdout.trim(), stderr.trim()]
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        return Err(if message.is_empty() {
            format!("claude /usage exited with {}", output.status)
        } else {
            message
        });
    }
    let envelope: Value = serde_json::from_str(&stdout)
        .map_err(|error| format!("could not read Claude usage response: {error}"))?;
    let result = envelope
        .get("result")
        .and_then(Value::as_str)
        .ok_or_else(|| "Claude usage response did not include result".to_string())?;
    extract_claude_usage(result)
        .ok_or_else(|| "Claude usage response did not include plan limits".into())
}

fn extract_claude_usage(output: &str) -> Option<ClaudeUsage> {
    let percent = Regex::new(r"(?i)([0-9]+(?:\.[0-9]+)?)%\s+used").ok()?;
    let reset = Regex::new(r"(?i)\bresets?\s+(.+)$").ok()?;
    let mut limits = Vec::new();
    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let lower = line.to_lowercase();
        let label = if lower.starts_with("current session") {
            Some("Claude 5h".to_string())
        } else if lower.starts_with("current week") {
            let scope = line
                .split_once('(')
                .and_then(|(_, rest)| rest.split_once(')'))
                .map(|(scope, _)| scope.trim());
            Some(match scope {
                Some(scope) if scope.eq_ignore_ascii_case("all models") => "Claude 7d".into(),
                Some(scope) if !scope.is_empty() => format!("Claude {scope}"),
                _ => "Claude 7d".into(),
            })
        } else {
            None
        };
        let Some(label) = label else { continue };
        let Some(captures) = percent.captures(line) else {
            continue;
        };
        let used_percent = captures.get(1)?.as_str().parse::<f64>().ok()?;
        let reset = reset.captures(line).and_then(|captures| {
            let whole = captures.get(0)?;
            let value = captures.get(1)?.as_str().trim();
            let wrapped = line[..whole.start()].trim_end().ends_with('(');
            Some(if wrapped {
                value.strip_suffix(')').unwrap_or(value).to_string()
            } else {
                value.to_string()
            })
        });
        limits.push(ClaudeLimit {
            label,
            used_percent,
            reset,
        });
    }
    (!limits.is_empty()).then_some(ClaudeUsage { limits })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_current_claude_plan_limits() {
        let usage = extract_claude_usage(
            "You are currently using your subscription to power your Claude Code usage\n\n\
             Current session: 2% used · resets Jan 8 at 7:30pm (America/New_York)\n\
             Current week (all models): 0% used · resets Jan 12 at 9:30am (America/New_York)\n",
        )
        .unwrap();

        assert_eq!(usage.limits.len(), 2);
        assert_eq!(usage.limits[0].label, "Claude 5h");
        assert_eq!(usage.limits[0].used_percent, 2.0);
        assert_eq!(
            usage.limits[0].reset.as_deref(),
            Some("Jan 8 at 7:30pm (America/New_York)")
        );
        assert_eq!(usage.limits[1].label, "Claude 7d");
        assert_eq!(usage.limits[1].used_percent, 0.0);
    }

    #[test]
    fn extracts_optional_model_limit_and_parenthesized_reset() {
        let usage = extract_claude_usage(
            "Current week (Sonnet): 12.5% used\n\
             Current session: 7.5% used (resets Aug 25, 2:29pm (America/Los_Angeles))",
        )
        .unwrap();

        assert_eq!(usage.limits[0].label, "Claude Sonnet");
        assert_eq!(usage.limits[0].reset, None);
        assert_eq!(usage.limits[1].used_percent, 7.5);
        assert_eq!(
            usage.limits[1].reset.as_deref(),
            Some("Aug 25, 2:29pm (America/Los_Angeles)")
        );
    }

    #[test]
    fn ignores_cost_and_activity_rows() {
        assert!(extract_claude_usage("Session cost: $0.00\nTotal sessions: 42\n").is_none());
    }
}
