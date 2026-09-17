use chrono::{Datelike, NaiveDate};

/// Fixed, small palette cycled across whichever batches are selected —
/// same spirit as the .status badge colors, just enough distinct hues
/// to tell a handful of lines apart.
const COLORS: [&str; 6] = [
    "#2c3e50", "#5b7a4f", "#a3623e", "#7a4f7a", "#4f7a7a", "#a33333",
];

pub struct Series {
    pub batch_code: String,
    pub points: Vec<(String, f64)>, // (occurred_at ISO date, value)
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
/// axes are scaled to whatever data is actually present.
pub fn render_line_chart(series: &[Series]) -> String {
    let points_by_series: Vec<Vec<(f64, f64)>> = series
        .iter()
        .map(|s| {
            s.points
                .iter()
                .filter_map(|(date, value)| day_offset(date).map(|d| (d as f64, *value)))
                .collect()
        })
        .collect();

    let all_days: Vec<f64> = points_by_series.iter().flatten().map(|(d, _)| *d).collect();
    let all_values: Vec<f64> = points_by_series.iter().flatten().map(|(_, v)| *v).collect();

    if all_values.is_empty() {
        return "<p class=\"muted\">No data for the selected batches.</p>".to_string();
    }

    let min_day = all_days.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_day = all_days.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let day_span = if max_day > min_day { max_day - min_day } else { 1.0 };

    let min_v = all_values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_v = all_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let value_pad = if (max_v - min_v).abs() < 1e-9 { 1.0 } else { (max_v - min_v) * 0.1 };
    let (y_min, y_max) = (min_v - value_pad, max_v + value_pad);
    let value_span = if y_max > y_min { y_max - y_min } else { 1.0 };

    let plot_w = WIDTH - MARGIN_LEFT - MARGIN_RIGHT;
    let plot_h = HEIGHT - MARGIN_TOP - MARGIN_BOTTOM;

    let x_of = |day: f64| MARGIN_LEFT + (day - min_day) / day_span * plot_w;
    let y_of = |value: f64| MARGIN_TOP + plot_h - (value - y_min) / value_span * plot_h;

    let mut svg = format!(
        r#"<svg viewBox="0 0 {WIDTH} {HEIGHT}" xmlns="http://www.w3.org/2000/svg" class="chart-svg">"#
    );

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

    // X-axis first/last date labels.
    if let Some(first) = series.iter().flat_map(|s| s.points.first()).map(|(d, _)| d).next() {
        svg.push_str(&format!(
            r##"<text x="{x}" y="{y}" font-size="11" fill="#746f66" text-anchor="start">{first}</text>"##,
            x = MARGIN_LEFT,
            y = HEIGHT - MARGIN_BOTTOM + 16.0
        ));
    }
    if let Some(last) = series.iter().flat_map(|s| s.points.last()).map(|(d, _)| d).last() {
        svg.push_str(&format!(
            r##"<text x="{x}" y="{y}" font-size="11" fill="#746f66" text-anchor="end">{last}</text>"##,
            x = WIDTH - MARGIN_RIGHT,
            y = HEIGHT - MARGIN_BOTTOM + 16.0
        ));
    }

    for (i, points) in points_by_series.iter().enumerate() {
        let color = COLORS[i % COLORS.len()];
        if points.len() == 1 {
            let (d, v) = points[0];
            svg.push_str(&format!(
                r#"<circle cx="{cx}" cy="{cy}" r="4" fill="{color}" />"#,
                cx = x_of(d),
                cy = y_of(v)
            ));
        } else {
            let coords: Vec<String> = points
                .iter()
                .map(|(d, v)| format!("{:.1},{:.1}", x_of(*d), y_of(*v)))
                .collect();
            svg.push_str(&format!(
                r#"<polyline points="{}" fill="none" stroke="{color}" stroke-width="2" />"#,
                coords.join(" ")
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

fn day_offset(iso_date: &str) -> Option<i64> {
    NaiveDate::parse_from_str(iso_date, "%Y-%m-%d")
        .ok()
        .map(|d| d.num_days_from_ce() as i64)
}
