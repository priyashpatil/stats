use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{Days, Local, NaiveDate, Utc};
use regex::Regex;

use crate::cache::{load_amp_request_ledger, load_cached_amp, write_usage_cache};
use crate::model::{
    AmpActivityUsage, AmpDailyUsageBucket, AmpRequestLedger, AmpRequestRecord, AmpTokenCategory,
    AmpUsage, AppState,
};
use crate::worker::sleep_stop;

static AMP_USAGE_LOCK: Mutex<()> = Mutex::new(());
static AMP_REQUEST_LEDGER: OnceLock<Mutex<AmpRequestLedger>> = OnceLock::new();

const AMP_REQUEST_BUDGET: usize = 40;
const AMP_HISTORICAL_BUDGET: usize = 24;
const AMP_WINDOW_SECONDS: f64 = 3_600.0;
const AMP_CURRENT_INTERVAL_SECONDS: f64 = 900.0;

#[derive(Clone, Copy, Eq, PartialEq)]
enum AmpRequestKind {
    Account,
    Current,
    Historical,
}

impl AmpRequestKind {
    fn label(self) -> &'static str {
        match self {
            Self::Account => "account",
            Self::Current => "current",
            Self::Historical => "historical",
        }
    }
}

enum AmpFetch<T> {
    Ready(T),
    Deferred(Duration),
}

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
                Ok(AmpFetch::Ready(result)) => {
                    write_usage_cache("amp", &result);
                    let mut guard = state.lock().unwrap();
                    guard.amp.result = Some(result);
                    guard.amp.error = None;
                    guard.amp.updated_at = Some(Local::now());
                    guard.amp.stale = false;
                }
                Ok(AmpFetch::Deferred(_)) => {}
                Err(err) => load_cached_amp(&state, err),
            }
            sleep_stop(&stop, Duration::from_secs(interval));
        }
    });
}

pub(crate) fn spawn_refresh_amp_activity(
    state: &Arc<Mutex<AppState>>,
    stop: &Arc<AtomicBool>,
    interval: u64,
) {
    let state = Arc::clone(state);
    let stop = Arc::clone(stop);
    thread::spawn(move || {
        sleep_stop(&stop, Duration::from_millis(500));
        let mut activity = state
            .lock()
            .unwrap()
            .amp_activity
            .result
            .clone()
            .unwrap_or_default();
        while !stop.load(Ordering::Relaxed) {
            let today = Utc::now().date_naive();
            let history_days = state.lock().unwrap().amp_activity_history_days.max(7);
            'dates: for date in activity_dates(today, history_days) {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                let completed_and_cached = date < today
                    && activity
                        .daily_usage_buckets
                        .iter()
                        .any(|bucket| bucket.date == date.to_string());
                if completed_and_cached {
                    continue;
                }
                loop {
                    match read_amp_daily_usage(date) {
                        Ok(AmpFetch::Ready(bucket)) => {
                            upsert_daily_bucket(&mut activity, bucket);
                            write_usage_cache("amp-activity", &activity);
                            let mut guard = state.lock().unwrap();
                            guard.amp_activity.result = Some(activity.clone());
                            guard.amp_activity.error = None;
                            guard.amp_activity.retry_after = None;
                            guard.amp_activity.updated_at = Some(Local::now());
                            guard.amp_activity.stale = false;
                            break;
                        }
                        Ok(AmpFetch::Deferred(delay)) if delay <= Duration::from_secs(2) => {
                            sleep_stop(&stop, delay);
                        }
                        Ok(AmpFetch::Deferred(_)) if date == today => continue 'dates,
                        Ok(AmpFetch::Deferred(delay)) => {
                            let mut guard = state.lock().unwrap();
                            guard.amp_activity.error = None;
                            guard.amp_activity.retry_after = Some(delay);
                            break 'dates;
                        }
                        Err(error) => {
                            let mut guard = state.lock().unwrap();
                            guard.amp_activity.error = Some(error);
                            guard.amp_activity.retry_after = None;
                            break 'dates;
                        }
                    }
                }
            }
            if state.lock().unwrap().amp_activity_history_days > history_days {
                continue;
            }
            sleep_stop(&stop, Duration::from_secs(interval));
        }
    });
}

fn activity_dates(today: NaiveDate, days: usize) -> Vec<NaiveDate> {
    (0..days as u64)
        .filter_map(|days| today.checked_sub_days(Days::new(days)))
        .collect()
}

fn upsert_daily_bucket(activity: &mut AmpActivityUsage, bucket: AmpDailyUsageBucket) {
    activity
        .daily_usage_buckets
        .retain(|existing| existing.date != bucket.date);
    activity.daily_usage_buckets.push(bucket);
    activity
        .daily_usage_buckets
        .sort_by(|left, right| left.date.cmp(&right.date));
}

fn read_amp_daily_usage(date: NaiveDate) -> Result<AmpFetch<AmpDailyUsageBucket>, String> {
    let end = date
        .checked_add_days(Days::new(1))
        .ok_or_else(|| "could not build Amp activity range".to_string())?;
    let start_arg = format!("{date}T00:00:00Z");
    let end_arg = format!("{end}T00:00:00Z");
    let kind = if date == Utc::now().date_naive() {
        AmpRequestKind::Current
    } else {
        AmpRequestKind::Historical
    };
    let output = match run_amp_usage(
        &[
            "usage",
            "--no-color",
            "--details",
            "--start",
            &start_arg,
            "--end",
            &end_arg,
        ],
        kind,
    )? {
        AmpFetch::Ready(output) => output,
        AmpFetch::Deferred(delay) => return Ok(AmpFetch::Deferred(delay)),
    };
    extract_amp_daily_usage(&output, date)
        .map(AmpFetch::Ready)
        .ok_or_else(|| format!("could not read Amp activity for {date}"))
}

fn read_amp_usage() -> Result<AmpFetch<AmpUsage>, String> {
    let output = match run_amp_usage(
        &["usage", "--no-color", "--details"],
        AmpRequestKind::Account,
    )? {
        AmpFetch::Ready(output) => output,
        AmpFetch::Deferred(delay) => return Ok(AmpFetch::Deferred(delay)),
    };
    extract_amp_usage(&output)
        .map(AmpFetch::Ready)
        .ok_or_else(|| "could not read Amp usage".into())
}

fn run_amp_usage(args: &[&str], kind: AmpRequestKind) -> Result<AmpFetch<String>, String> {
    let delay = reserve_amp_request(kind);
    if !delay.is_zero() {
        return Ok(AmpFetch::Deferred(delay));
    }
    let _usage_guard = AMP_USAGE_LOCK.lock().unwrap();
    let output = Command::new("amp")
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if output.status.success() {
        if is_rate_limited(&stdout) {
            let retry_seconds = record_rate_limit(&stdout);
            return Err(format!(
                "Amp usage rate limited; retrying in {}",
                compact_retry_duration(retry_seconds)
            ));
        }
        return Ok(AmpFetch::Ready(stdout));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let message = [stdout.trim(), stderr.trim()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if is_rate_limited(&message) {
        let retry_seconds = record_rate_limit(&message);
        return Err(format!(
            "Amp usage rate limited; retrying in {}",
            compact_retry_duration(retry_seconds)
        ));
    }
    Err(if message.is_empty() {
        format!("amp usage exited with {}", output.status)
    } else {
        message
    })
}

fn reserve_amp_request(kind: AmpRequestKind) -> Duration {
    let now = epoch_seconds();
    let ledger = AMP_REQUEST_LEDGER.get_or_init(|| Mutex::new(load_amp_request_ledger()));
    let mut ledger = ledger.lock().unwrap();
    prune_request_ledger(&mut ledger, now);
    if let Some(delay) = amp_request_delay(&ledger, kind, now) {
        return delay;
    }
    ledger.requests.push(AmpRequestRecord {
        requested_at: now,
        kind: kind.label().into(),
    });
    write_usage_cache("amp-requests", &*ledger);
    Duration::ZERO
}

fn amp_request_delay(
    ledger: &AmpRequestLedger,
    kind: AmpRequestKind,
    now: f64,
) -> Option<Duration> {
    if let Some(blocked_until) = ledger.blocked_until
        && blocked_until > now
    {
        return Some(Duration::from_secs_f64(blocked_until - now));
    }
    if kind == AmpRequestKind::Current
        && let Some(last) = ledger
            .requests
            .iter()
            .rev()
            .find(|request| request.kind == kind.label())
        && now - last.requested_at < AMP_CURRENT_INTERVAL_SECONDS
    {
        return Some(Duration::from_secs_f64(
            AMP_CURRENT_INTERVAL_SECONDS - (now - last.requested_at),
        ));
    }
    let historical_count = ledger
        .requests
        .iter()
        .filter(|request| request.kind == AmpRequestKind::Historical.label())
        .count();
    if ledger.requests.len() >= AMP_REQUEST_BUDGET
        || kind == AmpRequestKind::Historical && historical_count >= AMP_HISTORICAL_BUDGET
    {
        let oldest_relevant =
            if kind == AmpRequestKind::Historical && historical_count >= AMP_HISTORICAL_BUDGET {
                ledger
                    .requests
                    .iter()
                    .find(|request| request.kind == AmpRequestKind::Historical.label())
            } else {
                ledger.requests.first()
            };
        return Some(
            oldest_relevant
                .map(|request| {
                    Duration::from_secs_f64(
                        (request.requested_at + AMP_WINDOW_SECONDS - now).max(1.0),
                    )
                })
                .unwrap_or(Duration::from_secs(60)),
        );
    }
    if let Some(last) = ledger.requests.last()
        && now - last.requested_at < 1.0
    {
        return Some(Duration::from_secs_f64(1.0 - (now - last.requested_at)));
    }
    None
}

fn prune_request_ledger(ledger: &mut AmpRequestLedger, now: f64) {
    ledger
        .requests
        .retain(|request| now - request.requested_at < AMP_WINDOW_SECONDS);
    if ledger
        .blocked_until
        .is_some_and(|blocked_until| blocked_until <= now)
    {
        ledger.blocked_until = None;
    }
}

fn record_rate_limit(message: &str) -> f64 {
    let now = epoch_seconds();
    let ledger = AMP_REQUEST_LEDGER.get_or_init(|| Mutex::new(load_amp_request_ledger()));
    let mut ledger = ledger.lock().unwrap();
    prune_request_ledger(&mut ledger, now);
    let retry_seconds = retry_after_seconds(message).unwrap_or_else(|| {
        ledger
            .requests
            .first()
            .map(|request| (request.requested_at + AMP_WINDOW_SECONDS - now).max(60.0))
            .unwrap_or(AMP_WINDOW_SECONDS)
    });
    ledger.blocked_until = Some(now + retry_seconds);
    write_usage_cache("amp-requests", &*ledger);
    retry_seconds
}

fn retry_after_seconds(message: &str) -> Option<f64> {
    let captures = Regex::new(
        r"(?i)(?:retry[- ]?after|try again in|retry in)\D*([0-9]+)\s*(seconds?|secs?|s|minutes?|mins?|m|hours?|hrs?|h)?",
    )
    .ok()?
    .captures(message)?;
    let amount = captures.get(1)?.as_str().parse::<f64>().ok()?;
    Some(
        match captures.get(2).map(|unit| unit.as_str().to_lowercase()) {
            Some(unit) if unit.starts_with('m') => amount * 60.0,
            Some(unit) if unit.starts_with('h') => amount * 3_600.0,
            _ => amount,
        },
    )
}

fn is_rate_limited(message: &str) -> bool {
    let lowercase = message.to_lowercase();
    lowercase.contains("rate limit")
        || lowercase.contains("too many requests")
        || lowercase.contains("http 429")
        || lowercase.contains("status 429")
        || lowercase.contains("error 429")
}

fn compact_retry_duration(seconds: f64) -> String {
    let minutes = (seconds / 60.0).ceil() as u64;
    if minutes < 60 {
        format!("{minutes}m")
    } else {
        let hours = minutes / 60;
        let remainder = minutes % 60;
        if remainder == 0 {
            format!("{hours}h")
        } else {
            format!("{hours}h {remainder}m")
        }
    }
}

fn epoch_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or_default()
}

fn extract_amp_usage(output: &str) -> Option<AmpUsage> {
    let cleaned = strip_ansi(output).replace("**", "");
    let subscription = Regex::new(
        r"(?i)(?:Amp\s+([^:\r\n]+?)\s+Subscription|Subscription\s+([^:\r\n]+)):\s*([0-9]+(?:\.[0-9]+)?)%\s+other usage(?:\s+and\s+([0-9]+(?:\.[0-9]+)?)%\s+orb usage)?\s+remaining(?:\s+-\s+([^\r\n]+))?",
    )
    .ok()?
    .captures(&cleaned)?;
    let orb_runtime = Regex::new(r"(?im)^Total Orb runtime:\s*([^\r\n(]+)")
        .ok()?
        .captures(&cleaned)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().trim().to_string());
    let individual_credits_remaining =
        Regex::new(r"(?im)^Individual credits(?: remaining)?:\s*([^\s]+)(?:\s+remaining)?")
            .ok()?
            .captures(&cleaned)
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().trim().to_string());
    Some(AmpUsage {
        plan: subscription
            .get(1)
            .or_else(|| subscription.get(2))
            .map(|value| value.as_str().trim().to_string()),
        other_percent_remaining: subscription
            .get(3)
            .and_then(|value| value.as_str().parse().ok()),
        orb_percent_remaining: subscription
            .get(4)
            .and_then(|value| value.as_str().parse().ok()),
        orb_runtime,
        individual_credits_remaining,
        reset: subscription
            .get(5)
            .map(|value| value.as_str().trim().to_string()),
    })
}

fn extract_amp_daily_usage(output: &str, date: NaiveDate) -> Option<AmpDailyUsageBucket> {
    let cleaned = strip_ansi(output).replace("**", "");
    let tokens = Regex::new(r"(?im)^Total tokens:\s*([0-9,]+)")
        .ok()?
        .captures(&cleaned)?
        .get(1)?
        .as_str()
        .replace(',', "")
        .parse()
        .ok()?;
    let orb_runtime_millis = Regex::new(r"(?im)^Total Orb runtime:.*\(([0-9,]+) ms\)")
        .ok()?
        .captures(&cleaned)
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().replace(',', "").parse().ok())
        .unwrap_or_default();
    let costs = table_rows(&cleaned, "Daily Recorded Cost")
        .into_iter()
        .find(|row| row.first().is_some_and(|value| value == &date.to_string()));
    Some(AmpDailyUsageBucket {
        date: date.to_string(),
        tokens,
        orb_runtime_millis,
        covered_cost: costs
            .as_ref()
            .and_then(|row| row.get(1))
            .and_then(|value| parse_currency(value))
            .unwrap_or_default(),
        paid_cost: costs
            .as_ref()
            .and_then(|row| row.get(2))
            .and_then(|value| parse_currency(value))
            .unwrap_or_default(),
        sources: token_categories(&cleaned, "From"),
        models: token_categories(&cleaned, "Models"),
    })
}

fn token_categories(output: &str, heading: &str) -> Vec<AmpTokenCategory> {
    table_rows(output, heading)
        .into_iter()
        .filter_map(|row| {
            Some(AmpTokenCategory {
                label: row.first()?.to_string(),
                tokens: row.get(1)?.replace(',', "").parse().ok()?,
            })
        })
        .collect()
}

fn table_rows(output: &str, heading: &str) -> Vec<Vec<String>> {
    let marker = format!("## {heading}");
    let Some(section) = output.split(&marker).nth(1) else {
        return Vec::new();
    };
    section
        .lines()
        .take_while(|line| !line.starts_with("## "))
        .filter(|line| line.starts_with('|') && !line.contains("| ---"))
        .skip(1)
        .map(|line| {
            line.trim_matches('|')
                .split('|')
                .map(|value| value.trim().to_string())
                .collect()
        })
        .collect()
}

fn parse_currency(value: &str) -> Option<f64> {
    value
        .trim()
        .trim_start_matches('$')
        .replace(',', "")
        .parse()
        .ok()
}

fn strip_ansi(value: &str) -> String {
    Regex::new(r"\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x07]*(?:\x07|\x1b\\)|[ -/]*[@-~])")
        .unwrap()
        .replace_all(value, "")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        AmpRequestKind, activity_dates, amp_request_delay, extract_amp_daily_usage,
        extract_amp_usage, is_rate_limited, prune_request_ledger, retry_after_seconds,
    };
    use crate::model::{AmpRequestLedger, AmpRequestRecord};
    use chrono::NaiveDate;

    #[test]
    fn extracts_current_amp_megawatt_usage() {
        let output = "Signed in as user@example.com\n**Amp Megawatt Subscription:** 82% other usage and 64.5% orb usage remaining - resets upon renewal in 1 month\n**Individual credits:** $1.01 remaining\n\nRange: 2026-08-18T05:45:24.249Z to 2026-08-25T05:45:24.249Z (end exclusive)\nTotal Orb runtime: 1h20m12.210s (4,812,210 ms)\n";
        let usage = extract_amp_usage(output).expect("usage");
        assert_eq!(usage.plan.as_deref(), Some("Megawatt"));
        assert_eq!(usage.other_percent_remaining, Some(82.0));
        assert_eq!(usage.orb_percent_remaining, Some(64.5));
        assert_eq!(usage.orb_runtime.as_deref(), Some("1h20m12.210s"));
        assert_eq!(usage.individual_credits_remaining.as_deref(), Some("$1.01"));
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
        assert_eq!(usage.orb_percent_remaining, Some(64.5));
        assert_eq!(usage.orb_runtime, None);
        assert_eq!(usage.individual_credits_remaining, None);
    }

    #[test]
    fn extracts_other_subscription_plans_and_day_renewals() {
        let usage = extract_amp_usage(
            "Amp Gigawatt Subscription: 97.5% other usage and 100% orb usage remaining - resets upon renewal in 29 days\n",
        )
        .expect("usage");
        assert_eq!(usage.plan.as_deref(), Some("Gigawatt"));
        assert_eq!(usage.other_percent_remaining, Some(97.5));
        assert_eq!(usage.orb_percent_remaining, Some(100.0));
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
        assert_eq!(usage.orb_percent_remaining, Some(64.5));
        assert_eq!(usage.reset, None);
    }

    #[test]
    fn extracts_historical_usage_with_optional_fields_omitted() {
        let usage = extract_amp_usage(
            "Subscription Free: 72.25% other usage remaining\nIndividual credits remaining: $0.50\n",
        )
        .expect("usage");
        assert_eq!(usage.plan.as_deref(), Some("Free"));
        assert_eq!(usage.other_percent_remaining, Some(72.25));
        assert_eq!(usage.orb_percent_remaining, None);
        assert_eq!(usage.individual_credits_remaining.as_deref(), Some("$0.50"));
        assert_eq!(usage.reset, None);
    }

    #[test]
    fn deserializes_cache_data_from_before_optional_amp_fields() {
        let usage: crate::model::AmpUsage = serde_json::from_str(
            r#"{"plan":"Megawatt","other_percent_remaining":80.0,"reset":"in 2 days"}"#,
        )
        .expect("old cache");
        assert_eq!(usage.other_percent_remaining, Some(80.0));
        assert_eq!(usage.orb_percent_remaining, None);
        assert_eq!(usage.individual_credits_remaining, None);
    }

    #[test]
    fn extracts_range_scoped_amp_activity() {
        let output = r#"# Usage
Range: 2026-08-24T00:00:00.000Z to 2026-08-25T00:00:00.000Z (end exclusive)
Total Orb runtime: 1h2m3s (3,723,000 ms)
Total tokens: 352,596,082

## From
| Source | Tokens |
| --- | ---: |
| ChatGPT Plus/Pro | 352,500,000 |
| Amp | 96,082 |

## Models
| Model | Tokens |
| --- | ---: |
| GPT-5.6 Sol | 340,187,011 |
| GPT-5.6 Terra | 12,409,071 |

## Daily Recorded Cost
| Date (UTC) | Covered | Paid | Total |
| --- | ---: | ---: | ---: |
| 2026-08-24 | $1.25 | $0.05 | $1.30 |

## Usage by Thread
| Thread | Tokens | Covered | Paid | Total |
| --- | ---: | ---: | ---: | ---: |
| Private title (T-example) | 42 | $0 | $0 | $0 |
"#;
        let usage = extract_amp_daily_usage(output, NaiveDate::from_ymd_opt(2026, 8, 24).unwrap())
            .expect("activity");

        assert_eq!(usage.tokens, 352_596_082);
        assert_eq!(usage.orb_runtime_millis, 3_723_000);
        assert_eq!(usage.covered_cost, 1.25);
        assert_eq!(usage.paid_cost, 0.05);
        assert_eq!(usage.sources.len(), 2);
        assert_eq!(usage.sources[0].label, "ChatGPT Plus/Pro");
        assert_eq!(usage.models[1].tokens, 12_409_071);
    }

    #[test]
    fn builds_a_thirty_day_activity_window_newest_first() {
        let today = NaiveDate::from_ymd_opt(2026, 8, 25).unwrap();
        let dates = activity_dates(today, 30);
        assert_eq!(dates.len(), 30);
        assert_eq!(dates[0], today);
        assert_eq!(dates[29], NaiveDate::from_ymd_opt(2026, 7, 27).unwrap());
    }

    #[test]
    fn recognizes_rate_limits_and_retry_windows() {
        assert!(is_rate_limited("HTTP 429: Too Many Requests"));
        assert!(is_rate_limited("usage rate limit exceeded"));
        assert_eq!(retry_after_seconds("Retry-After: 90 seconds"), Some(90.0));
        assert_eq!(retry_after_seconds("Retry in 45m"), Some(2_700.0));
        assert_eq!(retry_after_seconds("Try again in 12 minutes"), Some(720.0));
        assert_eq!(retry_after_seconds("Try again in 2 hours"), Some(7_200.0));
        assert!(!is_rate_limited("Total tokens: 429"));
    }

    #[test]
    fn enforces_rolling_total_and_historical_budgets() {
        let now = 10_000.0;
        let total_ledger = AmpRequestLedger {
            requests: (0..40)
                .map(|offset| AmpRequestRecord {
                    requested_at: now - 100.0 + offset as f64,
                    kind: "account".into(),
                })
                .collect(),
            blocked_until: None,
        };
        assert!(amp_request_delay(&total_ledger, AmpRequestKind::Account, now).is_some());

        let history_ledger = AmpRequestLedger {
            requests: (0..24)
                .map(|offset| AmpRequestRecord {
                    requested_at: now - 100.0 + offset as f64,
                    kind: "historical".into(),
                })
                .collect(),
            blocked_until: None,
        };
        assert!(amp_request_delay(&history_ledger, AmpRequestKind::Historical, now).is_some());
        assert!(amp_request_delay(&history_ledger, AmpRequestKind::Account, now).is_none());
    }

    #[test]
    fn enforces_current_refresh_and_server_cooldowns() {
        let now = 10_000.0;
        let current_ledger = AmpRequestLedger {
            requests: vec![AmpRequestRecord {
                requested_at: now - 300.0,
                kind: "current".into(),
            }],
            blocked_until: None,
        };
        assert_eq!(
            amp_request_delay(&current_ledger, AmpRequestKind::Current, now)
                .unwrap()
                .as_secs(),
            600
        );
        let blocked_ledger = AmpRequestLedger {
            blocked_until: Some(now + 120.0),
            ..AmpRequestLedger::default()
        };
        assert_eq!(
            amp_request_delay(&blocked_ledger, AmpRequestKind::Account, now)
                .unwrap()
                .as_secs(),
            120
        );
    }

    #[test]
    fn prunes_expired_requests_and_cooldowns() {
        let now = 10_000.0;
        let mut ledger = AmpRequestLedger {
            requests: vec![
                AmpRequestRecord {
                    requested_at: now - 3_600.0,
                    kind: "historical".into(),
                },
                AmpRequestRecord {
                    requested_at: now - 3_599.0,
                    kind: "account".into(),
                },
            ],
            blocked_until: Some(now),
        };
        prune_request_ledger(&mut ledger, now);
        assert_eq!(ledger.requests.len(), 1);
        assert_eq!(ledger.requests[0].kind, "account");
        assert_eq!(ledger.blocked_until, None);
    }
}
