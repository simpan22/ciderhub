use chrono::{Datelike, Months, NaiveDate};

use crate::models::BatchTimelineRow;

const MONTH_ABBREVS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Fixed, small palette cycled across whichever batches are selected —
/// same spirit as the .status badge colors, just enough distinct hues
/// to tell a handful of lines apart.
const COLORS: [&str; 6] = [
    "#2c3e50", "#5b7a4f", "#a3623e", "#7a4f7a", "#4f7a7a", "#a33333",
];

/// One data point already resolved to days-since-the-batch's-own-first-
/// juicing-event (not a calendar date — see docs/specs/plot-days-since-juicing.md)
/// plus a ready-made hover tooltip. `charts.rs` only knows about days
/// and pre-formatted text; the metric-specific value formatting and
/// day-offset math both happen in the caller (`dashboard.rs`).
pub struct SeriesPoint {
    pub day: i64,
    pub value: f64,
    pub tooltip: String,
}

pub struct Series {
    pub batch_code: String,
    pub points: Vec<SeriesPoint>,
}

const WIDTH: f64 = 640.0;
const HEIGHT: f64 = 260.0;
const MARGIN_LEFT: f64 = 50.0;
const MARGIN_RIGHT: f64 = 20.0;
const MARGIN_TOP: f64 = 16.0;
const MARGIN_BOTTOM: f64 = 36.0;

/// Renders one metric's chart as a small, self-contained SVG (plus an
/// HTML legend below it) — no charting library, per vision.md's stated
/// preference for server-rendered SVG. Each series gets its own line;
/// axes are scaled to whatever data is actually present. The x-axis is
/// always widened to include day 0 ("Fermentation Start"), even when a
/// series's own data starts later, so every chart shares the same
/// anchor point.
pub fn render_line_chart(series: &[Series]) -> String {
    let all_days: Vec<i64> = series.iter().flat_map(|s| s.points.iter().map(|p| p.day)).collect();
    let all_values: Vec<f64> = series.iter().flat_map(|s| s.points.iter().map(|p| p.value)).collect();

    if all_values.is_empty() {
        return "<p class=\"muted\">No data for the selected batches.</p>".to_string();
    }

    let min_day = all_days.iter().cloned().fold(0, i64::min);
    let max_day = all_days.iter().cloned().fold(0, i64::max);
    let day_span = if max_day > min_day { (max_day - min_day) as f64 } else { 1.0 };

    let min_v = all_values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_v = all_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let value_pad = if (max_v - min_v).abs() < 1e-9 { 1.0 } else { (max_v - min_v) * 0.1 };
    let (y_min, y_max) = (min_v - value_pad, max_v + value_pad);
    let value_span = if y_max > y_min { y_max - y_min } else { 1.0 };

    let plot_w = WIDTH - MARGIN_LEFT - MARGIN_RIGHT;
    let plot_h = HEIGHT - MARGIN_TOP - MARGIN_BOTTOM;

    let x_of = |day: i64| MARGIN_LEFT + (day - min_day) as f64 / day_span * plot_w;
    let y_of = |value: f64| MARGIN_TOP + plot_h - (value - y_min) / value_span * plot_h;

    let mut svg = format!(
        r#"<svg viewBox="0 0 {WIDTH} {HEIGHT}" xmlns="http://www.w3.org/2000/svg" class="chart-svg">"#
    );

    // Day-since-juicing tick gridlines, drawn first so the series lines
    // painted afterward sit on top of them — same treatment already
    // established for the Season overview's month gridlines, just in
    // days instead of months.
    for day in day_ticks(min_day, max_day) {
        let x = x_of(day);
        svg.push_str(&format!(
            r##"<line x1="{x:.1}" y1="{y0:.1}" x2="{x:.1}" y2="{y1:.1}" stroke="#ddd6c8" stroke-width="1" />"##,
            y0 = MARGIN_TOP,
            y1 = HEIGHT - MARGIN_BOTTOM
        ));
        let label = if day == 0 { "Fermentation Start".to_string() } else { day.to_string() };
        svg.push_str(&format!(
            r##"<text x="{x:.1}" y="{y:.1}" font-size="11" fill="#746f66" text-anchor="middle">{label}</text>"##,
            y = HEIGHT - MARGIN_BOTTOM + 16.0
        ));
    }

    // Axis lines.
    svg.push_str(&format!(
        r##"<line x1="{x0}" y1="{y0}" x2="{x0}" y2="{y1}" stroke="#8899aa" stroke-width="1" />"##,
        x0 = MARGIN_LEFT,
        y0 = MARGIN_TOP,
        y1 = HEIGHT - MARGIN_BOTTOM
    ));
    svg.push_str(&format!(
        r##"<line x1="{x0}" y1="{y1}" x2="{x2}" y2="{y1}" stroke="#8899aa" stroke-width="1" />"##,
        x0 = MARGIN_LEFT,
        y1 = HEIGHT - MARGIN_BOTTOM,
        x2 = WIDTH - MARGIN_RIGHT
    ));

    // Y-axis min/max labels.
    svg.push_str(&format!(
        r##"<text x="{x}" y="{y}" font-size="11" fill="#746f66" text-anchor="end">{v:.3}</text>"##,
        x = MARGIN_LEFT - 6.0,
        y = y_of(y_max) + 4.0,
        v = max_v
    ));
    svg.push_str(&format!(
        r##"<text x="{x}" y="{y}" font-size="11" fill="#746f66" text-anchor="end">{v:.3}</text>"##,
        x = MARGIN_LEFT - 6.0,
        y = y_of(y_min) + 4.0,
        v = min_v
    ));

    for (i, s) in series.iter().enumerate() {
        let color = COLORS[i % COLORS.len()];

        if s.points.len() >= 2 {
            let coords: Vec<String> = s
                .points
                .iter()
                .map(|p| format!("{:.1},{:.1}", x_of(p.day), y_of(p.value)))
                .collect();
            svg.push_str(&format!(
                r#"<polyline points="{}" fill="none" stroke="{color}" stroke-width="2" />"#,
                coords.join(" ")
            ));
        }

        // Every point gets a dot with a hover tooltip, whether or not
        // it's also part of a connecting line.
        for p in &s.points {
            svg.push_str(&format!(
                r#"<circle cx="{cx:.1}" cy="{cy:.1}" r="4" fill="{color}"><title>{tooltip}</title></circle>"#,
                cx = x_of(p.day),
                cy = y_of(p.value),
                tooltip = escape_xml_text(&p.tooltip)
            ));
        }
    }

    svg.push_str("</svg>");

    let legend: String = series
        .iter()
        .enumerate()
        .map(|(i, s)| {
            format!(
                r#"<span class="chart-legend__item"><span class="chart-legend__swatch" style="background:{}"></span>{}</span>"#,
                COLORS[i % COLORS.len()],
                s.batch_code
            )
        })
        .collect();

    format!(r#"{svg}<div class="chart-legend">{legend}</div>"#)
}

/// Tick interval in days, chosen from the axis's day-span so a chart
/// never ends up with either a handful of ticks crammed together or
/// dozens of them — same fixed-threshold approach already used for the
/// Season overview's month gridlines.
fn day_tick_interval(span_days: i64) -> i64 {
    match span_days {
        d if d <= 14 => 2,
        d if d <= 30 => 5,
        d if d <= 90 => 10,
        d if d <= 180 => 30,
        _ => 60,
    }
}

/// Every tick day within `[axis_min, axis_max]`, spaced by
/// `day_tick_interval`'s result and always including `0` — the axis
/// itself is always widened to include `0`, so this is really just
/// "every Nth day counting outward from Fermentation Start."
fn day_ticks(axis_min: i64, axis_max: i64) -> Vec<i64> {
    let interval = day_tick_interval(axis_max - axis_min);
    let mut ticks = Vec::new();

    let mut day = 0;
    while day >= axis_min {
        ticks.push(day);
        day -= interval;
    }
    let mut day = interval;
    while day <= axis_max {
        ticks.push(day);
        day += interval;
    }

    ticks.sort_unstable();
    ticks
}

/// Minimal XML text-content escaping for the handful of characters
/// that are structurally significant inside a `<title>` element —
/// needed because tooltip text can include a user-entered event note,
/// unlike every other value this module embeds (numbers, dates).
fn escape_xml_text(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn day_offset(iso_date: &str) -> Option<i64> {
    NaiveDate::parse_from_str(iso_date, "%Y-%m-%d")
        .ok()
        .map(|d| d.num_days_from_ce() as i64)
}

// Color encodes phase here, not batch identity (the opposite of
// render_line_chart) — every bar is the same "thing" (a batch), so
// color is free to carry which part of the process a stretch of time
// was in; the row label already carries batch identity.
const PHASE_FERMENTING: &str = "#5b7a4f";
const PHASE_CONDITIONING: &str = "#a3623e";
const PHASE_BOTTLED: &str = "#2c3e50";
const PHASE_FAILED: &str = "#a33333";

const TIMELINE_WIDTH: f64 = 640.0;
const TIMELINE_MARGIN_LEFT: f64 = 90.0;
const TIMELINE_MARGIN_RIGHT: f64 = 20.0;
const TIMELINE_MARGIN_TOP: f64 = 16.0;
const TIMELINE_ROW_H: f64 = 34.0;
const TIMELINE_BAR_H: f64 = 16.0;
const TIMELINE_AXIS_PAD_DAYS: i64 = 3;

struct ResolvedRow {
    code: String,
    start_day: i64,
    end_day: i64,
    racking_day: Option<i64>,
    bottling_day: Option<i64>,
    failed_day: Option<i64>,
}

/// Renders the Season overview: one horizontal row per batch, spanning
/// from its start to its end (failed/bottled/today), with colored
/// segments for each phase it has actually passed through. Every row
/// shares one x-axis, per the todo's explicit "duplicated timelines,
/// same interval" requirement.
pub fn render_season_timeline(rows: &[BatchTimelineRow]) -> String {
    let mut resolved = Vec::new();
    let mut min_day = i64::MAX;
    let mut max_day = i64::MIN;
    let mut earliest_label: Option<&str> = None;
    let mut latest_label: Option<&str> = None;

    for row in rows {
        let (Some(start_day), Some(end_day)) = (day_offset(&row.start), day_offset(&row.end)) else {
            continue;
        };
        if start_day < min_day {
            min_day = start_day;
            earliest_label = Some(&row.start);
        }
        if end_day > max_day {
            max_day = end_day;
            latest_label = Some(&row.end);
        }
        resolved.push(ResolvedRow {
            code: row.code.clone(),
            start_day,
            end_day,
            racking_day: row.racking_at.as_deref().and_then(day_offset),
            bottling_day: row.bottling_at.as_deref().and_then(day_offset),
            failed_day: row.failed_at.as_deref().and_then(day_offset),
        });
    }

    if resolved.is_empty() {
        return "<p class=\"muted\">No batches with logged events yet.</p>".to_string();
    }

    let axis_min = min_day - TIMELINE_AXIS_PAD_DAYS;
    let axis_max = (max_day + TIMELINE_AXIS_PAD_DAYS).max(axis_min + 1);
    let axis_span = (axis_max - axis_min) as f64;

    let plot_w = TIMELINE_WIDTH - TIMELINE_MARGIN_LEFT - TIMELINE_MARGIN_RIGHT;
    let x_of = |day: i64| TIMELINE_MARGIN_LEFT + (day - axis_min) as f64 / axis_span * plot_w;

    let axis_y = TIMELINE_MARGIN_TOP + resolved.len() as f64 * TIMELINE_ROW_H + 6.0;
    let height = axis_y + 20.0;

    let mut svg = format!(
        r#"<svg viewBox="0 0 {TIMELINE_WIDTH} {height}" xmlns="http://www.w3.org/2000/svg" class="chart-svg">"#
    );

    // Month gridlines first, so the phase bars painted afterward sit
    // on top of them rather than the other way around.
    for (day, label) in month_gridlines(axis_min, axis_max) {
        let x = x_of(day);
        svg.push_str(&format!(
            r##"<line x1="{x:.1}" y1="{y0:.1}" x2="{x:.1}" y2="{axis_y:.1}" stroke="#ddd6c8" stroke-width="1" />"##,
            y0 = TIMELINE_MARGIN_TOP
        ));
        svg.push_str(&format!(
            r##"<text x="{x:.1}" y="{ty:.1}" font-size="11" fill="#746f66" text-anchor="middle">{label}</text>"##,
            ty = axis_y + 16.0
        ));
    }

    for (i, r) in resolved.iter().enumerate() {
        let y = TIMELINE_MARGIN_TOP + i as f64 * TIMELINE_ROW_H;
        let cy = y + TIMELINE_BAR_H / 2.0;

        svg.push_str(&format!(
            r##"<text x="{x:.1}" y="{ty:.1}" font-size="12" fill="#3a352c" text-anchor="end">{code}</text>"##,
            x = TIMELINE_MARGIN_LEFT - 8.0,
            ty = cy + 4.0,
            code = r.code
        ));

        // Zero-width span (e.g. a batch whose only event is its own
        // "failed" event) — a bar has no width to draw, so mark the
        // point with a dot instead, same fallback render_line_chart
        // uses for a single-point series.
        if r.start_day == r.end_day {
            let color = if r.failed_day.is_some() {
                PHASE_FAILED
            } else if r.bottling_day.is_some() {
                PHASE_BOTTLED
            } else {
                PHASE_FERMENTING
            };
            svg.push_str(&format!(
                r#"<circle cx="{cx:.1}" cy="{cy:.1}" r="5" fill="{color}" />"#,
                cx = x_of(r.start_day)
            ));
            continue;
        }

        if let Some(failed_day) = r.failed_day {
            // Failure overrides every other phase for the bar's whole
            // span, matching compute_status's override — even if a
            // bottling event exists somewhere in the data, it isn't
            // reflected here.
            draw_segment(&mut svg, x_of(r.start_day), x_of(failed_day), y, PHASE_FAILED);
            continue;
        }

        // Fermenting: start until racking (or until the bar's end, if
        // never racked). Clipped to the bar's end so an out-of-order
        // racking date logged after bottling can't draw past it.
        let fermenting_end = r.racking_day.unwrap_or(r.end_day).clamp(r.start_day, r.end_day);
        draw_segment(&mut svg, x_of(r.start_day), x_of(fermenting_end), y, PHASE_FERMENTING);

        if let Some(racking_day) = r.racking_day {
            let conditioning_start = racking_day.clamp(r.start_day, r.end_day);
            if r.end_day > conditioning_start {
                draw_segment(&mut svg, x_of(conditioning_start), x_of(r.end_day), y, PHASE_CONDITIONING);
            }
        }

        // Bottling is a moment, not a phase with duration — a thin
        // end-cap marker, not a shaded span.
        if let Some(bottling_day) = r.bottling_day {
            svg.push_str(&format!(
                r#"<circle cx="{cx:.1}" cy="{cy:.1}" r="4" fill="{PHASE_BOTTLED}" />"#,
                cx = x_of(bottling_day)
            ));
        }
    }

    svg.push_str(&format!(
        r##"<line x1="{x0:.1}" y1="{axis_y:.1}" x2="{x1:.1}" y2="{axis_y:.1}" stroke="#8899aa" stroke-width="1" />"##,
        x0 = TIMELINE_MARGIN_LEFT,
        x1 = TIMELINE_WIDTH - TIMELINE_MARGIN_RIGHT
    ));
    if let Some(label) = earliest_label {
        svg.push_str(&format!(
            r##"<text x="{x:.1}" y="{y:.1}" font-size="11" fill="#746f66" text-anchor="start">{label}</text>"##,
            x = TIMELINE_MARGIN_LEFT,
            y = axis_y + 16.0
        ));
    }
    if let Some(label) = latest_label {
        svg.push_str(&format!(
            r##"<text x="{x:.1}" y="{y:.1}" font-size="11" fill="#746f66" text-anchor="end">{label}</text>"##,
            x = TIMELINE_WIDTH - TIMELINE_MARGIN_RIGHT,
            y = axis_y + 16.0
        ));
    }

    svg.push_str("</svg>");

    let legend = format!(
        r##"<div class="chart-legend">
            <span class="chart-legend__item"><span class="chart-legend__swatch" style="background:{PHASE_FERMENTING}"></span>Fermenting</span>
            <span class="chart-legend__item"><span class="chart-legend__swatch" style="background:{PHASE_CONDITIONING}"></span>Conditioning</span>
            <span class="chart-legend__item"><span class="chart-legend__swatch" style="background:{PHASE_BOTTLED}"></span>Bottled</span>
            <span class="chart-legend__item"><span class="chart-legend__swatch" style="background:{PHASE_FAILED}"></span>Failed</span>
        </div>"##
    );

    format!("{svg}{legend}")
}

/// The 1st of every month falling within `[axis_min, axis_max]`
/// (day-of-common-era offsets), paired with its 3-letter abbreviation.
fn month_gridlines(axis_min: i64, axis_max: i64) -> Vec<(i64, &'static str)> {
    let (Some(min_date), Some(max_date)) = (
        NaiveDate::from_num_days_from_ce_opt(axis_min as i32),
        NaiveDate::from_num_days_from_ce_opt(axis_max as i32),
    ) else {
        return Vec::new();
    };

    let mut cursor = NaiveDate::from_ymd_opt(min_date.year(), min_date.month(), 1).unwrap_or(min_date);
    let mut lines = Vec::new();
    loop {
        if cursor > max_date {
            break;
        }
        let day = cursor.num_days_from_ce() as i64;
        if day >= axis_min {
            lines.push((day, MONTH_ABBREVS[cursor.month0() as usize]));
        }
        cursor = match cursor.checked_add_months(Months::new(1)) {
            Some(next) => next,
            None => break,
        };
    }
    lines
}

fn draw_segment(svg: &mut String, x0: f64, x1: f64, y: f64, color: &str) {
    let (x0, x1) = if x1 >= x0 { (x0, x1) } else { (x1, x0) };
    svg.push_str(&format!(
        r#"<rect x="{x0:.1}" y="{y:.1}" width="{w:.1}" height="{TIMELINE_BAR_H:.1}" fill="{color}" rx="2" />"#,
        w = x1 - x0
    ));
}
