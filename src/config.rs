use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use chrono_tz::Tz;
use serde::Deserialize;

use crate::model::Clock;

const CONFIG_VERSION: u64 = 2;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Config {
    pub(crate) version: u64,
    pub(crate) clocks: Vec<Clock>,
    pub(crate) sections: SectionsConfig,
    pub(crate) section_display: SectionDisplayConfig,
    pub(crate) refresh: RefreshConfig,
    pub(crate) desktop: DesktopConfig,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct SectionsConfig {
    pub(crate) clocks: bool,
    pub(crate) system: bool,
    pub(crate) ai: bool,
    pub(crate) amp_activity: bool,
    pub(crate) codex_activity: bool,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub(crate) struct SectionDisplayConfig {
    pub(crate) clocks: ClocksDisplayConfig,
    pub(crate) system: SystemDisplayConfig,
    pub(crate) ai: AiDisplayConfig,
    pub(crate) amp_activity: AmpActivityDisplayConfig,
    pub(crate) codex_activity: CodexActivityDisplayConfig,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct ClocksDisplayConfig {
    pub(crate) heading: bool,
    pub(crate) clock_1: bool,
    pub(crate) clock_2: bool,
    pub(crate) clock_3: bool,
    pub(crate) clock_4: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct SystemDisplayConfig {
    pub(crate) heading: bool,
    pub(crate) cpu: bool,
    pub(crate) ram: bool,
    pub(crate) gpu: bool,
    pub(crate) storage: bool,
    pub(crate) network: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct AiDisplayConfig {
    pub(crate) heading: bool,
    pub(crate) amp_plan: bool,
    pub(crate) amp_orbs: bool,
    pub(crate) amp_credits: bool,
    pub(crate) codex_quota: bool,
    #[serde(default)]
    pub(crate) claude_quota: bool,
    #[serde(default)]
    pub(crate) antigravity_quota: bool,
    #[serde(default)]
    pub(crate) cursor_quota: bool,
    #[serde(default)]
    pub(crate) grok_quota: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct AmpActivityDisplayConfig {
    pub(crate) heading: bool,
    pub(crate) calendar: bool,
    pub(crate) daily_activity: bool,
    pub(crate) usage_summary: bool,
    pub(crate) models: bool,
    pub(crate) sources: bool,
    pub(crate) sync_alerts: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct CodexActivityDisplayConfig {
    pub(crate) heading: bool,
    pub(crate) calendar: bool,
    pub(crate) overview: bool,
    pub(crate) daily_activity: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RefreshConfig {
    pub(crate) codex_seconds: u64,
    pub(crate) amp_seconds: u64,
    #[serde(default = "default_claude_seconds")]
    pub(crate) claude_seconds: u64,
    #[serde(default = "default_quota_seconds")]
    pub(crate) quota_seconds: u64,
    pub(crate) storage_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
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
            section_display: SectionDisplayConfig::default(),
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

macro_rules! all_true_default {
    ($type:ty { $($field:ident),+ $(,)? }) => {
        impl Default for $type {
            fn default() -> Self {
                Self { $($field: true),+ }
            }
        }
    };
}

all_true_default!(ClocksDisplayConfig {
    heading,
    clock_1,
    clock_2,
    clock_3,
    clock_4
});
all_true_default!(SystemDisplayConfig {
    heading,
    cpu,
    ram,
    gpu,
    storage,
    network
});
impl Default for AiDisplayConfig {
    fn default() -> Self {
        Self {
            heading: true,
            amp_plan: true,
            amp_orbs: true,
            amp_credits: true,
            codex_quota: true,
            claude_quota: true,
            antigravity_quota: false,
            cursor_quota: false,
            grok_quota: false,
        }
    }
}
all_true_default!(AmpActivityDisplayConfig {
    heading,
    calendar,
    daily_activity,
    usage_summary,
    models,
    sources,
    sync_alerts
});
all_true_default!(CodexActivityDisplayConfig {
    heading,
    calendar,
    overview,
    daily_activity
});

impl SectionDisplayConfig {
    pub(crate) fn system_needed(&self, sections: &SectionsConfig) -> bool {
        sections.system
            && (self.system.cpu
                || self.system.ram
                || self.system.gpu
                || self.system.storage
                || self.system.network)
    }

    pub(crate) fn amp_ai_needed(&self, sections: &SectionsConfig) -> bool {
        sections.ai && (self.ai.amp_plan || self.ai.amp_orbs || self.ai.amp_credits)
    }

    pub(crate) fn codex_ai_needed(&self, sections: &SectionsConfig) -> bool {
        sections.ai && self.ai.codex_quota
    }

    pub(crate) fn claude_ai_needed(&self, sections: &SectionsConfig) -> bool {
        sections.ai && self.ai.claude_quota
    }

    pub(crate) fn antigravity_ai_needed(&self, sections: &SectionsConfig) -> bool {
        sections.ai && self.ai.antigravity_quota
    }

    pub(crate) fn cursor_ai_needed(&self, sections: &SectionsConfig) -> bool {
        sections.ai && self.ai.cursor_quota
    }

    pub(crate) fn grok_ai_needed(&self, sections: &SectionsConfig) -> bool {
        sections.ai && self.ai.grok_quota
    }

    pub(crate) fn amp_activity_needed(&self, sections: &SectionsConfig) -> bool {
        sections.amp_activity
            && (self.amp_activity.calendar
                || self.amp_activity.daily_activity
                || self.amp_activity.usage_summary
                || self.amp_activity.models
                || self.amp_activity.sources
                || self.amp_activity.sync_alerts)
    }

    pub(crate) fn codex_activity_needed(&self, sections: &SectionsConfig) -> bool {
        sections.codex_activity
            && (self.codex_activity.calendar
                || self.codex_activity.overview
                || self.codex_activity.daily_activity)
    }
}

impl Default for RefreshConfig {
    fn default() -> Self {
        Self {
            codex_seconds: 60,
            amp_seconds: 300,
            claude_seconds: default_claude_seconds(),
            quota_seconds: default_quota_seconds(),
            storage_seconds: 300,
        }
    }
}

fn default_claude_seconds() -> u64 {
    300
}

fn default_quota_seconds() -> u64 {
    300
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
    let requirements = [
        (
            config.sections.clocks,
            config.section_display.clocks.any(),
            "clocks",
        ),
        (
            config.sections.system,
            config.section_display.system.any(),
            "system",
        ),
        (config.sections.ai, config.section_display.ai.any(), "ai"),
        (
            config.sections.amp_activity,
            config.section_display.amp_activity.any(),
            "amp_activity",
        ),
        (
            config.sections.codex_activity,
            config.section_display.codex_activity.any(),
            "codex_activity",
        ),
    ];
    for (enabled, any, section) in requirements {
        if enabled && !any {
            return Err(format!(
                "sections.{section} requires at least one section_display.{section} option"
            ));
        }
    }
    if config.refresh.codex_seconds < 5 {
        return Err("refresh.codex_seconds must be at least 5".into());
    }
    if config.refresh.amp_seconds < 60 {
        return Err("refresh.amp_seconds must be at least 60".into());
    }
    if config.refresh.claude_seconds < 60 {
        return Err("refresh.claude_seconds must be at least 60".into());
    }
    if config.refresh.quota_seconds < 60 {
        return Err("refresh.quota_seconds must be at least 60".into());
    }
    if config.refresh.storage_seconds < 60 {
        return Err("refresh.storage_seconds must be at least 60".into());
    }
    if !(10..=24).contains(&config.desktop.font_size) {
        return Err("desktop.font_size must be between 10 and 24".into());
    }
    Ok(())
}

macro_rules! any_enabled {
    ($type:ty { $($field:ident),+ $(,)? }) => {
        impl $type {
            fn any(&self) -> bool { false $(|| self.$field)+ }
        }
    };
}

any_enabled!(ClocksDisplayConfig {
    heading,
    clock_1,
    clock_2,
    clock_3,
    clock_4
});
any_enabled!(SystemDisplayConfig {
    heading,
    cpu,
    ram,
    gpu,
    storage,
    network
});
any_enabled!(AiDisplayConfig {
    heading,
    amp_plan,
    amp_orbs,
    amp_credits,
    codex_quota,
    claude_quota,
    antigravity_quota,
    cursor_quota,
    grok_quota
});
any_enabled!(AmpActivityDisplayConfig {
    heading,
    calendar,
    daily_activity,
    usage_summary,
    models,
    sources,
    sync_alerts
});
any_enabled!(CodexActivityDisplayConfig {
    heading,
    calendar,
    overview,
    daily_activity
});

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
        assert_eq!(config.version, 2);
        assert_eq!(config.clocks.len(), 4);
        assert!(config.sections.amp_activity);
        assert_eq!(config.desktop.font_size, 15);
        assert!(!config.desktop.show_scrollbar);
    }

    #[test]
    fn rejects_version_one_config() {
        let path = temporary_path("version-one");
        write(&path, &valid_config().replace("version = 2", "version = 1"));

        let error = load(&path).unwrap_err();

        assert!(error.contains("unsupported version 1"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_missing_section_display_table() {
        let path = temporary_path("missing-display-table");
        write(
            &path,
            &valid_config().replace("[section_display.clocks]", "[removed_clocks]"),
        );

        let error = load(&path).unwrap_err();

        assert!(error.contains("missing field `clocks`"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_missing_display_option() {
        let path = temporary_path("missing-display-option");
        write(&path, &valid_config().replace("clock_1 = true\n", ""));

        let error = load(&path).unwrap_err();

        assert!(error.contains("missing field `clock_1`"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn enabled_section_requires_a_display_option() {
        let path = temporary_path("empty-enabled-section");
        write(&path, &empty_system_config(true));

        let error = load(&path).unwrap_err();

        assert!(
            error.contains("sections.system requires at least one section_display.system option")
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn disabled_section_allows_no_display_options() {
        let path = temporary_path("empty-disabled-section");
        write(&path, &empty_system_config(false));

        let config = load(&path).unwrap();

        assert!(!config.sections.system);
        assert!(!config.section_display.system.any());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn provider_demand_ignores_heading_only_sections() {
        let sections = SectionsConfig::default();
        let display = SectionDisplayConfig {
            system: SystemDisplayConfig {
                heading: true,
                cpu: false,
                ram: false,
                gpu: false,
                storage: false,
                network: false,
            },
            ai: AiDisplayConfig {
                heading: true,
                amp_plan: false,
                amp_orbs: false,
                amp_credits: false,
                codex_quota: false,
                claude_quota: false,
                antigravity_quota: false,
                cursor_quota: false,
                grok_quota: false,
            },
            amp_activity: AmpActivityDisplayConfig {
                heading: true,
                calendar: false,
                daily_activity: false,
                usage_summary: false,
                models: false,
                sources: false,
                sync_alerts: false,
            },
            codex_activity: CodexActivityDisplayConfig {
                heading: true,
                calendar: false,
                overview: false,
                daily_activity: false,
            },
            ..SectionDisplayConfig::default()
        };

        assert!(!display.system_needed(&sections));
        assert!(!display.amp_ai_needed(&sections));
        assert!(!display.codex_ai_needed(&sections));
        assert!(!display.claude_ai_needed(&sections));
        assert!(!display.antigravity_ai_needed(&sections));
        assert!(!display.cursor_ai_needed(&sections));
        assert!(!display.grok_ai_needed(&sections));
        assert!(!display.amp_activity_needed(&sections));
        assert!(!display.codex_activity_needed(&sections));
    }

    #[test]
    fn loads_valid_config() {
        let path = temporary_path("valid");
        write(
            &path,
            &valid_config()
                .replace("label = \"Mumbai\"", "label = \"London\"")
                .replace("clocks = true", "clocks = false")
                .replace("ai = true", "ai = false")
                .replace("amp_activity = true", "amp_activity = false")
                .replace("codex_seconds = 60", "codex_seconds = 10")
                .replace("amp_seconds = 300", "amp_seconds = 120")
                .replace("claude_seconds = 300", "claude_seconds = 180")
                .replace("storage_seconds = 300", "storage_seconds = 180")
                .replace("font_size = 15", "font_size = 18"),
        );

        let config = load(&path).unwrap();
        assert_eq!(config.clocks[0].label, "London");
        assert!(!config.sections.clocks);
        assert!(!config.sections.ai);
        assert!(!config.sections.amp_activity);
        assert!(config.sections.codex_activity);
        assert_eq!(config.refresh.codex_seconds, 10);
        assert_eq!(config.refresh.claude_seconds, 180);
        assert_eq!(config.desktop.font_size, 18);
        assert!(!config.desktop.show_scrollbar);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn loads_config_created_before_claude_integration() {
        let path = temporary_path("pre-claude");
        write(
            &path,
            &valid_config()
                .replace("claude_quota = true\n", "")
                .replace("antigravity_quota = false\n", "")
                .replace("cursor_quota = false\n", "")
                .replace("grok_quota = false\n", "")
                .replace("claude_seconds = 300\n", "")
                .replace("quota_seconds = 300\n", ""),
        );

        let config = load(&path).unwrap();

        assert!(!config.section_display.ai.claude_quota);
        assert!(!config.section_display.ai.antigravity_quota);
        assert!(!config.section_display.ai.cursor_quota);
        assert!(!config.section_display.ai.grok_quota);
        assert_eq!(config.refresh.claude_seconds, 300);
        assert_eq!(config.refresh.quota_seconds, 300);
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
        write(&path, &valid_config().replace("version = 2", "version = 3"));
        let error = load(&path).unwrap_err();
        assert!(error.contains("unsupported version 3"));
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
        r#"version = 2

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

[sections]
clocks = true
system = true
ai = true
amp_activity = true
codex_activity = true

[section_display.clocks]
heading = true
clock_1 = true
clock_2 = true
clock_3 = true
clock_4 = true

[section_display.system]
heading = true
cpu = true
ram = true
gpu = true
storage = true
network = true

[section_display.ai]
heading = true
amp_plan = true
amp_orbs = true
amp_credits = true
codex_quota = true
claude_quota = true
antigravity_quota = false
cursor_quota = false
grok_quota = false

[section_display.amp_activity]
heading = true
calendar = true
daily_activity = true
usage_summary = true
models = true
sources = true
sync_alerts = true

[section_display.codex_activity]
heading = true
calendar = true
overview = true
daily_activity = true

[refresh]
codex_seconds = 60
amp_seconds = 300
claude_seconds = 300
quota_seconds = 300
storage_seconds = 300

[desktop]
font_size = 15
show_scrollbar = false
"#
        .into()
    }

    fn empty_system_config(enabled: bool) -> String {
        valid_config()
            .replace("system = true", &format!("system = {enabled}"))
            .replace(
                "[section_display.system]\nheading = true\ncpu = true\nram = true\ngpu = true\nstorage = true\nnetwork = true",
                "[section_display.system]\nheading = false\ncpu = false\nram = false\ngpu = false\nstorage = false\nnetwork = false",
            )
    }
}
