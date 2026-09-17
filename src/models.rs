use serde::{Deserialize, Deserializer};

/// Treats an empty string (as submitted by a blank HTML form field) as `None`
/// instead of `Some("")`.
pub fn empty_string_as_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Option<String> = Option::deserialize(deserializer)?;
    Ok(value.filter(|s| !s.trim().is_empty()))
}

/// Same, but for an optional numeric field submitted as text.
pub fn empty_number_as_none<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Option<String> = Option::deserialize(deserializer)?;
    match value.filter(|s| !s.trim().is_empty()) {
        Some(s) => s.parse::<f64>().map(Some).map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

#[derive(sqlx::FromRow)]
pub struct Season {
    pub id: i64,
    pub year: i64,
}

#[derive(Deserialize)]
pub struct SeasonForm {
    pub year: i64,
}

#[derive(sqlx::FromRow)]
pub struct Unit {
    pub id: i64,
    pub name: String,
}

#[derive(Deserialize)]
pub struct UnitForm {
    pub name: String,
}

pub struct UnitOption {
    pub id: i64,
    pub name: String,
    pub selected: bool,
}

#[derive(sqlx::FromRow)]
pub struct Tree {
    pub id: i64,
    pub name: String,
    pub variety: Option<String>,
    pub planted_on: Option<String>,
    pub location: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct TreeForm {
    pub name: String,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub variety: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub planted_on: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub location: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub notes: Option<String>,
}

// Built by hand in the handler (not `query_as!`), since `status` (and
// everything below it) is computed from the batch's events rather
// than fetched from a column.
pub struct BatchListItem {
    pub id: i64,
    pub code: String,
    pub name: Option<String>,
    pub status: &'static str,
    pub season_year: i64,
    pub started_on: Option<String>,
    pub bottled_on: Option<String>,
    pub trees: Vec<String>,
    pub additives: Vec<String>,
    /// Potential ABV assuming full attenuation to ~1.000 SG, from the
    /// batch's starting gravity alone — only trustworthy (and only
    /// ever `Some`) when that reading was taken the same day as the
    /// batch's first juicing event. See docs/specs/batch-list-columns.md.
    pub abv_pct: Option<f64>,
    pub days_since_bottling: Option<i64>,
}

#[derive(Deserialize)]
pub struct NewBatchForm {
    pub season_id: i64,
    pub code: String,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub name: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateBatchForm {
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub name: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub notes: Option<String>,
}

/// A single row already formatted for display in a batch's timeline.
/// Formatting the summary in Rust (rather than branching per event type in
/// the template) keeps the template a flat list, not a 7-way match.
pub struct TimelineRow {
    pub event_id: i64,
    pub occurred_at: String,
    pub kind_label: &'static str,
    /// Lowercase, matches the `{kind}` path segment used to edit this event.
    pub kind_slug: &'static str,
    pub summary: String,
}

/// One tree's checkbox + optional weight in the "Log juicing" /
/// juicing-edit form. `checked`/`weight_kg` are only meaningful when
/// rendering an edit form for an existing event; on the blank "add"
/// form every tree starts unchecked with no weight.
pub struct TreeWeightField {
    pub tree_id: i64,
    pub tree_name: String,
    pub checked: bool,
    pub weight_kg: Option<f64>,
}

pub struct MonthOption {
    pub value: u32,
    pub label: &'static str,
    pub selected: bool,
}

// Unlike picking/juicing, additives (yeast, nutrients, campden,
// potassium sorbate before bottling, ...) aren't guaranteed to fall
// within the harvest season year, so this takes a full date like
// measurement does.
#[derive(Deserialize)]
pub struct AddAdditiveForm {
    pub occurred_at: String,
    pub substance: String,
    pub amount: f64,
    pub unit_id: i64,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct AddMeasurementForm {
    pub occurred_at: String,
    #[serde(deserialize_with = "empty_number_as_none", default)]
    pub specific_gravity: Option<f64>,
    #[serde(deserialize_with = "empty_number_as_none", default)]
    pub ph: Option<f64>,
    #[serde(deserialize_with = "empty_number_as_none", default)]
    pub temperature_c: Option<f64>,
    #[serde(deserialize_with = "empty_number_as_none", default)]
    pub volume_l: Option<f64>,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub tasting_notes: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub notes: Option<String>,
}

pub struct TreeYield {
    pub tree_name: String,
    pub total_kg: f64,
}

#[derive(Deserialize)]
pub struct AddRackingForm {
    pub occurred_at: String,
    pub volume_l: f64,
    #[serde(deserialize_with = "empty_number_as_none", default)]
    pub loss_l: Option<f64>,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct AddBottlingForm {
    pub occurred_at: String,
    pub bottle_count: i64,
    pub bottle_size_ml: i64,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub carbonation_method: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct AddFailureForm {
    pub occurred_at: String,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct AddTastingForm {
    pub occurred_at: String,
    pub score: i64,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub tasting_notes: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub notes: Option<String>,
}

pub struct BatchOption {
    pub id: i64,
    pub code: String,
    pub checked: bool,
}

pub struct MetricOption {
    pub slug: &'static str,
    pub label: &'static str,
    pub checked: bool,
}

/// One metric's rendered chart, ready to drop into the page — the SVG
/// markup (or an empty-state message) plus its own legend.
pub struct ChartSection {
    pub label: &'static str,
    pub svg: String,
}

pub struct SeasonOption {
    pub id: i64,
    pub year: i64,
    pub selected: bool,
}

/// One batch's resolved phase boundaries for the Season overview
/// timeline. Already reduced to plain dates by the query layer —
/// `render_season_timeline` only turns these into x-coordinates, it
/// doesn't know anything about events or status.
pub struct BatchTimelineRow {
    pub code: String,
    pub start: String,
    pub racking_at: Option<String>,
    pub bottling_at: Option<String>,
    pub failed_at: Option<String>,
    /// `failed_at`, else `bottling_at`, else today — where the bar ends.
    pub end: String,
}
