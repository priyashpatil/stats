use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use chrono::Local;
use regex::Regex;

use crate::cache::{load_cached_amp, write_usage_cache};
use crate::command::run_output;
use crate::model::{AmpUsage, AppState};
use crate::worker::sleep_stop;

pub(crate) fn spawn_refresh_amp(
    state: &Arc<Mutex<AppState>>,
    stop: &Arc<AtomicBool>,
    interval: u64,
) {
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

#[cfg(test)]
mod tests {
    use super::extract_amp_usage;

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
        let usage = extract_amp_usage("Subscription Megawatt: 82% other usage and 64.5% orb usage remaining - resets upon renewal in 1 month\n").expect("usage");
        assert_eq!(usage.plan.as_deref(), Some("Megawatt"));
        assert_eq!(usage.other_percent_remaining, Some(82.0));
    }

    #[test]
    fn extracts_other_subscription_plans_and_day_renewals() {
        let usage = extract_amp_usage(
            "Amp Gigawatt Subscription: 97.5% other usage and 100% orb usage remaining - resets upon renewal in 29 days\n",
        )
        .expect("usage");
        assert_eq!(usage.plan.as_deref(), Some("Gigawatt"));
        assert_eq!(usage.other_percent_remaining, Some(97.5));
        assert_eq!(
            usage.reset.as_deref(),
            Some("resets upon renewal in 29 days")
        );
    }

    #[test]
    fn extracts_subscription_usage_without_a_renewal_date() {
        let usage = extract_amp_usage(
            "Amp Megawatt Subscription: 82% other usage and 64.5% orb usage remaining\n",
        )
        .expect("usage");
        assert_eq!(usage.plan.as_deref(), Some("Megawatt"));
        assert_eq!(usage.other_percent_remaining, Some(82.0));
        assert_eq!(usage.reset, None);
    }
}
