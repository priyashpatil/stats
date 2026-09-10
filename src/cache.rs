use crate::cli::env_u64;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local, TimeZone};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::model::{AmpActivityUsage, AmpRequestLedger, AmpUsage, AppState, CodexActivityUsage};

#[derive(Debug, Clone, Deserialize)]
struct CacheEnvelope<T> {
    cached_at: f64,
    result: T,
}

fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("stats")
}

fn cache_path(provider: &str) -> PathBuf {
    cache_dir().join(format!("{provider}-usage.json"))
}

fn read_usage_cache<T>(provider: &str, max_age: Option<u64>) -> Option<(T, DateTime<Local>)>
where
    T: for<'de> Deserialize<'de>,
{
    let payload = fs::read_to_string(cache_path(provider)).ok()?;
    let envelope: CacheEnvelope<T> = serde_json::from_str(&payload).ok()?;
    if let Some(max_age) = max_age {
        let now = epoch_seconds();
        if now - envelope.cached_at > max_age as f64 {
            return None;
        }
    }
    let updated_at = Local
        .timestamp_opt(envelope.cached_at as i64, 0)
        .single()
        .unwrap_or_else(Local::now);
    Some((envelope.result, updated_at))
}

pub(crate) fn write_usage_cache<T>(provider: &str, result: &T)
where
    T: Serialize,
{
    let directory = cache_dir();
    if fs::create_dir_all(&directory).is_err() {
        return;
    }
    set_private_directory_permissions(&directory);

    let path = cache_path(provider);
    let temp_path = directory.join(format!(
        ".{provider}-usage-{}-{}.tmp",
        std::process::id(),
        rand::thread_rng().r#gen::<u64>()
    ));
    let payload = json!({
        "cached_at": epoch_seconds(),
        "result": result,
    });
    if write_private_file(&temp_path, payload.to_string().as_bytes()).is_err() {
        let _ = fs::remove_file(temp_path);
        return;
    }
    if fs::rename(&temp_path, path).is_err() {
        let _ = fs::remove_file(temp_path);
    }
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) {}

fn write_private_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(contents)
}

fn epoch_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or_default()
}

pub(crate) fn load_amp_request_ledger() -> AmpRequestLedger {
    read_usage_cache::<AmpRequestLedger>("amp-requests", None)
        .map(|(ledger, _)| ledger)
        .unwrap_or_default()
}

pub(crate) fn prime_usage_caches(state: &Arc<Mutex<AppState>>) {
    if let Some((result, updated_at)) = read_usage_cache::<AmpUsage>("amp", None) {
        let mut state = state.lock().unwrap();
        state.amp.result = Some(result);
        state.amp.updated_at = Some(updated_at);
        state.amp.stale = true;
    }
    if let Some((result, updated_at)) = read_usage_cache::<AmpActivityUsage>("amp-activity", None) {
        let mut state = state.lock().unwrap();
        state.amp_activity.result = Some(result);
        state.amp_activity.updated_at = Some(updated_at);
        state.amp_activity.stale = true;
    }
    if let Some((result, updated_at)) = read_usage_cache::<Value>("codex", None) {
        let mut state = state.lock().unwrap();
        state.codex.result = Some(result);
        state.codex.updated_at = Some(updated_at);
    }
    if let Some((result, updated_at)) =
        read_usage_cache::<CodexActivityUsage>("codex-activity", None)
    {
        let mut state = state.lock().unwrap();
        state.codex_activity.result = Some(result);
        state.codex_activity.updated_at = Some(updated_at);
    }
}

pub(crate) fn load_cached_amp(state: &Arc<Mutex<AppState>>, error: String) {
    let cached = read_usage_cache::<AmpUsage>("amp", Some(env_u64("STATS_USAGE_CACHE_TTL", 600)))
        .or_else(|| read_usage_cache::<AmpUsage>("amp", None));
    let mut state = state.lock().unwrap();
    if let Some((result, updated_at)) = cached {
        state.amp.result = Some(result);
        state.amp.updated_at = Some(updated_at);
        state.amp.error = None;
        state.amp.stale = true;
    } else {
        state.amp.error = Some(error);
    }
}

pub(crate) fn load_cached_codex(state: &Arc<Mutex<AppState>>, error: String) {
    let cached = read_usage_cache::<Value>("codex", Some(env_u64("STATS_USAGE_CACHE_TTL", 600)))
        .or_else(|| read_usage_cache::<Value>("codex", None));
    let mut state = state.lock().unwrap();
    if let Some((result, updated_at)) = cached {
        state.codex.result = Some(result);
        state.codex.updated_at = Some(updated_at);
        state.codex.error = None;
    } else {
        state.codex.error = Some(error);
    }
}

pub(crate) fn load_cached_codex_activity(state: &Arc<Mutex<AppState>>, error: String) {
    let cached = read_usage_cache::<CodexActivityUsage>(
        "codex-activity",
        Some(env_u64("STATS_USAGE_CACHE_TTL", 600)),
    )
    .or_else(|| read_usage_cache::<CodexActivityUsage>("codex-activity", None));
    let mut state = state.lock().unwrap();
    if let Some((result, updated_at)) = cached {
        state.codex_activity.result = Some(result);
        state.codex_activity.updated_at = Some(updated_at);
        state.codex_activity.error = None;
    } else {
        state.codex_activity.error = Some(error);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::env;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn restricts_usage_cache_to_the_current_user() {
        let directory = env::temp_dir().join(format!(
            "stats-cache-permissions-{}-{}",
            std::process::id(),
            rand::thread_rng().r#gen::<u64>()
        ));
        fs::create_dir(&directory).unwrap();
        set_private_directory_permissions(&directory);
        let file = directory.join("usage.json");
        write_private_file(&file, b"{}").unwrap();
        assert_eq!(
            fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
