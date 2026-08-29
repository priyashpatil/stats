use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use chrono::Local;

use crate::cache::{read_cached_quota, write_usage_cache};
use crate::model::{AppState, ProviderState, QuotaUsage};
use crate::worker::sleep_stop;

pub(crate) mod amp;
pub(crate) mod antigravity;
pub(crate) mod claude;
pub(crate) mod codex;
pub(crate) mod cursor;
pub(crate) mod grok;

pub(crate) fn spawn_quota_refresh(
    state: &Arc<Mutex<AppState>>,
    stop: &Arc<AtomicBool>,
    interval: u64,
    provider: &'static str,
    select: fn(&mut AppState) -> &mut ProviderState<QuotaUsage>,
    read: fn() -> Result<QuotaUsage, String>,
) {
    let state = Arc::clone(state);
    let stop = Arc::clone(stop);
    thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            match read() {
                Ok(result) => {
                    write_usage_cache(provider, &result);
                    let mut guard = state.lock().unwrap();
                    let provider_state = select(&mut guard);
                    provider_state.result = Some(result);
                    provider_state.error = None;
                    provider_state.updated_at = Some(Local::now());
                    provider_state.stale = false;
                }
                Err(error) => {
                    let cached = read_cached_quota(provider);
                    let mut guard = state.lock().unwrap();
                    let provider_state = select(&mut guard);
                    if let Some((result, updated_at)) = cached {
                        provider_state.result = Some(result);
                        provider_state.updated_at = Some(updated_at);
                        provider_state.error = None;
                        provider_state.stale = true;
                    } else {
                        provider_state.error = Some(error);
                    }
                }
            }
            sleep_stop(&stop, Duration::from_secs(interval));
        }
    });
}
