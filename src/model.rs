use std::path::PathBuf;
use std::time::Instant;

use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) struct Args {
    pub(crate) action: Action,
    pub(crate) mode: Mode,
    pub(crate) interval: u64,
    pub(crate) once: bool,
    pub(crate) amp_interval: u64,
    pub(crate) storage_interval: u64,
    pub(crate) clocks: Vec<Clock>,
    pub(crate) config_path: PathBuf,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum Action {
    Run,
    ConfigPath,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Clock {
    pub(crate) label: String,
    pub(crate) timezone: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum Mode {
    Stats,
    CodexUsageStatus,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ProviderState<T> {
    pub(crate) result: Option<T>,
    pub(crate) error: Option<String>,
    pub(crate) updated_at: Option<DateTime<Local>>,
    pub(crate) ready: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct AmpUsage {
    pub(crate) plan: Option<String>,
    pub(crate) other_percent_remaining: Option<f64>,
    pub(crate) reset: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct CodexActivityUsage {
    pub(crate) daily_usage_buckets: Option<Vec<CodexDailyUsageBucket>>,
    pub(crate) summary: Option<CodexActivitySummary>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct CodexDailyUsageBucket {
    pub(crate) start_date: String,
    pub(crate) tokens: u64,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct CodexActivitySummary {
    pub(crate) lifetime_tokens: Option<u64>,
    pub(crate) peak_daily_tokens: Option<u64>,
    pub(crate) longest_running_turn_sec: Option<u64>,
    pub(crate) current_streak_days: Option<u64>,
    pub(crate) longest_streak_days: Option<u64>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct DailyTokenUsage {
    pub(crate) date: NaiveDate,
    pub(crate) tokens: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct SystemMetrics {
    pub(crate) cpu_percent: Option<f64>,
    pub(crate) load_avg: (f64, f64, f64),
    pub(crate) gpu_percent: Option<f64>,
    pub(crate) ram_percent: f64,
    pub(crate) ram_used: u64,
    pub(crate) ram_total: u64,
    pub(crate) net_down_rate: Option<f64>,
    pub(crate) net_up_rate: Option<f64>,
    pub(crate) net_interface: String,
    pub(crate) storage_free: u64,
    pub(crate) storage_total: u64,
    pub(crate) storage_percent_free: f64,
    pub(crate) storage_updated_at: Instant,
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            cpu_percent: None,
            load_avg: (0.0, 0.0, 0.0),
            gpu_percent: None,
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
pub(crate) struct AppState {
    pub(crate) amp: ProviderState<AmpUsage>,
    pub(crate) codex: ProviderState<Value>,
    pub(crate) codex_activity: ProviderState<CodexActivityUsage>,
    pub(crate) system: SystemMetrics,
}
