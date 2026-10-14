//! Bradley-Terry taste rating from pairwise comparisons — see
//! docs/specs/bradley-terry-taste-rating.md.
//!
//! Pure and database-free, so it can be unit tested directly. The
//! rating is recomputed from every comparison on each request rather
//! than stored, same as `status` — it can never drift from the log.

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Win,
    Loss,
    Tie,
}

impl Outcome {
    /// The column value in `comparison_events.outcome`.
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Win => "win",
            Outcome::Loss => "loss",
            Outcome::Tie => "tie",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "win" => Some(Outcome::Win),
            "loss" => Some(Outcome::Loss),
            "tie" => Some(Outcome::Tie),
            _ => None,
        }
    }

    /// The same result seen from the other batch's side.
    pub fn flipped(self) -> Self {
        match self {
            Outcome::Win => Outcome::Loss,
            Outcome::Loss => Outcome::Win,
            Outcome::Tie => Outcome::Tie,
        }
    }
}

/// One logged comparison between batches `a` and `b`, `outcome` from
/// `a`'s point of view.
pub struct Comparison {
    pub a: i64,
    pub b: i64,
    pub outcome: Outcome,
}

/// A `Comparison` plus the date it was logged on (`YYYY-MM-DD`).
pub struct DatedComparison {
    pub occurred_at: String,
    pub comparison: Comparison,
}

#[derive(Clone, Copy, Debug)]
pub struct Rating {
    /// Elo-style: 1500 is an average batch, +400 is ~10:1 odds of
    /// being preferred.
    pub rating: f64,
    pub comparisons: u32,
}

const MAX_ITERATIONS: usize = 10_000;
const TOLERANCE: f64 = 1e-10;

/// Fits Bradley-Terry strengths with Hunter's (2004) MM iteration.
///
/// Ties count as half a win for each side. Every batch also gets one
/// virtual tie against a fixed "average batch" of strength 1: without
/// it, a batch that has won every comparison would get an infinite
/// strength, and groups of batches never compared with each other
/// would have no common scale. Its pull towards average fades as real
/// comparisons accumulate.
///
/// Only batches with at least one comparison appear in the result.
pub fn fit(comparisons: &[Comparison]) -> HashMap<i64, Rating> {
    let mut index: HashMap<i64, usize> = HashMap::new();
    let mut ids: Vec<i64> = Vec::new();
    let mut games: Vec<(usize, usize)> = Vec::new();
    // Starts at 0.5: the virtual tie's half-win.
    let mut wins: Vec<f64> = Vec::new();
    let mut counts: Vec<u32> = Vec::new();

    for c in comparisons {
        if c.a == c.b {
            continue;
        }
        let mut slot = |id: i64| {
            *index.entry(id).or_insert_with(|| {
                ids.push(id);
                wins.push(0.5);
                counts.push(0);
                ids.len() - 1
            })
        };
        let (a, b) = (slot(c.a), slot(c.b));
        match c.outcome {
            Outcome::Win => wins[a] += 1.0,
            Outcome::Loss => wins[b] += 1.0,
            Outcome::Tie => {
                wins[a] += 0.5;
                wins[b] += 0.5;
            }
        }
        counts[a] += 1;
        counts[b] += 1;
        games.push((a, b));
    }

    let mut strength = vec![1.0_f64; ids.len()];
    for _ in 0..MAX_ITERATIONS {
        // The virtual opponent's term: 1 / (p_i + 1).
        let mut denom: Vec<f64> = strength.iter().map(|p| 1.0 / (p + 1.0)).collect();
        for &(a, b) in &games {
            let d = 1.0 / (strength[a] + strength[b]);
            denom[a] += d;
            denom[b] += d;
        }

        let mut max_change = 0.0_f64;
        let next: Vec<f64> = (0..ids.len())
            .map(|i| {
                let p = wins[i] / denom[i];
                max_change = max_change.max(((p - strength[i]) / strength[i]).abs());
                p
            })
            .collect();
        strength = next;
        if max_change < TOLERANCE {
            break;
        }
    }

    ids.iter()
        .enumerate()
        .map(|(i, &id)| {
            (
                id,
                Rating {
                    rating: 1500.0 + 400.0 * strength[i].log10(),
                    comparisons: counts[i],
                },
            )
        })
        .collect()
}

/// One point in a batch's rating history.
#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub date: String,
    pub rating: Rating,
    /// False only for the closing entry described on `history`.
    pub compared: bool,
}

/// For each date (ascending) on which a batch was compared, its rating
/// from a fit over every comparison up to and including that date —
/// what was known then, so later comparisons never change earlier
/// entries. A batch only gets entries on dates *it* was compared,
/// not whenever two other batches were.
///
/// Comparisons between other batches still move a batch's rating a
/// little, so when its current rating differs from its last entry, one
/// closing entry (`compared: false`) is added at the date of the latest
/// comparison overall. That keeps each history ending at the same
/// value as `fit` over everything, i.e. what the batch list shows. See
/// docs/specs/plot-comparative-rating.md.
pub fn history(comparisons: &[DatedComparison]) -> HashMap<i64, Vec<HistoryEntry>> {
    let mut sorted: Vec<&DatedComparison> = comparisons.iter().collect();
    sorted.sort_by(|a, b| a.occurred_at.cmp(&b.occurred_at));

    let mut history: HashMap<i64, Vec<HistoryEntry>> = HashMap::new();
    let mut so_far: Vec<Comparison> = Vec::new();
    let mut i = 0;
    while i < sorted.len() {
        let date = &sorted[i].occurred_at;
        let mut compared_today: Vec<i64> = Vec::new();
        while i < sorted.len() && &sorted[i].occurred_at == date {
            let c = &sorted[i].comparison;
            so_far.push(Comparison { a: c.a, b: c.b, outcome: c.outcome });
            for id in [c.a, c.b] {
                if !compared_today.contains(&id) {
                    compared_today.push(id);
                }
            }
            i += 1;
        }

        let ratings = fit(&so_far);
        for id in compared_today {
            if let Some(rating) = ratings.get(&id) {
                history.entry(id).or_default().push(HistoryEntry {
                    date: date.clone(),
                    rating: *rating,
                    compared: true,
                });
            }
        }
    }

    if let Some(last) = sorted.last() {
        let current = fit(&so_far);
        for (id, entries) in history.iter_mut() {
            let (Some(entry), Some(now)) = (entries.last(), current.get(id)) else {
                continue;
            };
            if entry.date != last.occurred_at && (entry.rating.rating - now.rating).abs() > 1e-6 {
                entries.push(HistoryEntry {
                    date: last.occurred_at.clone(),
                    rating: *now,
                    compared: false,
                });
            }
        }
    }

    history
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmp(a: i64, b: i64, outcome: Outcome) -> Comparison {
        Comparison { a, b, outcome }
    }

    fn rating(ratings: &HashMap<i64, Rating>, id: i64) -> f64 {
        ratings[&id].rating
    }

    #[test]
    fn single_win_splits_around_average() {
        let r = fit(&[cmp(1, 2, Outcome::Win)]);
        assert!(rating(&r, 1) > 1500.0);
        assert!(rating(&r, 2) < 1500.0);
        assert!((rating(&r, 1) - 1500.0 - (1500.0 - rating(&r, 2))).abs() < 1e-6);
    }

    #[test]
    fn unbeaten_batch_stays_finite() {
        let r = fit(&[
            cmp(1, 2, Outcome::Win),
            cmp(1, 2, Outcome::Win),
            cmp(1, 3, Outcome::Win),
            cmp(3, 1, Outcome::Loss),
        ]);
        assert!(rating(&r, 1).is_finite());
        assert!(rating(&r, 1) > rating(&r, 3));
    }

    #[test]
    fn disconnected_groups_share_a_scale() {
        let r = fit(&[cmp(1, 2, Outcome::Win), cmp(3, 4, Outcome::Win)]);
        assert!((rating(&r, 1) - rating(&r, 3)).abs() < 1e-6);
        assert!((rating(&r, 2) - rating(&r, 4)).abs() < 1e-6);
    }

    #[test]
    fn ties_only_leaves_everyone_average() {
        let r = fit(&[cmp(1, 2, Outcome::Tie), cmp(2, 3, Outcome::Tie)]);
        for id in [1, 2, 3] {
            assert!((rating(&r, id) - 1500.0).abs() < 1e-6);
        }
    }

    #[test]
    fn symmetric_cycle_is_all_equal() {
        let r = fit(&[
            cmp(1, 2, Outcome::Win),
            cmp(2, 3, Outcome::Win),
            cmp(3, 1, Outcome::Win),
        ]);
        for id in [1, 2, 3] {
            assert!((rating(&r, id) - 1500.0).abs() < 1e-6);
        }
    }

    #[test]
    fn swapping_sides_gives_the_same_ratings() {
        let r1 = fit(&[cmp(1, 2, Outcome::Win), cmp(2, 3, Outcome::Tie)]);
        let r2 = fit(&[cmp(2, 1, Outcome::Loss), cmp(3, 2, Outcome::Tie)]);
        for id in [1, 2, 3] {
            assert!((rating(&r1, id) - rating(&r2, id)).abs() < 1e-9);
        }
    }

    #[test]
    fn matches_reference_values() {
        // Reference computed independently by gradient ascent on the
        // same regularised log-likelihood (not the MM update).
        let r = fit(&[
            cmp(1, 2, Outcome::Win),
            cmp(1, 2, Outcome::Win),
            cmp(2, 3, Outcome::Win),
            cmp(3, 1, Outcome::Win),
        ]);
        assert!((rating(&r, 1) - 1559.586_315_672_4).abs() < 1e-6);
        assert!((rating(&r, 2) - 1440.413_684_327_6).abs() < 1e-6);
        assert!((rating(&r, 3) - 1500.0).abs() < 1e-6);
    }

    fn dated(occurred_at: &str, a: i64, b: i64, outcome: Outcome) -> DatedComparison {
        DatedComparison { occurred_at: occurred_at.to_string(), comparison: cmp(a, b, outcome) }
    }

    #[test]
    fn history_ends_at_the_full_fit() {
        let log = [
            dated("2026-10-03", 1, 2, Outcome::Win),
            dated("2026-10-01", 2, 3, Outcome::Tie),
            dated("2026-10-02", 3, 1, Outcome::Win),
        ];
        let all: Vec<Comparison> = log
            .iter()
            .map(|d| cmp(d.comparison.a, d.comparison.b, d.comparison.outcome))
            .collect();
        let full = fit(&all);
        let h = history(&log);
        for id in [1, 2, 3] {
            let last = h[&id].last().unwrap().rating.rating;
            assert!((last - full[&id].rating).abs() < 1e-6, "{id}: {last} vs {}", full[&id].rating);
        }
    }

    #[test]
    fn history_has_dates_the_batch_was_compared_plus_a_closing_entry() {
        let h = history(&[
            dated("2026-10-01", 1, 2, Outcome::Win),
            dated("2026-10-02", 2, 3, Outcome::Win),
            dated("2026-10-02", 2, 3, Outcome::Tie),
            dated("2026-10-04", 2, 3, Outcome::Win),
        ]);
        let dates = |id: i64| {
            h[&id]
                .iter()
                .map(|e| (e.date.as_str(), e.compared))
                .collect::<Vec<_>>()
        };
        // Batch 1 was only compared once; 2-vs-3 results since then
        // moved its rating, so it gets one closing entry at the last
        // comparison date rather than one per day.
        assert_eq!(dates(1), [("2026-10-01", true), ("2026-10-04", false)]);
        // Two comparisons on one day give one entry, after both.
        assert_eq!(dates(2), [("2026-10-01", true), ("2026-10-02", true), ("2026-10-04", true)]);
        assert_eq!(dates(3), [("2026-10-02", true), ("2026-10-04", true)]);
        assert_eq!(h[&2][1].rating.comparisons, 3);
    }

    #[test]
    fn later_comparisons_leave_earlier_entries_alone() {
        let early = [dated("2026-10-01", 1, 2, Outcome::Win)];
        let more = [
            dated("2026-10-01", 1, 2, Outcome::Win),
            dated("2026-10-05", 2, 1, Outcome::Win),
            dated("2026-10-06", 2, 1, Outcome::Win),
        ];
        let before = history(&early)[&1][0].rating.rating;
        let h = history(&more);
        assert!((h[&1][0].rating.rating - before).abs() < 1e-12);
        assert!(h[&1].last().unwrap().rating.rating < before);
    }

    #[test]
    fn counts_comparisons_and_skips_unrated() {
        let r = fit(&[cmp(1, 2, Outcome::Win), cmp(1, 3, Outcome::Tie), cmp(4, 4, Outcome::Win)]);
        assert_eq!(r[&1].comparisons, 2);
        assert_eq!(r[&2].comparisons, 1);
        assert!(!r.contains_key(&4));
    }
}
