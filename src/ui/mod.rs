use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, Terminal};
use serde_json::Value;

const CLOCKS: &[(&str, &str)] = &[
    ("Mumbai", "Asia/Kolkata"),
    ("Paris", "Europe/Paris"),
    ("Sydney", "Australia/Sydney"),
    ("Seattle", "America/Los_Angeles"),
];

const BAR_FILLED: &str = "━";
const BAR_EMPTY: &str = "·";

const CODEX_GUTTER_WIDTH: usize = 9;

#[derive(Debug, Clone)]
struct AiQuotaRow {
    label: String,
    percent_left: f64,
    reset: Option<String>,
    suffix: Option<String>,
}

fn equal_column_widths(width: usize, count: usize) -> Vec<usize> {
    let base = width / count;
    let remainder = width % count;
    (0..count)
        .map(|index| base + usize::from(index < remainder))
        .collect()
}

use crate::model::{AmpUsage, AppState, CodexActivityUsage, ProviderState, SystemMetrics};
use crate::providers::codex::{codex_weekly_window, left_percent, ordered_buckets};

mod activity;
use activity::render_codex_activity;

pub(crate) fn run_tui(state: &Arc<Mutex<AppState>>, stop: &Arc<AtomicBool>) -> Result<(), String> {
    enable_raw_mode().map_err(|err| err.to_string())?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        Clear(ClearType::Purge),
        Clear(ClearType::All),
        MoveTo(0, 0),
        Hide,
        EnableMouseCapture
    )
    .map_err(|err| err.to_string())?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|err| err.to_string())?;
    let result = loop {
        terminal
            .draw(|frame| draw(frame, state))
            .map_err(|err| err.to_string())?;
        if event::poll(Duration::from_millis(200)).map_err(|err| err.to_string())?
            && let Event::Key(key) = event::read().map_err(|err| err.to_string())?
            && matches!(
                key.code,
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc
            )
        {
            stop.store(true, Ordering::Relaxed);
            break Ok(());
        }
        if stop.load(Ordering::Relaxed) {
            break Ok(());
        }
    };
    disable_raw_mode().map_err(|err| err.to_string())?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        Show,
        LeaveAlternateScreen
    )
    .map_err(|err| err.to_string())?;
    terminal.show_cursor().map_err(|err| err.to_string())?;
    result
}

fn draw(frame: &mut Frame, state: &Arc<Mutex<AppState>>) {
    let area = frame.area();
    let snapshot = state.lock().unwrap().clone();
    let lines = stats_lines(&snapshot, area.width as usize);
    let paragraph = Paragraph::new(Text::from(lines));
    frame.render_widget(paragraph, area);
}

fn stats_lines(state: &AppState, width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    render_clocks(&mut lines, width);
    render_system(&mut lines, &state.system, width);
    render_ai(
        &mut lines,
        &state.amp,
        &state.codex,
        &state.codex_activity,
        width,
    );
    lines
}

fn render_clocks(lines: &mut Vec<Line<'static>>, width: usize) {
    let gap = if width >= 72 { 4 } else { 2 };
    let card_widths =
        equal_column_widths(width.saturating_sub(gap * (CLOCKS.len() - 1)), CLOCKS.len());
    if card_widths.contains(&0) {
        return;
    }
    let mut city_spans = Vec::new();
    let mut time_spans = Vec::new();
    let now = Local::now();
    for (index, ((city, zone_name), card_width)) in CLOCKS.iter().zip(card_widths).enumerate() {
        let zone: Tz = zone_name.parse().unwrap_or(chrono_tz::UTC);
        let zoned = now.with_timezone(&zone);
        if index > 0 {
            city_spans.push(Span::raw(" ".repeat(gap)));
            time_spans.push(Span::raw(" ".repeat(gap)));
        }
        city_spans.push(span(
            fixed(&city.to_uppercase(), card_width),
            Color::Cyan,
            true,
        ));
        time_spans.push(Span::styled(
            fixed(
                &zoned.format("%I:%M %p").to_string().to_lowercase(),
                card_width,
            ),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
    }
    lines.push(Line::from(city_spans));
    lines.push(Line::from(time_spans));
    lines.push(Line::default());
}

fn render_system(lines: &mut Vec<Line<'static>>, system: &SystemMetrics, width: usize) {
    let mut rows = vec![
        metric_row("CPU", system.cpu_percent, "used", true, width),
        metric_row("RAM", Some(system.ram_percent), "used", true, width),
        metric_row("GPU", system.gpu_percent, "used", true, width),
    ];
    let used_percent = (100.0 - system.storage_percent_free).clamp(0.0, 100.0);
    let color = color_for_usage(used_percent);
    let mut storage = vec![dim(fixed("Storage", 8))];
    let storage_value = format!("{:>3}% free", system.storage_percent_free.round() as i64);
    storage.extend(bar_spans(
        used_percent,
        metric_bar_width(width, 8, storage_value.chars().count()),
        color,
    ));
    storage.extend([
        Span::raw("  "),
        span(
            storage_value,
            color_for_remaining(system.storage_percent_free),
            true,
        ),
    ]);
    rows.push(Line::from(storage));
    rows.push(Line::from(vec![
        dim(fixed("Network", 8)),
        span(
            format!("↓ {}", rate_label(system.net_down_rate)),
            Color::Green,
            true,
        ),
        Span::raw("  "),
        span(
            format!("↑ {}", rate_label(system.net_up_rate)),
            Color::Green,
            true,
        ),
    ]));
    section(lines, "System", "", width);
    lines.push(Line::default());
    lines.extend(rows);
    lines.push(Line::default());
}

fn render_ai(
    lines: &mut Vec<Line<'static>>,
    amp: &ProviderState<AmpUsage>,
    codex: &ProviderState<Value>,
    codex_activity: &ProviderState<CodexActivityUsage>,
    width: usize,
) {
    render_ai_at(
        lines,
        amp,
        codex,
        codex_activity,
        width,
        Utc::now().date_naive(),
    );
}

fn render_ai_at(
    lines: &mut Vec<Line<'static>>,
    amp: &ProviderState<AmpUsage>,
    codex: &ProviderState<Value>,
    codex_activity: &ProviderState<CodexActivityUsage>,
    width: usize,
    today: NaiveDate,
) {
    section(lines, "AI", "", width);
    lines.push(Line::default());

    let mut rows = Vec::new();
    let mut statuses = Vec::new();
    collect_amp_ai_rows(&mut rows, &mut statuses, amp);
    collect_codex_ai_rows(&mut rows, &mut statuses, codex);

    for status in statuses {
        lines.push(status);
    }

    if !rows.is_empty() {
        render_ai_quota_rows(lines, rows, width);
    }
    lines.push(Line::default());
    render_codex_activity(lines, codex_activity, width, today);

    lines.push(Line::default());
}

fn collect_amp_ai_rows(
    rows: &mut Vec<AiQuotaRow>,
    statuses: &mut Vec<Line<'static>>,
    amp: &ProviderState<AmpUsage>,
) {
    if let Some(error) = &amp.error {
        statuses.push(ai_status_row(
            "Megawatt",
            format!("Error: {error}"),
            Color::Red,
        ));
        return;
    }
    let Some(result) = &amp.result else {
        statuses.push(ai_status_row(
            "Megawatt",
            "Loading Amp usage...",
            Color::Yellow,
        ));
        return;
    };
    if let Some(percent_left) = result.other_percent_remaining {
        rows.push(AiQuotaRow {
            label: result.plan.clone().unwrap_or_else(|| "Megawatt".into()),
            percent_left,
            reset: result.reset.as_deref().map(amp_compact_reset_label),
            suffix: None,
        });
    }
}

fn amp_compact_reset_label(reset: &str) -> String {
    amp_compact_reset_label_at(reset, Local::now().date_naive())
}

fn amp_compact_reset_label_at(reset: &str, today: NaiveDate) -> String {
    let reset = reset.strip_prefix("resets upon renewal ").unwrap_or(reset);
    let Some(duration) = reset.strip_prefix("in ") else {
        return reset.to_string();
    };
    let mut parts = duration.split_whitespace();
    let (Some(amount), Some(unit), None) = (parts.next(), parts.next(), parts.next()) else {
        return reset.to_string();
    };
    let count = amount.replace(',', "").parse::<u32>().ok();
    let reset_date = match (count, unit) {
        (Some(count), "day" | "days") => {
            today.checked_add_signed(chrono::Duration::days(i64::from(count)))
        }
        (Some(count), "week" | "weeks") => {
            today.checked_add_signed(chrono::Duration::days(i64::from(count).saturating_mul(7)))
        }
        (Some(count), "month" | "months") => today.checked_add_months(chrono::Months::new(count)),
        _ => None,
    };
    if let Some(reset_date) = reset_date {
        return reset_date.format("%-d %b").to_string();
    }
    let unit = match unit {
        "minute" | "minutes" => "m",
        "hour" | "hours" => "h",
        _ => return reset.to_string(),
    };
    format!("in {amount}{unit}")
}

fn collect_codex_ai_rows(
    rows: &mut Vec<AiQuotaRow>,
    statuses: &mut Vec<Line<'static>>,
    codex: &ProviderState<Value>,
) {
    if let Some(error) = &codex.error {
        statuses.push(ai_status_row(
            "Codex Pro",
            format!("Error: {error}"),
            Color::Red,
        ));
        return;
    }
    let Some(result) = &codex.result else {
        statuses.push(ai_status_row(
            "Codex Pro",
            "Loading Codex usage status...",
            Color::Yellow,
        ));
        return;
    };
    for snapshot in ordered_buckets(result) {
        if let Some(window) = codex_weekly_window(snapshot) {
            rows.push(AiQuotaRow {
                label: "Codex Pro".into(),
                percent_left: left_percent(window),
                reset: Some(codex_compact_reset_label(window)),
                suffix: None,
            });
        }
    }
}

fn render_ai_quota_rows(lines: &mut Vec<Line<'static>>, rows: Vec<AiQuotaRow>, width: usize) {
    const VALUE_WIDTH: usize = 9;
    const LABEL_GAP: usize = 1;
    const VALUE_GAP: usize = 2;
    const TAIL_GAP: usize = 2;

    let tails = rows.iter().map(ai_quota_tail).collect::<Vec<_>>();
    let tail_width = tails
        .iter()
        .map(|tail| tail.chars().count())
        .max()
        .unwrap_or_default();
    let fixed_width = CODEX_GUTTER_WIDTH + LABEL_GAP + VALUE_GAP + VALUE_WIDTH;
    let available = width.saturating_sub(fixed_width);
    let show_tail = tail_width > 0 && available >= 4 + TAIL_GAP + tail_width;
    let bar_width = if show_tail {
        available.saturating_sub(TAIL_GAP + tail_width)
    } else {
        available
    };

    for (row, tail) in rows.into_iter().zip(tails) {
        if width < fixed_width {
            let label_width = CODEX_GUTTER_WIDTH.min(width);
            let value_width = width.saturating_sub(label_width);
            let color = color_for_remaining(row.percent_left);
            let mut spans = vec![dim(fixed(&row.label, label_width))];
            let full_value = format!("{}% left", row.percent_left.round() as i64);
            let compact_value = format!("{}%", row.percent_left.round() as i64);
            if value_width >= full_value.chars().count() {
                spans.push(span(fixed(&full_value, value_width), color, true));
            } else if value_width >= compact_value.chars().count() {
                spans.push(span(fixed(&compact_value, value_width), color, true));
            }
            lines.push(Line::from(spans));
            continue;
        }
        let color = color_for_remaining(row.percent_left);
        let mut spans = vec![
            dim(fixed(&row.label, CODEX_GUTTER_WIDTH)),
            Span::raw(" ".repeat(LABEL_GAP)),
        ];
        if bar_width >= 4 {
            spans.extend(bar_spans(row.percent_left, bar_width, color));
        }
        spans.push(Span::raw(" ".repeat(VALUE_GAP)));
        spans.push(span(
            format!("{:>3}% left", row.percent_left.round() as i64),
            color,
            true,
        ));
        if show_tail {
            spans.push(Span::raw(" ".repeat(TAIL_GAP)));
            spans.push(dim(fixed(&tail, tail_width)));
        }
        lines.push(Line::from(spans));
    }
}

fn ai_quota_tail(row: &AiQuotaRow) -> String {
    [row.suffix.as_deref(), row.reset.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ")
}

fn ai_status_row(label: &str, message: impl Into<String>, color: Color) -> Line<'static> {
    Line::from(vec![
        dim(fixed(label, CODEX_GUTTER_WIDTH)),
        span(message.into(), color, true),
    ])
}

fn section(lines: &mut Vec<Line<'static>>, title: &str, meta: &str, width: usize) {
    let heading = title.to_uppercase();
    let mut spans = vec![span(heading.clone(), Color::Cyan, true)];
    let mut used = heading.len();
    if !meta.is_empty() {
        spans.push(dim(" "));
        spans.push(dim(meta.to_string()));
        used += meta.len() + 1;
    }
    spans.push(dim("  "));
    spans.push(dim("-".repeat(width.saturating_sub(used + 2))));
    lines.push(Line::from(spans));
}

fn metric_row(
    label: &str,
    percent: Option<f64>,
    suffix: &str,
    usage: bool,
    width: usize,
) -> Line<'static> {
    if let Some(percent) = percent {
        let color = if usage {
            color_for_usage(percent)
        } else {
            color_for_remaining(percent)
        };
        let value = format!("{:>3}% {suffix}", percent.round() as i64);
        let mut row = vec![dim(fixed(label, 8))];
        row.extend(bar_spans(
            percent,
            metric_bar_width(width, 8, value.chars().count()),
            color,
        ));
        row.extend([Span::raw("  "), span(value, color, true)]);
        Line::from(row)
    } else {
        Line::from(vec![dim(fixed(label, 8)), dim("sampling")])
    }
}

fn metric_bar_width(width: usize, label_width: usize, value_width: usize) -> usize {
    width.saturating_sub(label_width + 2 + value_width)
}

fn bar_spans(percent: f64, width: usize, color: Color) -> Vec<Span<'static>> {
    let filled = ((percent.clamp(0.0, 100.0) / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    vec![
        Span::styled(BAR_FILLED.repeat(filled), Style::default().fg(color)),
        dim(BAR_EMPTY.repeat(empty)),
    ]
}

fn window_label(window: &Value) -> String {
    let minutes = window.get("windowDurationMins").and_then(Value::as_i64);
    match minutes {
        Some(300) => "5h".into(),
        Some(10080) => "Weekly".into(),
        Some(value) if value % 60 == 0 => format!("{}h", value / 60),
        Some(value) => format!("{value}m"),
        None => "Limit".into(),
    }
}

fn codex_compact_reset_label(window: &Value) -> String {
    let Some(epoch) = window.get("resetsAt").and_then(Value::as_i64) else {
        return "unknown".into();
    };
    let reset = Local
        .timestamp_opt(epoch, 0)
        .single()
        .unwrap_or_else(Local::now);
    if reset.date_naive() == Local::now().date_naive() {
        reset.format("%-I:%M%P").to_string()
    } else {
        reset.format("%-d %b").to_string()
    }
}

fn reset_label(window: &Value) -> String {
    let Some(epoch) = window.get("resetsAt").and_then(Value::as_i64) else {
        return "reset unknown".into();
    };
    let reset = Local
        .timestamp_opt(epoch, 0)
        .single()
        .unwrap_or_else(Local::now);
    if reset.date_naive() == Local::now().date_naive() {
        format!(
            "resets {}",
            reset.format("%I:%M %p").to_string().to_lowercase()
        )
    } else {
        format!(
            "resets {} {}",
            reset.format("%I:%M %p").to_string().to_lowercase(),
            reset.format("%-d %b")
        )
    }
}

fn color_for_remaining(percent: f64) -> Color {
    if percent <= 15.0 {
        Color::Red
    } else if percent <= 35.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

fn color_for_usage(percent: f64) -> Color {
    color_for_remaining(100.0 - percent)
}

fn rate_label(value: Option<f64>) -> String {
    let Some(mut value) = value else {
        return "sampling".into();
    };
    let units = ["B/s", "KB/s", "MB/s", "GB/s"];
    let mut index = 0;
    while value >= 1024.0 && index < units.len() - 1 {
        value /= 1024.0;
        index += 1;
    }
    let decimals = if value >= 10.0 || index == 0 { 0 } else { 1 };
    format!("{value:.decimals$} {}", units[index])
}

fn fixed(value: &str, width: usize) -> String {
    let clipped = value.chars().take(width).collect::<String>();
    format!("{clipped:<width$}")
}

fn span<T: Into<String>>(value: T, color: Color, bold: bool) -> Span<'static> {
    let mut style = Style::default().fg(color);
    if bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    Span::styled(value.into(), style)
}

fn dim<T: Into<String>>(value: T) -> Span<'static> {
    Span::styled(
        value.into(),
        Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
    )
}

pub(crate) fn print_once(state: &Arc<Mutex<AppState>>) {
    let started_at = Local::now();
    let deadline = Instant::now() + Duration::from_secs(25);
    while Instant::now() < deadline {
        let ready = {
            let state = state.lock().unwrap();
            provider_ready_for_once(&state.amp, &started_at)
                && provider_ready_for_once(&state.codex, &started_at)
        };
        if ready {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    let state = state.lock().unwrap().clone();
    println!("Stats");
    let gpu = state
        .system
        .gpu_percent
        .map(|value| format!("{value:.0}%"))
        .unwrap_or_else(|| "sampling".into());
    println!(
        "CPU {:.0}% RAM {:.0}% GPU {gpu}",
        state.system.cpu_percent.unwrap_or_default(),
        state.system.ram_percent
    );
    println!(
        "Network down {} up {}",
        rate_label(state.system.net_down_rate),
        rate_label(state.system.net_up_rate)
    );
    println!("Storage {:.0}% free", state.system.storage_percent_free);
    if let Some(error) = &state.amp.error {
        println!("Amp error: {error}");
    } else if let Some(amp) = &state.amp.result
        && let Some(percent) = amp.other_percent_remaining
    {
        let reset = amp
            .reset
            .as_deref()
            .map(amp_compact_reset_label)
            .map(|reset| format!(" {reset}"))
            .unwrap_or_default();
        println!(
            "{} {:.0}% left{reset}",
            amp.plan.as_deref().unwrap_or("Megawatt"),
            percent
        );
    }
    if let Some(error) = &state.codex.error {
        println!("Codex error: {error}");
    } else if let Some(result) = &state.codex.result {
        for snapshot in ordered_buckets(result) {
            let plan = snapshot
                .get("planType")
                .and_then(Value::as_str)
                .map(|plan| if plan == "prolite" { "Pro" } else { plan })
                .unwrap_or("");
            println!("Codex {plan}");
            if let Some(window) = codex_weekly_window(snapshot) {
                println!(
                    "{} {:.0}% left {}",
                    window_label(window),
                    left_percent(window),
                    reset_label(window)
                );
            }
        }
    }
}

fn provider_ready_for_once<T>(provider: &ProviderState<T>, started_at: &DateTime<Local>) -> bool {
    if provider.error.is_some() {
        return true;
    }
    provider
        .updated_at
        .as_ref()
        .is_some_and(|updated_at| updated_at.signed_duration_since(*started_at).num_seconds() >= -1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn date(value: &str) -> NaiveDate {
        NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap()
    }

    fn activity(buckets: &[(&str, u64)]) -> CodexActivityUsage {
        CodexActivityUsage {
            daily_usage_buckets: Some(
                buckets
                    .iter()
                    .map(|(start_date, tokens)| crate::model::CodexDailyUsageBucket {
                        start_date: (*start_date).into(),
                        tokens: *tokens,
                    })
                    .collect(),
            ),
            summary: None,
        }
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().fold(String::new(), |mut text, span| {
            text.push_str(span.content.as_ref());
            text
        })
    }

    #[test]
    fn renders_clocks_as_equal_high_contrast_columns() {
        let mut lines = Vec::new();

        render_clocks(&mut lines, 58);

        assert_eq!(lines.len(), 3);
        assert_eq!(line_text(&lines[0]).chars().count(), 58);
        assert_eq!(line_text(&lines[1]).chars().count(), 58);
        assert!(line_text(&lines[0]).contains("MUMBAI"));
        assert!(line_text(&lines[0]).contains("SEATTLE"));
        assert!(
            lines[0]
                .spans
                .iter()
                .filter(|span| span.style.fg == Some(Color::Cyan))
                .count()
                == 4
        );
        assert!(
            lines[1]
                .spans
                .iter()
                .filter(|span| span.style.add_modifier.contains(Modifier::BOLD))
                .count()
                == 4
        );
    }

    #[test]
    fn renders_only_one_codex_weekly_row() {
        let codex = ProviderState {
            result: Some(json!({
                "rateLimitsByLimitId": {
                    "codex": {
                        "primary": {
                            "resetsAt": 1784696828_i64,
                            "usedPercent": 25,
                            "windowDurationMins": 300
                        },
                        "secondary": {
                            "resetsAt": 1784696828_i64,
                            "usedPercent": 4,
                            "windowDurationMins": 10080
                        }
                    }
                }
            })),
            ..ProviderState::default()
        };
        let mut rows = Vec::new();
        let mut statuses = Vec::new();

        collect_codex_ai_rows(&mut rows, &mut statuses, &codex);

        assert!(statuses.is_empty());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label, "Codex Pro");
        assert_eq!(rows[0].percent_left, 96.0);
    }

    #[test]
    fn compacts_ai_reset_labels() {
        let today = date("2026-08-22");
        for (reset, expected) in [
            ("resets upon renewal in 1 day", "23 Aug"),
            ("resets upon renewal in 29 days", "20 Sep"),
            ("resets upon renewal in 1 month", "22 Sep"),
            ("resets upon renewal in 2 months", "22 Oct"),
            ("resets upon renewal in 2 weeks", "5 Sep"),
            ("resets upon renewal in 3 hours", "in 3h"),
            ("resets upon renewal on 27 Aug", "on 27 Aug"),
        ] {
            assert_eq!(amp_compact_reset_label_at(reset, today), expected);
        }

        let future = Local::now() + chrono::Duration::days(7);
        let window = json!({ "resetsAt": future.timestamp() });
        assert_eq!(
            codex_compact_reset_label(&window),
            future.format("%-d %b").to_string()
        );
    }

    #[test]
    fn keeps_the_weekly_row_within_narrow_widths() {
        let row = AiQuotaRow {
            label: "Codex Pro".into(),
            percent_left: 96.0,
            reset: Some("09:48am 8 Aug".into()),
            suffix: None,
        };

        for width in [9, 10, 11, 12, 18, 19] {
            let mut lines = Vec::new();
            render_ai_quota_rows(&mut lines, vec![row.clone()], width);
            assert_eq!(lines.len(), 1);
            assert!(line_text(&lines[0]).chars().count() <= width);
            if width >= 18 {
                assert!(line_text(&lines[0]).contains("96% left"));
            } else if width >= 12 {
                assert!(line_text(&lines[0]).contains("96%"));
            } else {
                assert!(!line_text(&lines[0]).contains("96"));
            }
        }
    }

    #[test]
    fn aligns_ai_quota_tracks_values_and_reset_columns() {
        let rows = vec![
            AiQuotaRow {
                label: "Megawatt".into(),
                percent_left: 100.0,
                reset: Some("22 Sep".into()),
                suffix: None,
            },
            AiQuotaRow {
                label: "Codex Pro".into(),
                percent_left: 95.0,
                reset: Some("27 Aug".into()),
                suffix: None,
            },
        ];
        let mut lines = Vec::new();

        render_ai_quota_rows(&mut lines, rows, 58);

        assert_eq!(lines.len(), 2);
        assert!(
            lines
                .iter()
                .all(|line| line_text(line).chars().count() == 58)
        );
        assert_eq!(
            lines[0].spans[2].content.chars().count() + lines[0].spans[3].content.chars().count(),
            lines[1].spans[2].content.chars().count() + lines[1].spans[3].content.chars().count()
        );
        assert_eq!(
            lines[0].spans[2].content.chars().count() + lines[0].spans[3].content.chars().count(),
            29
        );
        let span_start = |line: &Line<'_>, index: usize| {
            line.spans[..index]
                .iter()
                .map(|span| span.content.chars().count())
                .sum::<usize>()
        };
        assert_eq!(span_start(&lines[0], 5), span_start(&lines[1], 5));
        assert_eq!(span_start(&lines[0], 7), span_start(&lines[1], 7));
    }

    #[test]
    fn separates_activity_from_the_codex_quota() {
        let amp = ProviderState {
            result: Some(AmpUsage {
                plan: Some("Megawatt".into()),
                other_percent_remaining: Some(82.0),
                reset: Some("resets upon renewal in 1 month".into()),
            }),
            ..ProviderState::default()
        };
        let codex = ProviderState {
            result: Some(json!({
                "rateLimitsByLimitId": {
                    "codex": {
                        "primary": {
                            "resetsAt": 1784696828_i64,
                            "usedPercent": 4,
                            "windowDurationMins": 10080
                        }
                    }
                }
            })),
            ..ProviderState::default()
        };
        let activity = ProviderState {
            result: Some(activity(&[("2026-07-12", 1), ("2026-08-02", 2)])),
            ..ProviderState::default()
        };
        let mut lines = Vec::new();

        render_ai_at(&mut lines, &amp, &codex, &activity, 40, date("2026-08-02"));
        let text = lines.iter().map(line_text).collect::<Vec<_>>();
        let megawatt = text
            .iter()
            .position(|line| line.contains("Megawatt"))
            .unwrap();
        let quota = text
            .iter()
            .position(|line| line.contains("Codex Pro"))
            .unwrap();

        assert!(text[0].starts_with("AI"));
        assert!(megawatt < quota);
        assert!(text[quota + 1].is_empty());
        assert!(text[quota + 2].contains("Jul"));
        assert!(text[quota + 3].starts_with("Sun"));
    }
}
