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

pub fn empty_id_as_none<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Option<String> = Option::deserialize(deserializer)?;
    match value.filter(|s| !s.trim().is_empty()) {
        Some(s) => s.parse::<i64>().map(Some).map_err(serde::de::Error::custom),
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

#[derive(sqlx::FromRow)]
pub struct Vessel {
    pub id: i64,
    pub name: String,
    pub capacity_l: Option<f64>,
    pub kind: String,
    pub active: bool,
}

#[derive(Deserialize)]
pub struct VesselForm {
    pub name: String,
    #[serde(deserialize_with = "empty_number_as_none", default)]
    pub capacity_l: Option<f64>,
    pub kind: String,
    #[serde(default)]
    pub active: Option<String>, // present ("on") when the checkbox is checked
}

#[derive(sqlx::FromRow)]
pub struct BatchListItem {
    pub id: i64,
    pub code: String,
    pub name: Option<String>,
    pub status: String,
    pub season_year: i64,
    pub vessel_name: Option<String>,
}

#[derive(Deserialize)]
pub struct NewBatchForm {
    pub season_id: i64,
    pub code: String,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub name: Option<String>,
    #[serde(deserialize_with = "empty_id_as_none", default)]
    pub vessel_id: Option<i64>,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub started_on: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateBatchForm {
    pub status: String,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub name: Option<String>,
    #[serde(deserialize_with = "empty_id_as_none", default)]
    pub vessel_id: Option<i64>,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub notes: Option<String>,
}

/// A single row already formatted for display in a batch's timeline.
/// Formatting the summary in Rust (rather than branching per event type in
/// the template) keeps the template a flat list, not a 7-way match.
pub struct TimelineRow {
    pub occurred_at: String,
    pub kind_label: &'static str,
    pub summary: String,
}

#[derive(Deserialize)]
pub struct AddPickingForm {
    pub occurred_at: String,
    pub tree_id: i64,
    pub weight_kg: f64,
}

#[derive(Deserialize)]
pub struct AddJuicingForm {
    pub occurred_at: String,
    pub input_weight_kg: f64,
    pub output_volume_l: f64,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub equipment: Option<String>,
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
}

pub struct TreeYield {
    pub tree_name: String,
    pub total_kg: f64,
}
