use chrono::{Datelike, Local, NaiveDate};

use crate::models::MonthOption;

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

pub fn today_iso() -> String {
    Local::now().date_naive().to_string()
}

pub fn today_month() -> u32 {
    Local::now().date_naive().month()
}

pub fn today_day() -> u32 {
    Local::now().date_naive().day()
}

/// Combines a batch's season year with a user-picked month/day — used for
/// event types (picking, juicing) that always fall within their batch's
/// own harvest season, so the form never asks for a year at all.
pub fn season_date(year: i64, month: u32, day: u32) -> anyhow::Result<String> {
    let date = NaiveDate::from_ymd_opt(year as i32, month, day)
        .ok_or_else(|| anyhow::anyhow!("{year:04}-{month:02}-{day:02} is not a valid date"))?;
    Ok(date.to_string())
}

pub fn month_options() -> Vec<MonthOption> {
    let current = today_month();
    MONTH_NAMES
        .iter()
        .enumerate()
        .map(|(i, label)| MonthOption {
            value: i as u32 + 1,
            label,
            selected: i as u32 + 1 == current,
        })
        .collect()
}
