use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use chrono_tz::Tz;
use serde::Deserialize;

use crate::model::Clock;

const CONFIG_VERSION: u64 = 1;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub(crate) struct Config {
    pub(crate) version: u64,
    pub(crate) clocks: Vec<Clock>,
    pub(crate) sections: SectionsConfig,
    pub(crate) refresh: RefreshConfig,
    pub(crate) desktop: DesktopConfig,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(default)]
pub(crate) struct SectionsConfig {
    pub(crate) clocks: bool,
    pub(crate) system: bool,
    pub(crate) ai: bool,
    pub(crate) amp_activity: bool,
    pub(crate) codex_activity: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub(crate) struct RefreshConfig {
    pub(crate) codex_seconds: u64,
    pub(crate) amp_seconds: u64,
    pub(crate) storage_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub(crate) struct DesktopConfig {
    pub(crate) font_size: u64,
    pub(crate) show_scrollbar: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            clocks: default_clocks(),
            sections: SectionsConfig::default(),
            refresh: RefreshConfig::default(),
            desktop: DesktopConfig::default(),
        }
    }
}

impl Default for SectionsConfig {
    fn default() -> Self {
        Self {
            clocks: true,
            system: true,
            ai: true,
            amp_activity: true,
            codex_activity: true,
        }
    }
}

impl Default for RefreshConfig {
    fn default() -> Self {
        Self {
            codex_seconds: 60,
            amp_seconds: 300,
            storage_seconds: 300,
        }
    }
}

impl Default for DesktopConfig {
    fn default() -> Self {
        Self {
            font_size: 15,
            show_scrollbar: false,
        }
    }
}

pub(crate) fn default_path() -> Result<PathBuf, String> {
    path_from(env::var_os("XDG_CONFIG_HOME"), dirs::home_dir())
}

fn path_from(xdg_config_home: Option<OsString>, home: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(path) = xdg_config_home.filter(|value| !value.is_empty()) {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            return Ok(path.join("stats/config.toml"));
        }
    }

    home.map(|path| path.join(".config/stats/config.toml"))
        .ok_or_else(|| "could not determine the home directory for the Stats config".into())
}

pub(crate) fn load(path: &Path) -> Result<Config, String> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(err) => return Err(format!("could not read config {}: {err}", path.display())),
    };
    let config: Config = toml::from_str(&contents)
        .map_err(|err| format!("invalid config {}: {err}", path.display()))?;
    validate(&config).map_err(|err| format!("invalid config {}: {err}", path.display()))?;
    Ok(config)
}

fn validate(config: &Config) -> Result<(), String> {
    if config.version != CONFIG_VERSION {
        return Err(format!(
            "unsupported version {}; expected {CONFIG_VERSION}",
            config.version
        ));
    }
    validate_clocks(&config.clocks)?;
    if config.refresh.codex_seconds < 5 {
        return Err("refresh.codex_seconds must be at least 5".into());
    }
    if config.refresh.amp_seconds < 60 {
        return Err("refresh.amp_seconds must be at least 60".into());
    }
    if config.refresh.storage_seconds < 60 {
        return Err("refresh.storage_seconds must be at least 60".into());
    }
    if !(10..=24).contains(&config.desktop.font_size) {
        return Err("desktop.font_size must be between 10 and 24".into());
    }
    Ok(())
}

pub(crate) fn validate_clocks(clocks: &[Clock]) -> Result<(), String> {
    if clocks.len() != 4 {
        return Err("clocks must contain exactly 4 entries".into());
    }
    for clock in clocks {
        if clock.label.trim().is_empty() {
            return Err("clock labels cannot be empty".into());
        }
        if clock.timezone.parse::<Tz>().is_err() {
            return Err(format!("unknown clock timezone: {}", clock.timezone));
        }
    }
    Ok(())
}

fn default_clocks() -> Vec<Clock> {
    [
        ("Mumbai", "Asia/Kolkata"),
        ("Paris", "Europe/Paris"),
        ("Sydney", "Australia/Sydney"),
        ("Seattle", "America/Los_Angeles"),
    ]
    .into_iter()
    .map(|(label, timezone)| Clock {
        label: label.into(),
        timezone: timezone.into(),
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn uses_xdg_config_home_when_absolute() {
        let path = path_from(Some(OsString::from("/tmp/config")), None).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/config/stats/config.toml"));
    }

    #[test]
    fn falls_back_to_dot_config_for_relative_xdg_path() {
        let path = path_from(
            Some(OsString::from("relative")),
            Some(PathBuf::from("/Users/example")),
        )
        .unwrap();
        assert_eq!(
            path,
            PathBuf::from("/Users/example/.config/stats/config.toml")
        );
    }

    #[test]
    fn missing_file_uses_defaults() {
        let path = temporary_path("missing");
        let config = load(&path).unwrap();
        assert_eq!(config.version, 1);
        assert_eq!(config.clocks.len(), 4);
        assert!(config.sections.amp_activity);
        assert_eq!(config.desktop.font_size, 15);
        assert!(!config.desktop.show_scrollbar);
    }

    #[test]
    fn loads_valid_config() {
        let path = temporary_path("valid");
        write(
            &path,
            r#"
version = 1

[[clocks]]
label = "London"
timezone = "Europe/London"
[[clocks]]
label = "New York"
timezone = "America/New_York"
[[clocks]]
label = "Tokyo"
timezone = "Asia/Tokyo"
[[clocks]]
label = "Sydney"
timezone = "Australia/Sydney"

[sections]
clocks = false
system = true
ai = false
amp_activity = false
codex_activity = true

[refresh]
codex_seconds = 10
amp_seconds = 120
storage_seconds = 180

[desktop]
font_size = 18
show_scrollbar = false
"#,
        );

        let config = load(&path).unwrap();
        assert_eq!(config.clocks[0].label, "London");
        assert!(!config.sections.clocks);
        assert!(!config.sections.ai);
        assert!(!config.sections.amp_activity);
        assert!(config.sections.codex_activity);
        assert_eq!(config.refresh.codex_seconds, 10);
        assert_eq!(config.desktop.font_size, 18);
        assert!(!config.desktop.show_scrollbar);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_invalid_timezone() {
        let path = temporary_path("timezone");
        write(
            &path,
            &valid_config().replace("Asia/Kolkata", "Nowhere/Unknown"),
        );
        let error = load(&path).unwrap_err();
        assert!(error.contains("unknown clock timezone"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_unsupported_version() {
        let path = temporary_path("version");
        write(&path, &valid_config().replace("version = 1", "version = 2"));
        let error = load(&path).unwrap_err();
        assert!(error.contains("unsupported version 2"));
        fs::remove_file(path).unwrap();
    }

    fn temporary_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("stats-{name}-{nonce}.toml"))
    }

    fn write(path: &Path, contents: &str) {
        fs::write(path, contents).unwrap();
    }

    fn valid_config() -> String {
        r#"version = 1

[[clocks]]
label = "Mumbai"
timezone = "Asia/Kolkata"
[[clocks]]
label = "Paris"
timezone = "Europe/Paris"
[[clocks]]
label = "Sydney"
timezone = "Australia/Sydney"
[[clocks]]
label = "Seattle"
timezone = "America/Los_Angeles"
"#
        .into()
    }
}
