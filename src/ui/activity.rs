use std::collections::BTreeMap;

use chrono::{Datelike, Days, NaiveDate};
use ratatui::style::Color;
use ratatui::text::{Line, Span};

use super::{CODEX_GUTTER_WIDTH, ai_status_row, dim, equal_column_widths, fixed, span};
use crate::model::{
    AmpActivityUsage, AmpTokenCategory, CodexActivitySummary, CodexActivityUsage,
    CodexDailyUsageBucket, DailyTokenUsage, ProviderState,
};

const ACTIVITY_MIN_GUTTER_WIDTH: usize = 5;
const AMP_DETAIL_DAYS: u64 = 30;

pub(super) fn amp_activity_history_days(panel_width: usize, utc_today: NaiveDate) -> usize {
    activity_week_capacity(panel_width)
        .saturating_sub(1)
        .saturating_mul(7)
        .saturating_add(utc_today.weekday().num_days_from_sunday() as usize + 1)
}

#[derive(Debug, Clone)]
struct ActivityCalendar {
    start_week: NaiveDate,
    weeks: usize,
    gutter_width: usize,
    utc_today: NaiveDate,
    latest_date: NaiveDate,
    tokens_by_date: BTreeMap<NaiveDate, u64>,
    quartiles: [u64; 3],
    summary: Option<CodexActivitySummary>,
}

pub(super) fn render_codex_activity(
    lines: &mut Vec<Line<'static>>,
    activity: &ProviderState<CodexActivityUsage>,
    width: usize,
    utc_today: NaiveDate,
) {
    if activity_week_capacity(width) == 0 {
        return;
    }
    if let Some(error) = &activity.error {
        lines.push(ai_status_row(
            "Activity",
            format!("Error: {error}"),
            Color::Red,
        ));
        return;
    }
    let Some(result) = &activity.result else {
        lines.push(ai_status_row(
            "Activity",
            "Loading token activity...",
            Color::Yellow,
        ));
        return;
    };
    let Some(calendar) = activity_calendar(result, width, utc_today) else {
        lines.push(Line::from(vec![
            dim(fixed("Activity", CODEX_GUTTER_WIDTH)),
            dim("unavailable"),
        ]));
        return;
    };
    render_activity_calendar(lines, &calendar, width, true);
}

pub(super) fn render_amp_activity(
    lines: &mut Vec<Line<'static>>,
    activity: &ProviderState<AmpActivityUsage>,
    width: usize,
    utc_today: NaiveDate,
) {
    if activity_week_capacity(width) == 0 {
        return;
    }
    if let Some(error) = &activity.error {
        lines.push(ai_status_row(
            "Activity",
            format!("Cached data; refresh failed: {error}"),
            Color::Yellow,
        ));
    }
    let Some(result) = &activity.result else {
        lines.push(ai_status_row(
            "Activity",
            "Loading Amp history...",
            Color::Yellow,
        ));
        return;
    };
    let codex_shaped = CodexActivityUsage {
        daily_usage_buckets: Some(
            result
                .daily_usage_buckets
                .iter()
                .map(|bucket| CodexDailyUsageBucket {
                    start_date: bucket.date.clone(),
                    tokens: bucket.tokens,
                })
                .collect(),
        ),
        summary: None,
    };
    let Some(calendar) = activity_calendar(&codex_shaped, width, utc_today) else {
        return;
    };
    render_activity_calendar(lines, &calendar, width, false);
    lines.push(Line::default());
    let detail_start = calendar
        .utc_today
        .checked_sub_days(Days::new(AMP_DETAIL_DAYS - 1))
        .unwrap_or(calendar.utc_today);
    let recent_activity = AmpActivityUsage {
        daily_usage_buckets: result
            .daily_usage_buckets
            .iter()
            .filter(|bucket| {
                NaiveDate::parse_from_str(&bucket.date, "%Y-%m-%d")
                    .is_ok_and(|date| date >= detail_start && date <= calendar.utc_today)
            })
            .cloned()
            .collect(),
    };
    lines.extend(amp_detail_rows(&recent_activity, width, calendar.utc_today));
}

pub(super) fn amp_activity_sync_message(
    activity: &ProviderState<AmpActivityUsage>,
    width: usize,
    utc_today: NaiveDate,
) -> Option<String> {
    let result = activity.result.as_ref()?;
    if activity.error.is_some() {
        return None;
    }
    let codex_shaped = CodexActivityUsage {
        daily_usage_buckets: Some(
            result
                .daily_usage_buckets
                .iter()
                .map(|bucket| CodexDailyUsageBucket {
                    start_date: bucket.date.clone(),
                    tokens: bucket.tokens,
                })
                .collect(),
        ),
        summary: None,
    };
    let calendar = activity_calendar(&codex_shaped, width, utc_today)?;
    let required_days = utc_today
        .signed_duration_since(calendar.start_week)
        .num_days()
        .max(0) as usize
        + 1;
    let cached_days = result
        .daily_usage_buckets
        .iter()
        .filter(|bucket| {
            NaiveDate::parse_from_str(&bucket.date, "%Y-%m-%d")
                .is_ok_and(|date| date >= calendar.start_week && date <= utc_today)
        })
        .count();
    if cached_days >= required_days {
        return None;
    }
    let remaining_days = required_days - cached_days;
    let retry_seconds = activity
        .retry_after
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let eta_seconds = (remaining_days as u64)
        .saturating_mul(3_600)
        .div_ceil(24)
        .saturating_add(retry_seconds);
    let resume = activity
        .retry_after
        .map(|duration| format!(" · resumes in ~{}", sync_eta(duration.as_secs())))
        .unwrap_or_default();
    Some(format!(
        "Amp Code activity history sync completes in ~{}{resume}",
        sync_eta(eta_seconds)
    ))
}

fn render_activity_calendar(
    lines: &mut Vec<Line<'static>>,
    calendar: &ActivityCalendar,
    width: usize,
    show_overview: bool,
) {
    lines.push(activity_month_labels(&calendar));
    for day_offset in 0..7 {
        let mut spans = vec![dim(fixed(
            ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][day_offset],
            calendar.gutter_width,
        ))];
        for week in 0..calendar.weeks {
            if week > 0 {
                spans.push(Span::raw(" "));
            }
            let date = calendar
                .start_week
                .checked_add_days(Days::new((week * 7 + day_offset) as u64))
                .unwrap_or(calendar.utc_today);
            if date > calendar.utc_today {
                spans.push(Span::raw(" "));
                continue;
            }
            let tokens = calendar.tokens_by_date.get(&date).copied().unwrap_or(0);
            if tokens == 0 {
                spans.push(dim("·"));
            } else {
                spans.push(span(
                    "▪",
                    activity_green(activity_intensity(tokens, calendar.quartiles)),
                    true,
                ));
            }
        }
        lines.push(Line::from(spans));
    }
    lines.push(Line::default());
    if show_overview {
        lines.extend(activity_overview_rows(&calendar, width));
        lines.push(Line::default());
    }
    lines.extend(activity_daily_rows(&calendar, width));
}

#[derive(Clone, Copy)]
enum AmpCategoryKind {
    Models,
    Sources,
}

impl AmpCategoryKind {
    fn heading(self) -> &'static str {
        match self {
            Self::Models => "Models",
            Self::Sources => "Sources",
        }
    }

    fn categories(self, bucket: &crate::model::AmpDailyUsageBucket) -> &[AmpTokenCategory] {
        match self {
            Self::Models => &bucket.models,
            Self::Sources => &bucket.sources,
        }
    }
}

fn amp_detail_rows(
    activity: &AmpActivityUsage,
    width: usize,
    utc_today: NaiveDate,
) -> Vec<Line<'static>> {
    let periods = [1_u64, 7, AMP_DETAIL_DAYS];
    let mut rows = vec![amp_table_row(
        "Metric",
        ["1D".into(), "7D".into(), "30D".into()],
        width,
    )];
    rows.push(amp_table_row(
        "Covered",
        periods.map(|days| {
            format!(
                "${:.2}",
                period_buckets(activity, utc_today, days)
                    .map(|bucket| bucket.covered_cost)
                    .sum::<f64>()
            )
        }),
        width,
    ));
    rows.push(amp_table_row(
        "Paid",
        periods.map(|days| {
            format!(
                "${:.2}",
                period_buckets(activity, utc_today, days)
                    .map(|bucket| bucket.paid_cost)
                    .sum::<f64>()
            )
        }),
        width,
    ));
    rows.push(amp_table_row(
        "Orb time",
        periods.map(|days| {
            compact_duration(
                period_buckets(activity, utc_today, days)
                    .map(|bucket| bucket.orb_runtime_millis)
                    .sum(),
            )
        }),
        width,
    ));
    rows.extend(category_table_rows(
        activity,
        AmpCategoryKind::Models,
        width,
        utc_today,
    ));
    rows.extend(category_table_rows(
        activity,
        AmpCategoryKind::Sources,
        width,
        utc_today,
    ));
    rows
}

fn period_buckets(
    activity: &AmpActivityUsage,
    utc_today: NaiveDate,
    days: u64,
) -> impl Iterator<Item = &crate::model::AmpDailyUsageBucket> {
    let start = utc_today
        .checked_sub_days(Days::new(days - 1))
        .unwrap_or(utc_today);
    activity.daily_usage_buckets.iter().filter(move |bucket| {
        NaiveDate::parse_from_str(&bucket.date, "%Y-%m-%d")
            .is_ok_and(|date| date >= start && date <= utc_today)
    })
}

fn amp_table_row(label: &str, values: [String; 3], width: usize) -> Line<'static> {
    let minimum_label_width = CODEX_GUTTER_WIDTH.min(width);
    let label_width = width.saturating_sub(21).clamp(minimum_label_width, 24);
    let column_widths = equal_column_widths(width.saturating_sub(label_width), values.len());
    let mut spans = vec![dim(fixed(label, label_width))];
    spans.extend(
        values
            .into_iter()
            .zip(column_widths)
            .map(|(value, width)| dim(right_fixed(&value, width))),
    );
    Line::from(spans)
}

fn right_fixed(value: &str, width: usize) -> String {
    let clipped = value.chars().take(width).collect::<String>();
    format!("{clipped:>width$}")
}

fn aggregate_categories<'a>(
    categories: impl Iterator<Item = &'a AmpTokenCategory>,
) -> Vec<(String, u64)> {
    let mut totals = BTreeMap::new();
    for category in categories {
        let total = totals.entry(category.label.clone()).or_insert(0_u64);
        *total = total.saturating_add(category.tokens);
    }
    let mut totals = totals
        .into_iter()
        .filter(|(_, tokens)| *tokens > 0)
        .collect::<Vec<_>>();
    totals.sort_by_key(|(_, tokens)| std::cmp::Reverse(*tokens));
    totals
}

fn category_table_rows(
    activity: &AmpActivityUsage,
    kind: AmpCategoryKind,
    width: usize,
    utc_today: NaiveDate,
) -> Vec<Line<'static>> {
    let categories = aggregate_categories(
        activity
            .daily_usage_buckets
            .iter()
            .flat_map(|bucket| kind.categories(bucket)),
    );
    let total = categories.iter().map(|(_, tokens)| *tokens).sum::<u64>();
    if total == 0 {
        return Vec::new();
    }
    let mut rows = vec![amp_table_row(
        kind.heading(),
        std::array::from_fn(|_| String::new()),
        width,
    )];
    rows.extend(
        categories
            .iter()
            .filter(|(_, tokens)| *tokens as f64 / total as f64 * 100.0 >= 1.0)
            .map(|(label, _)| {
                amp_table_row(
                    label,
                    [1_u64, 7, AMP_DETAIL_DAYS].map(|days| {
                        compact_token_count(
                            period_buckets(activity, utc_today, days)
                                .flat_map(|bucket| kind.categories(bucket))
                                .filter(|category| category.label == *label)
                                .map(|category| category.tokens)
                                .sum(),
                        )
                    }),
                    width,
                )
            }),
    );
    rows
}

fn compact_duration(millis: u64) -> String {
    let minutes = millis / 60_000;
    let hours = minutes / 60;
    if hours > 0 {
        format!("{hours}h{}m", minutes % 60)
    } else {
        format!("{minutes}m")
    }
}

fn sync_eta(seconds: u64) -> String {
    let minutes = seconds.div_ceil(60);
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let hours = minutes / 60;
    let remaining_minutes = minutes % 60;
    if remaining_minutes == 0 {
        format!("{hours}h")
    } else {
        format!("{hours}h {remaining_minutes}m")
    }
}

fn activity_overview_rows(calendar: &ActivityCalendar, width: usize) -> Vec<Line<'static>> {
    let mut stats = vec![
        (
            "7D",
            compact_token_count(activity_period_tokens(calendar, 7)),
        ),
        (
            "30D",
            compact_token_count(activity_period_tokens(calendar, 30)),
        ),
    ];
    if let Some(summary) = calendar.summary {
        stats.extend(
            [
                summary
                    .lifetime_tokens
                    .map(|tokens| ("Total", compact_token_count(tokens))),
                summary
                    .peak_daily_tokens
                    .map(|tokens| ("Peak", compact_token_count(tokens))),
                summary
                    .current_streak_days
                    .map(|days| ("Streak", format!("{days}d"))),
                summary
                    .longest_streak_days
                    .map(|days| ("Best", format!("{days}d"))),
            ]
            .into_iter()
            .flatten(),
        );
    }
    let gap = usize::from(width >= stats.len() * 2 - 1);
    let column_widths =
        equal_column_widths(width.saturating_sub(gap * (stats.len() - 1)), stats.len());
    let mut headings = Vec::with_capacity(stats.len());
    let mut values = Vec::with_capacity(stats.len());
    for (index, ((heading, value), column_width)) in
        stats.into_iter().zip(column_widths).enumerate()
    {
        if index > 0 {
            headings.push(Span::raw(" ".repeat(gap)));
            values.push(Span::raw(" ".repeat(gap)));
        }
        headings.push(dim(fixed(heading, column_width)));
        values.push(span(fixed(&value, column_width), Color::Green, true));
    }
    vec![Line::from(headings), Line::from(values)]
}

fn activity_period_tokens(calendar: &ActivityCalendar, days: u64) -> u64 {
    let start = calendar
        .latest_date
        .checked_sub_days(Days::new(days.saturating_sub(1)))
        .unwrap_or(calendar.latest_date);
    calendar
        .tokens_by_date
        .range(start..=calendar.latest_date)
        .fold(0, |total, (_, tokens)| total.saturating_add(*tokens))
}

fn activity_daily_rows(calendar: &ActivityCalendar, width: usize) -> Vec<Line<'static>> {
    if width < 7 {
        return Vec::new();
    }
    let gap = usize::from(width >= 13);
    let cell_widths = equal_column_widths(width.saturating_sub(gap * 6), 7);
    let first_date = calendar
        .latest_date
        .checked_sub_days(Days::new(6))
        .unwrap_or(calendar.latest_date);
    let dates = (0..7)
        .map(|offset| {
            first_date
                .checked_add_days(Days::new(offset))
                .unwrap_or(first_date)
        })
        .collect::<Vec<_>>();
    let mut date_spans = Vec::with_capacity(7);
    let mut value_spans = Vec::with_capacity(7);
    for (index, (date, cell_width)) in dates.into_iter().zip(cell_widths).enumerate() {
        if index > 0 {
            date_spans.push(Span::raw(" ".repeat(gap)));
            value_spans.push(Span::raw(" ".repeat(gap)));
        }
        let date_label = date.format("%-d/%-m").to_string();
        date_spans.push(dim(fixed(&date_label, cell_width)));
        let tokens = calendar.tokens_by_date.get(&date).copied().unwrap_or(0);
        if tokens == 0 {
            value_spans.push(dim(fixed("·", cell_width)));
        } else {
            let color = activity_green(activity_intensity(tokens, calendar.quartiles));
            value_spans.push(span(
                fixed(&compact_token_count(tokens), cell_width),
                color,
                true,
            ));
        }
    }
    vec![Line::from(date_spans), Line::from(value_spans)]
}

fn compact_token_count(tokens: u64) -> String {
    const UNITS: &[&str] = &["", "K", "M", "B", "T"];
    let mut value = tokens as f64;
    let mut unit = 0;
    while value >= 1_000.0 && unit < UNITS.len() - 1 {
        value /= 1_000.0;
        unit += 1;
    }

    loop {
        let decimals = if unit == 0 || value >= 100.0 || value.fract() < 0.05 {
            0
        } else {
            1
        };
        let scale = if decimals == 0 { 1.0 } else { 10.0 };
        let rounded = (value * scale).round() / scale;
        if rounded >= 1_000.0 && unit < UNITS.len() - 1 {
            value /= 1_000.0;
            unit += 1;
            continue;
        }
        let decimals = if unit == 0 || rounded >= 100.0 || rounded.fract() < 0.05 {
            0
        } else {
            1
        };
        return format!("{value:.decimals$}{}", UNITS[unit]);
    }
}

fn activity_calendar(
    usage: &CodexActivityUsage,
    width: usize,
    utc_today: NaiveDate,
) -> Option<ActivityCalendar> {
    let tokens_by_date = daily_token_usage(usage)?
        .into_iter()
        .map(|bucket| (bucket.date, bucket.tokens))
        .collect::<BTreeMap<_, _>>();
    let latest_date = *tokens_by_date.keys().next_back()?;
    let current_week = sunday_of_week(utc_today);
    let gutter_width = activity_gutter_width(width);
    let weeks = activity_week_capacity(width);
    if weeks == 0 {
        return None;
    }
    let start_week = current_week
        .checked_sub_days(Days::new(((weeks - 1) * 7) as u64))
        .unwrap_or(current_week);
    let visible_nonzero = tokens_by_date
        .range(start_week..=utc_today)
        .map(|(_, tokens)| *tokens)
        .filter(|tokens| *tokens > 0)
        .collect::<Vec<_>>();

    Some(ActivityCalendar {
        start_week,
        weeks,
        gutter_width,
        utc_today,
        latest_date,
        tokens_by_date,
        quartiles: activity_quartiles(&visible_nonzero),
        summary: usage.summary,
    })
}

fn daily_token_usage(usage: &CodexActivityUsage) -> Option<Vec<DailyTokenUsage>> {
    Some(
        usage
            .daily_usage_buckets
            .as_ref()?
            .iter()
            .filter_map(|bucket| {
                NaiveDate::parse_from_str(&bucket.start_date, "%Y-%m-%d")
                    .ok()
                    .map(|date| DailyTokenUsage {
                        date,
                        tokens: bucket.tokens,
                    })
            })
            .collect(),
    )
}

fn activity_week_capacity(width: usize) -> usize {
    width
        .saturating_sub(activity_gutter_width(width))
        .div_ceil(2)
}

fn activity_gutter_width(width: usize) -> usize {
    ACTIVITY_MIN_GUTTER_WIDTH + usize::from(width % 2 == ACTIVITY_MIN_GUTTER_WIDTH % 2)
}

fn sunday_of_week(date: NaiveDate) -> NaiveDate {
    date.checked_sub_days(Days::new(date.weekday().num_days_from_sunday() as u64))
        .unwrap_or(date)
}

fn activity_quartiles(values: &[u64]) -> [u64; 3] {
    if values.is_empty() {
        return [0; 3];
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let last = sorted.len() - 1;
    [sorted[last / 4], sorted[last / 2], sorted[last * 3 / 4]]
}

fn activity_intensity(tokens: u64, quartiles: [u64; 3]) -> usize {
    if tokens <= quartiles[0] {
        1
    } else if tokens <= quartiles[1] {
        2
    } else if tokens <= quartiles[2] {
        3
    } else {
        4
    }
}

fn activity_green(level: usize) -> Color {
    match level {
        1 => Color::Rgb(14, 68, 41),
        2 => Color::Rgb(0, 109, 50),
        3 => Color::Rgb(38, 166, 65),
        _ => Color::Rgb(57, 211, 83),
    }
}

fn activity_month_labels(calendar: &ActivityCalendar) -> Line<'static> {
    let grid_width = calendar.weeks * 2 - 1;
    let mut text = vec![' '; grid_width];
    let mut last_label_end = None;
    for week in 0..calendar.weeks {
        let week_start = calendar
            .start_week
            .checked_add_days(Days::new((week * 7) as u64))
            .unwrap_or(calendar.start_week);
        let visible_start = week_start;
        let week_end = week_start
            .checked_add_days(Days::new(6))
            .unwrap_or(week_start)
            .min(calendar.utc_today);
        if visible_start > week_end {
            continue;
        }
        let label_date = if week == 0 {
            Some(visible_start)
        } else {
            (0..7)
                .filter_map(|offset| week_start.checked_add_days(Days::new(offset)))
                .find(|date| *date >= visible_start && *date <= week_end && date.day() == 1)
        };
        let Some(label_date) = label_date else {
            continue;
        };
        let x = week * 2;
        let label = label_date.format("%b").to_string();
        if x + label.chars().count() > text.len() || last_label_end.is_some_and(|end| x <= end) {
            continue;
        }
        for (offset, ch) in label.chars().enumerate() {
            if x + offset < text.len() {
                text[x + offset] = ch;
            }
        }
        last_label_end = Some(x + label.len());
    }
    Line::from(vec![
        dim(fixed("", calendar.gutter_width)),
        dim(text.into_iter().collect::<String>()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Modifier;
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

    fn activity_with_summary(
        buckets: &[(&str, u64)],
        summary: CodexActivitySummary,
    ) -> CodexActivityUsage {
        CodexActivityUsage {
            summary: Some(summary),
            ..activity(buckets)
        }
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().fold(String::new(), |mut text, span| {
            text.push_str(span.content.as_ref());
            text
        })
    }

    #[test]
    fn anchors_calendar_to_the_utc_week_and_uses_every_column_that_fits() {
        let usage = activity(&[("2026-08-01", 20)]);

        let calendar = activity_calendar(&usage, 30, date("2026-08-05")).unwrap();

        assert_eq!(calendar.weeks, 13);
        assert_eq!(calendar.start_week, date("2026-05-10"));
        assert_eq!(sunday_of_week(calendar.utc_today), date("2026-08-02"));
    }

    #[test]
    fn places_activity_in_calendar_rows_and_marks_missing_past_dates() {
        let state = ProviderState {
            result: Some(activity(&[("2026-07-26", 10), ("2026-08-01", 20)])),
            ..ProviderState::default()
        };
        let mut lines = Vec::new();

        render_codex_activity(&mut lines, &state, 30, date("2026-08-01"));

        assert_eq!(lines.len(), 14);
        assert_eq!(line_text(&lines[1]).chars().count(), 30);
        assert!(line_text(&lines[1]).starts_with("Sun  ·"));
        assert!(line_text(&lines[1]).ends_with('▪'));
        assert!(line_text(&lines[7]).ends_with('▪'));
        assert!(
            lines[2..7]
                .iter()
                .all(|line| line_text(line).ends_with('·'))
        );
        assert!(line_text(&lines[8]).is_empty());
        assert!(line_text(&lines[9]).contains("7D"));
        assert!(line_text(&lines[9]).contains("30D"));
        assert!(line_text(&lines[11]).is_empty());
        assert!(line_text(&lines[12]).contains("26"));
        assert!(line_text(&lines[12]).contains('1'));
        assert!(line_text(&lines[13]).contains("10"));
        assert!(line_text(&lines[13]).contains("20"));
        assert!(lines[2].spans.iter().any(|span| {
            span.content.as_ref() == "·" && span.style.add_modifier.contains(Modifier::DIM)
        }));
    }

    #[test]
    fn leaves_current_week_future_days_blank() {
        let state = ProviderState {
            result: Some(activity(&[("2026-07-12", 1), ("2026-08-05", 5)])),
            ..ProviderState::default()
        };
        let mut lines = Vec::new();

        render_codex_activity(&mut lines, &state, 30, date("2026-08-05"));

        assert!(line_text(&lines[4]).ends_with('▪'));
        assert!(line_text(&lines[5]).ends_with(' '));
        assert!(line_text(&lines[6]).ends_with(' '));
        assert!(line_text(&lines[7]).ends_with(' '));
    }

    #[test]
    fn labels_months_above_their_first_visible_weeks() {
        let usage = activity(&[("2026-07-01", 1), ("2026-08-02", 2)]);
        let calendar = activity_calendar(&usage, 40, date("2026-08-02")).unwrap();

        let labels = line_text(&activity_month_labels(&calendar));

        assert!(labels.contains("Jul"));
        assert!(labels.contains("Aug"));
        assert!(labels.find("Jul").unwrap() < labels.find("Aug").unwrap());

        let edge_usage = activity(&[("2026-08-01", 1)]);
        let edge_calendar = activity_calendar(&edge_usage, 40, date("2026-08-01")).unwrap();
        let edge_labels = line_text(&activity_month_labels(&edge_calendar));
        assert!(!edge_labels.trim_end().ends_with('A'));
    }

    #[test]
    fn uses_full_width_even_when_returned_history_is_shorter() {
        let usage = activity(&[("2026-06-01", 1), ("2026-08-02", 2)]);

        let calendar = activity_calendar(&usage, 16, date("2026-08-02")).unwrap();

        assert_eq!(calendar.weeks, 6);
        assert_eq!(calendar.start_week, date("2026-06-28"));
    }

    #[test]
    fn assigns_four_levels_from_visible_quartiles() {
        let quartiles = activity_quartiles(&[10, 20, 30, 40, 50, 60, 70, 80]);

        assert_eq!(quartiles, [20, 40, 60]);
        assert_eq!(activity_intensity(10, quartiles), 1);
        assert_eq!(activity_intensity(30, quartiles), 2);
        assert_eq!(activity_intensity(50, quartiles), 3);
        assert_eq!(activity_intensity(70, quartiles), 4);
    }

    #[test]
    fn selects_the_latest_bucket_and_summarizes_anchored_calendar_days() {
        let usage = activity(&[
            ("2026-08-01", 47_250_000),
            ("2026-07-02", 99_000_000),
            ("2026-07-03", 10_000_000),
            ("2026-07-26", 200_000_000),
            ("2026-08-01", 100_000_000),
            ("2026-07-31", 47_250_000),
        ]);
        let calendar = activity_calendar(&usage, 100, date("2026-08-02")).unwrap();

        assert_eq!(calendar.latest_date, date("2026-08-01"));
        assert_eq!(activity_period_tokens(&calendar, 1), 100_000_000);
        assert_eq!(activity_period_tokens(&calendar, 7), 347_250_000);
        assert_eq!(activity_period_tokens(&calendar, 30), 357_250_000);
        let summary = activity_overview_rows(&calendar, 51);
        assert_eq!(summary.len(), 2);
        assert_eq!(summary[0].spans[0].content.trim(), "7D");
        assert_eq!(summary[0].spans[2].content.trim(), "30D");
        assert_eq!(summary[1].spans[0].content.trim(), "347M");
        assert_eq!(summary[1].spans[2].content.trim(), "357M");
    }

    #[test]
    fn compacts_large_token_counts_and_truncates_summary_by_width() {
        assert_eq!(compact_token_count(999), "999");
        assert_eq!(compact_token_count(1_250), "1.2K");
        assert_eq!(compact_token_count(999_499), "999K");
        assert_eq!(compact_token_count(999_500), "1M");
        assert_eq!(compact_token_count(999_999_999), "1B");
        assert_eq!(compact_token_count(2_100_000_000), "2.1B");
    }

    #[test]
    fn includes_account_metrics_in_the_overview_columns() {
        let usage = activity_with_summary(
            &[("2026-08-01", 2)],
            CodexActivitySummary {
                lifetime_tokens: Some(12_300_000_000),
                peak_daily_tokens: Some(420_000_000),
                longest_running_turn_sec: Some(9_000),
                current_streak_days: Some(5),
                longest_streak_days: Some(23),
            },
        );
        let calendar = activity_calendar(&usage, 100, date("2026-08-02")).unwrap();

        let overview = activity_overview_rows(&calendar, 51);
        assert_eq!(overview.len(), 2);
        assert_eq!(overview[0].spans.len(), 11);
        assert_eq!(overview[0].spans[4].content.trim(), "Total");
        assert_eq!(overview[0].spans[6].content.trim(), "Peak");
        assert_eq!(overview[0].spans[8].content.trim(), "Streak");
        assert_eq!(overview[0].spans[10].content.trim(), "Best");
        assert_eq!(overview[1].spans[4].content.trim(), "12.3B");
        assert_eq!(overview[1].spans[6].content.trim(), "420M");
        assert_eq!(overview[1].spans[8].content.trim(), "5d");
        assert_eq!(overview[1].spans[10].content.trim(), "23d");
        assert!(overview[0].spans[0].content.starts_with("7D"));
        assert!(overview[1].spans[0].content.starts_with('2'));
        assert_eq!(
            calendar.summary.unwrap().longest_running_turn_sec,
            Some(9_000)
        );
    }

    #[test]
    fn reads_bucket_only_and_enriched_activity_caches() {
        let cached: CodexActivityUsage = serde_json::from_value(json!({
            "dailyUsageBuckets": [{"startDate": "2026-08-01", "tokens": 2}]
        }))
        .unwrap();
        assert!(cached.summary.is_none());

        let enriched: CodexActivityUsage = serde_json::from_value(json!({
            "dailyUsageBuckets": [],
            "summary": {
                "lifetimeTokens": 12,
                "peakDailyTokens": 8,
                "longestRunningTurnSec": 90,
                "currentStreakDays": 3,
                "longestStreakDays": 7
            }
        }))
        .unwrap();
        assert_eq!(
            enriched.summary,
            Some(CodexActivitySummary {
                lifetime_tokens: Some(12),
                peak_daily_tokens: Some(8),
                longest_running_turn_sec: Some(90),
                current_streak_days: Some(3),
                longest_streak_days: Some(7),
            })
        );
    }

    #[test]
    fn renders_daily_dates_and_token_values_without_bars() {
        let usage = activity(&[
            ("2026-07-26", 10),
            ("2026-07-27", 20),
            ("2026-07-28", 30),
            ("2026-07-29", 40),
            ("2026-07-30", 50),
            ("2026-07-31", 60),
            ("2026-08-01", 70),
        ]);
        let calendar = activity_calendar(&usage, 51, date("2026-08-02")).unwrap();
        let rows = activity_daily_rows(&calendar, 51);

        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| line_text(row).chars().count() == 51));
        assert_eq!(rows[0].spans.len(), 13);
        assert_eq!(rows[0].spans[0].content.trim(), "26/7");
        assert_eq!(rows[0].spans[12].content.trim(), "1/8");
        assert_eq!(rows[1].spans[0].content.trim(), "10");
        assert_eq!(rows[1].spans[12].content.trim(), "70");
        assert_eq!(rows[1].spans[0].style.fg, Some(activity_green(1)));
        assert_eq!(rows[1].spans[12].style.fg, Some(activity_green(4)));
        for index in (0..13).step_by(2) {
            assert!(!rows[0].spans[index].content.starts_with(' '));
            assert!(!rows[1].spans[index].content.starts_with(' '));
        }
    }

    #[test]
    fn treats_null_buckets_as_unavailable_and_skips_malformed_dates() {
        let null_usage: CodexActivityUsage =
            serde_json::from_value(json!({"dailyUsageBuckets": null})).unwrap();
        assert!(activity_calendar(&null_usage, 40, date("2026-08-02")).is_none());

        let usage = activity(&[("not-a-date", 99), ("2026-08-02", 2)]);
        let calendar = activity_calendar(&usage, 40, date("2026-08-02")).unwrap();
        assert_eq!(calendar.latest_date, date("2026-08-02"));
        assert_eq!(calendar.tokens_by_date.len(), 1);
    }

    #[test]
    fn renders_missing_daily_usage_as_dim_dots() {
        let empty = activity(&[("2026-08-02", 0)]);
        let empty_calendar = activity_calendar(&empty, 22, date("2026-08-02")).unwrap();
        let rows = activity_daily_rows(&empty_calendar, 22);

        assert_eq!(rows.len(), 2);
        let dots = rows[1]
            .spans
            .iter()
            .filter(|span| span.content.contains('·'))
            .collect::<Vec<_>>();
        assert_eq!(dots.len(), 7);
        assert!(
            dots.iter()
                .all(|span| span.style.add_modifier.contains(Modifier::DIM))
        );
        assert!(activity_daily_rows(&empty_calendar, 6).is_empty());
    }

    #[test]
    fn renders_cached_amp_activity_and_aggregate_details() {
        let state = ProviderState {
            result: Some(AmpActivityUsage {
                daily_usage_buckets: vec![
                    crate::model::AmpDailyUsageBucket {
                        date: "2026-08-01".into(),
                        tokens: 100_000_000,
                        orb_runtime_millis: 60_000,
                        covered_cost: 1.25,
                        sources: vec![
                            AmpTokenCategory {
                                label: "ChatGPT Pro".into(),
                                tokens: 99_000_000,
                            },
                            AmpTokenCategory {
                                label: "Amp".into(),
                                tokens: 1_000_000,
                            },
                        ],
                        models: vec![
                            AmpTokenCategory {
                                label: "GPT-5.6 Sol".into(),
                                tokens: 90_000_000,
                            },
                            AmpTokenCategory {
                                label: "GPT-5.5".into(),
                                tokens: 10_000_000,
                            },
                            AmpTokenCategory {
                                label: "Unused model".into(),
                                tokens: 0,
                            },
                            AmpTokenCategory {
                                label: "Rare model".into(),
                                tokens: 1,
                            },
                        ],
                        ..crate::model::AmpDailyUsageBucket::default()
                    },
                    crate::model::AmpDailyUsageBucket {
                        date: "2026-08-02".into(),
                        tokens: 200_000_000,
                        orb_runtime_millis: 3_600_000,
                        paid_cost: 0.50,
                        ..crate::model::AmpDailyUsageBucket::default()
                    },
                    crate::model::AmpDailyUsageBucket {
                        date: "2026-06-20".into(),
                        covered_cost: 99.0,
                        orb_runtime_millis: 86_400_000,
                        models: vec![AmpTokenCategory {
                            label: "Old model".into(),
                            tokens: 100_000_000,
                        }],
                        sources: vec![AmpTokenCategory {
                            label: "Old source".into(),
                            tokens: 100_000_000,
                        }],
                        ..crate::model::AmpDailyUsageBucket::default()
                    },
                ],
            }),
            ..ProviderState::default()
        };
        let mut lines = Vec::new();

        let sync = amp_activity_sync_message(&state, 40, date("2026-08-02")).unwrap();
        render_amp_activity(&mut lines, &state, 40, date("2026-08-02"));
        let rendered = lines.iter().map(line_text).collect::<Vec<_>>();
        let text = rendered.join("\n");
        let covered = rendered
            .iter()
            .find(|line| line.starts_with("Covered"))
            .unwrap();
        let orb_time = rendered
            .iter()
            .find(|line| line.starts_with("Orb time"))
            .unwrap();
        let sol = rendered
            .iter()
            .find(|line| line.starts_with("GPT-5.6 Sol"))
            .unwrap();
        let chatgpt = rendered
            .iter()
            .find(|line| line.starts_with("ChatGPT Pro"))
            .unwrap();

        assert_eq!(sync, "Amp Code activity history sync completes in ~4h 53m");
        assert!(!text.contains("activity history sync"));
        assert!(text.contains("Metric"));
        assert!(text.contains("1D"));
        assert!(text.contains("7D"));
        assert!(text.contains("30D"));
        assert_eq!(text.matches("7D").count(), 1);
        assert_eq!(text.matches("30D").count(), 1);
        assert!(covered.contains("$0.00"));
        assert_eq!(covered.matches("$1.25").count(), 2);
        assert!(orb_time.contains("1h0m"));
        assert_eq!(orb_time.matches("1h1m").count(), 2);
        assert!(!text.contains('█'));
        assert!(text.contains("Models"));
        assert_eq!(sol.matches("90M").count(), 2);
        assert!(!sol.contains('%'));
        assert!(text.contains("GPT-5.5"));
        assert_eq!(text.matches("10M").count(), 2);
        assert!(!text.contains("Unused model"));
        assert!(!text.contains("Rare model"));
        assert!(!text.contains("<1%"));
        assert!(text.contains("Sources"));
        assert_eq!(chatgpt.matches("99M").count(), 2);
        assert!(!chatgpt.contains('%'));
        assert!(text.contains("Amp"));
        assert_eq!(text.matches("1M").count(), 2);
        assert!(!text.contains("Old model"));
        assert!(!text.contains("Old source"));
        assert!(!text.contains("$100.25"));
    }

    #[test]
    fn combines_the_budget_pause_with_the_overall_sync_eta() {
        let state = ProviderState {
            result: Some(AmpActivityUsage {
                daily_usage_buckets: vec![crate::model::AmpDailyUsageBucket {
                    date: "2026-08-02".into(),
                    tokens: 1,
                    ..crate::model::AmpDailyUsageBucket::default()
                }],
            }),
            retry_after: Some(std::time::Duration::from_secs(40 * 60)),
            ..ProviderState::default()
        };

        let message = amp_activity_sync_message(&state, 40, date("2026-08-02")).unwrap();

        assert_eq!(
            message,
            "Amp Code activity history sync completes in ~5h 38m · resumes in ~40m"
        );
    }

    #[test]
    fn amp_activity_history_matches_the_full_visible_grid() {
        let today = date("2026-08-02");
        assert_eq!(amp_activity_history_days(40, today), 120);
        assert_eq!(amp_activity_history_days(58, today), 183);

        let usage = CodexActivityUsage {
            daily_usage_buckets: Some(vec![CodexDailyUsageBucket {
                start_date: "2026-08-02".into(),
                tokens: 1,
            }]),
            summary: None,
        };
        let calendar = activity_calendar(&usage, 40, today).unwrap();
        assert_eq!(
            today.signed_duration_since(calendar.start_week).num_days() as usize + 1,
            amp_activity_history_days(40, today)
        );
    }
}
