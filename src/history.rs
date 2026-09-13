//! Burn-rate tracking: how fast a window is draining, and what that means.
//!
//! A percentage answers "how much is left". It does not answer the question
//! you actually have, which is "will it last until it resets". Two or more
//! readings of the same window give a slope, and a slope plus a reset time
//! gives an answer.
//!
//! Samples persist across restarts, because a projection that needs four
//! minutes of fresh readings to appear is switched off for most of the time
//! you would want it. What is stored is percentages and timestamps and
//! nothing else — the same posture as `state.json` — under the same
//! `projections` switch that governs the feature, so turning it off stops the
//! writing too rather than merely hiding the result.

use std::collections::HashMap;
use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::types::{fmt_countdown, now_unix, Reading, Snapshot};

/// Ignore anything older than this: yesterday's pace says nothing about now.
const WINDOW_SECS: u64 = 3 * 3600;
/// Below this span the slope is mostly noise.
const MIN_SPAN_SECS: u64 = 240;
/// A jump up this large means the window refilled; the old slope is void.
const REFILL_JUMP: f64 = 10.0;
const MAX_SAMPLES: usize = 64;

#[derive(Default, Serialize, Deserialize)]
pub struct History {
    series: HashMap<String, VecDeque<(u64, f64)>>,
}

fn path() -> std::path::PathBuf {
    dirs::data_dir()
        .unwrap_or_default()
        .join("limitcue")
        .join("history.json")
}

impl History {
    /// Read back what the last run saw, so a projection is available at the
    /// first hover rather than four minutes in. Anything older than the
    /// tracking window is dropped on the way in — a reading from yesterday
    /// says nothing about the current pace.
    pub fn load() -> Self {
        let mut me: History = std::fs::read_to_string(path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let cutoff = now_unix().saturating_sub(WINDOW_SECS);
        for series in me.series.values_mut() {
            while series.front().is_some_and(|(t, _)| *t < cutoff) {
                series.pop_front();
            }
        }
        me.series.retain(|_, s| !s.is_empty());
        me
    }

    /// Write the samples out. Cheap enough to do on every poll: a handful of
    /// numbers per window, and polls are minutes apart.
    pub fn save(&self) {
        let p = path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(j) = serde_json::to_string(self) {
            let _ = std::fs::write(p, j);
        }
    }

    /// Forget everything, on disk as well as in memory, for when the feature
    /// is switched off. Leaving a file behind that the user has just asked us
    /// to stop keeping would be the wrong way round.
    pub fn forget() {
        let _ = std::fs::remove_file(path());
    }
}

pub fn key(provider: &str, window: &str) -> String {
    format!("{provider}|{window}")
}

impl History {
    /// Fold a batch of readings in, dropping anything stale.
    pub fn record(&mut self, snaps: &[Snapshot]) {
        for s in snaps {
            let Reading::Ok { windows, .. } = &s.reading else {
                continue;
            };
            for w in windows {
                let Some(pct) = w.remaining_percent else {
                    continue;
                };
                let series = self
                    .series
                    .entry(key(&s.provider_id, &w.label))
                    .or_default();
                // A refill resets the clock: averaging across it would report
                // a gentle drain for a window that just jumped back to full.
                if let Some((_, last)) = series.back() {
                    if pct > last + REFILL_JUMP {
                        series.clear();
                    }
                }
                if series.back().map(|(t, _)| *t) == Some(s.fetched_at) {
                    continue;
                }
                series.push_back((s.fetched_at, pct));
                while series.len() > MAX_SAMPLES {
                    series.pop_front();
                }
                let cutoff = s.fetched_at.saturating_sub(WINDOW_SECS);
                while series.front().is_some_and(|(t, _)| *t < cutoff) {
                    series.pop_front();
                }
            }
        }
    }

    /// Percent consumed per hour, positive while draining. `None` until there
    /// is enough spread to mean anything.
    pub fn burn_per_hour(&self, key: &str) -> Option<f64> {
        let s = self.series.get(key)?;
        if s.len() < 2 {
            return None;
        }
        let (t0, _) = *s.front()?;
        let (t1, _) = *s.back()?;
        if t1.saturating_sub(t0) < MIN_SPAN_SECS {
            return None;
        }
        // Least squares over (hours since t0, percent). Averaging beats
        // first-to-last: one odd reading should not redraw the whole trend.
        let n = s.len() as f64;
        let pts: Vec<(f64, f64)> = s
            .iter()
            .map(|(t, p)| ((t.saturating_sub(t0)) as f64 / 3600.0, *p))
            .collect();
        let mean_x = pts.iter().map(|(x, _)| x).sum::<f64>() / n;
        let mean_y = pts.iter().map(|(_, y)| y).sum::<f64>() / n;
        let num: f64 = pts.iter().map(|(x, y)| (x - mean_x) * (y - mean_y)).sum();
        let den: f64 = pts.iter().map(|(x, _)| (x - mean_x).powi(2)).sum();
        if den <= f64::EPSILON {
            return None;
        }
        let slope = num / den; // percent per hour, negative while draining
        Some(-slope)
    }
}

/// What the current pace means for this window, in the words you would use.
/// `None` when there is no trend, when it is not draining, or when the answer
/// would not change anything.
pub fn projection(remaining: f64, burn_per_hour: f64, resets_in: Option<u64>) -> Option<String> {
    if burn_per_hour <= 0.5 {
        return None; // flat, or refilling
    }
    let hours_left = remaining / burn_per_hour;
    let secs_left = (hours_left * 3600.0) as u64;
    match resets_in {
        // Kept short on purpose: this sits on a 264 pt card, and an elided
        // projection is worse than none.
        Some(reset) if secs_left < reset => Some(format!(
            "Out {} before the reset",
            fmt_countdown(reset - secs_left)
        )),
        Some(reset) => {
            let at_reset = remaining - burn_per_hour * (reset as f64 / 3600.0);
            Some(format!("About {:.0}% left at the reset", at_reset.max(0.0)))
        }
        None => Some(format!("Out in {}", fmt_countdown(secs_left))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Fidelity, Window};

    fn snap(t: u64, pct: f64) -> Snapshot {
        Snapshot {
            provider_id: "p".into(),
            display_name: "P".into(),
            fidelity: Fidelity::Official,
            reading: Reading::Ok {
                windows: vec![Window {
                    label: "5h".into(),
                    remaining_percent: Some(pct),
                    remaining_count: None,
                    total_count: None,
                    resets_at: None,
                }],
                detail: None,
            },
            fetched_at: t,
            fetch_error: None,
        }
    }

    #[test]
    fn one_reading_is_not_a_trend() {
        let mut h = History::default();
        h.record(&[snap(0, 90.0)]);
        assert!(h.burn_per_hour(&key("p", "5h")).is_none());
    }

    #[test]
    fn two_close_readings_are_not_a_trend_either() {
        let mut h = History::default();
        h.record(&[snap(0, 90.0)]);
        h.record(&[snap(60, 89.0)]);
        assert!(h.burn_per_hour(&key("p", "5h")).is_none());
    }

    #[test]
    fn a_steady_drain_reports_its_rate() {
        let mut h = History::default();
        for i in 0..7 {
            h.record(&[snap(i * 600, 100.0 - i as f64 * 5.0)]); // 5% per 10 min
        }
        let rate = h.burn_per_hour(&key("p", "5h")).unwrap();
        assert!((rate - 30.0).abs() < 0.5, "expected ~30%/h, got {rate}");
    }

    #[test]
    fn a_refill_voids_the_old_slope() {
        let mut h = History::default();
        for i in 0..7 {
            h.record(&[snap(i * 600, 40.0 - i as f64 * 5.0)]);
        }
        h.record(&[snap(4200, 100.0)]); // window refilled
        assert!(
            h.burn_per_hour(&key("p", "5h")).is_none(),
            "history restarts at the refill"
        );
    }

    #[test]
    fn samples_survive_a_round_trip_through_json() {
        let mut h = History::default();
        for i in 0..5 {
            h.record(&[snap(i * 600, 100.0 - i as f64 * 5.0)]);
        }
        let encoded = serde_json::to_string(&h).unwrap();
        let back: History = serde_json::from_str(&encoded).unwrap();
        let rate = back
            .burn_per_hour(&key("p", "5h"))
            .expect("the trend survives");
        assert!((rate - 30.0).abs() < 0.5, "expected ~30%/h, got {rate}");
    }

    #[test]
    fn only_percentages_and_times_are_stored() {
        // The privacy claim is worth a test: nothing about *what* was being
        // done should be able to reach the file.
        let mut h = History::default();
        h.record(&[snap(0, 42.0)]);
        let encoded = serde_json::to_string(&h).unwrap();
        assert!(encoded.contains("42.0"), "{encoded}");
        assert!(encoded.contains("p|5h"), "{encoded}");
        // provider id and window label are the only strings; no display name,
        // no detail, no counts.
        assert!(!encoded.contains("display"), "{encoded}");
        assert!(!encoded.contains('P'), "no display name leaked: {encoded}");
    }

    #[test]
    fn a_flat_window_projects_nothing() {
        assert!(projection(50.0, 0.0, Some(3600)).is_none());
    }

    #[test]
    fn draining_faster_than_the_reset_says_so() {
        // 50% left, 50%/h, resets in 4h -> empty 3h early
        let t = projection(50.0, 50.0, Some(4 * 3600)).unwrap();
        assert!(t.starts_with("Out "), "{t}");
        assert!(t.contains("3h"), "{t}");
    }

    #[test]
    fn lasting_the_window_reports_what_is_left_at_the_reset() {
        // 80% left, 10%/h, resets in 4h -> ~40% left
        let t = projection(80.0, 10.0, Some(4 * 3600)).unwrap();
        assert!(t.contains("40%"), "{t}");
        assert!(t.len() < 34, "must fit the card without eliding: {t}");
    }
}
