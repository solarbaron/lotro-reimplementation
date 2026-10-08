// SPDX-License-Identifier: GPL-3.0-or-later
//! Measurement helpers: a fixed-memory histogram for latency percentiles and a
//! least-squares line for memory growth (`test-server.md` §13.3, §13.4).

/// Smallest value the histogram resolves; smaller values fall into the first bucket.
const LOW: f64 = 1.0e-3;
/// Ratio between neighbouring bucket bounds (1 % resolution).
const RATIO: f64 = 1.01;
/// Number of buckets: from [`LOW`] up to `LOW · RATIO^BUCKETS` ≈ 10⁶ (for milliseconds:
/// 1 µs to about 17 minutes).
const BUCKETS: usize = 2_090;

/// A histogram with logarithmic buckets: constant memory whatever the run's length, with
/// percentiles accurate to about 1 %. Values are in any unit (the report uses
/// milliseconds); non-finite and negative values are ignored.
#[derive(Debug, Clone)]
pub struct Histogram {
    buckets: Vec<u64>,
    count: u64,
    sum: f64,
    min: f64,
    max: f64,
}

impl Default for Histogram {
    fn default() -> Self {
        Self::new()
    }
}

impl Histogram {
    /// An empty histogram.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buckets: vec![0; BUCKETS],
            count: 0,
            sum: 0.0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
        }
    }

    fn index(v: f64) -> usize {
        if v <= LOW {
            return 0;
        }
        let i = ((v / LOW).ln() / RATIO.ln()).floor();
        if i.is_finite() && i >= 0.0 {
            (i as usize).min(BUCKETS - 1)
        } else {
            0
        }
    }

    /// Adds a value.
    pub fn record(&mut self, v: f64) {
        if !v.is_finite() || v < 0.0 {
            return;
        }
        self.buckets[Self::index(v)] += 1;
        self.count += 1;
        self.sum += v;
        self.min = self.min.min(v);
        self.max = self.max.max(v);
    }

    /// Adds every value of `other`.
    pub fn merge(&mut self, other: &Self) {
        for (a, b) in self.buckets.iter_mut().zip(&other.buckets) {
            *a += b;
        }
        self.count += other.count;
        self.sum += other.sum;
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
    }

    /// Number of values.
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Largest value, if any.
    #[must_use]
    pub fn max(&self) -> Option<f64> {
        (self.count > 0).then_some(self.max)
    }

    /// Mean value, if any.
    #[must_use]
    pub fn mean(&self) -> Option<f64> {
        (self.count > 0).then(|| self.sum / self.count as f64)
    }

    /// The value below which a fraction `q` (0–1) of the values lie, to about 1 %.
    #[must_use]
    pub fn quantile(&self, q: f64) -> Option<f64> {
        if self.count == 0 || !q.is_finite() {
            return None;
        }
        let rank = ((q.clamp(0.0, 1.0) * self.count as f64).ceil() as u64).max(1);
        let mut seen = 0u64;
        for (i, n) in self.buckets.iter().enumerate() {
            seen += n;
            if seen >= rank {
                if i == BUCKETS - 1 {
                    // The overflow bucket: everything above the range.
                    return Some(self.max);
                }
                let upper = LOW * RATIO.powi(i32::try_from(i + 1).unwrap_or(i32::MAX));
                return Some(upper.clamp(self.min, self.max));
            }
        }
        Some(self.max)
    }

    /// "p50 a / p99 b / max c" with `digits` decimals, or "-" when empty.
    #[must_use]
    pub fn summary(&self, digits: usize) -> String {
        match (self.quantile(0.5), self.quantile(0.99), self.max()) {
            (Some(a), Some(b), Some(c)) => {
                format!("p50 {a:.digits$} / p99 {b:.digits$} / max {c:.digits$}")
            }
            _ => "-".to_owned(),
        }
    }
}

/// Least-squares line through `(x, y)` points: `(slope, intercept)`, or `None` with fewer
/// than two points, no spread in x, or non-finite input.
#[must_use]
pub fn linear_fit(points: &[(f64, f64)]) -> Option<(f64, f64)> {
    if points.len() < 2 {
        return None;
    }
    let n = points.len() as f64;
    let mx = points.iter().map(|p| p.0).sum::<f64>() / n;
    let my = points.iter().map(|p| p.1).sum::<f64>() / n;
    let mut sxx = 0.0;
    let mut sxy = 0.0;
    for &(x, y) in points {
        sxx += (x - mx) * (x - mx);
        sxy += (x - mx) * (y - my);
    }
    if !(sxx.is_finite() && sxy.is_finite()) || sxx <= 0.0 {
        return None;
    }
    let slope = sxy / sxx;
    Some((slope, my - slope * mx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantiles_within_one_percent() {
        let mut h = Histogram::new();
        for i in 1..=10_000 {
            h.record(f64::from(i) / 10.0);
        }
        let p50 = h.quantile(0.5).unwrap();
        assert!((p50 - 500.0).abs() < 500.0 * 0.011, "{p50}");
        let p99 = h.quantile(0.99).unwrap();
        assert!((p99 - 990.0).abs() < 990.0 * 0.011, "{p99}");
        assert_eq!(h.max(), Some(1000.0));
        assert_eq!(h.count(), 10_000);
        assert_eq!(h.quantile(1.0), Some(1000.0));
        assert!(h.quantile(0.0).unwrap() >= 0.1);
    }

    #[test]
    fn odd_values_are_ignored_or_clamped() {
        let mut h = Histogram::new();
        assert_eq!(h.quantile(0.5), None);
        assert_eq!(h.summary(1), "-");
        h.record(f64::NAN);
        h.record(-1.0);
        h.record(f64::INFINITY);
        assert_eq!(h.count(), 0);
        h.record(0.0);
        h.record(1.0e12);
        assert_eq!(h.count(), 2);
        assert_eq!(h.quantile(1.0), Some(1.0e12));
        assert!(h.quantile(0.5).unwrap() < 0.002);
        let mut g = Histogram::new();
        g.record(5.0);
        g.merge(&h);
        assert_eq!(g.count(), 3);
        assert!(g.summary(1).starts_with("p50"));
    }

    #[test]
    fn fit() {
        let pts: Vec<(f64, f64)> = (0..10)
            .map(|i| (f64::from(i), 3.0 * f64::from(i) + 1.0))
            .collect();
        let (s, c) = linear_fit(&pts).unwrap();
        assert!((s - 3.0).abs() < 1e-9 && (c - 1.0).abs() < 1e-9);
        assert_eq!(linear_fit(&pts[..1]), None);
        assert_eq!(linear_fit(&[(1.0, 1.0), (1.0, 2.0)]), None);
        assert_eq!(linear_fit(&[(f64::NAN, 1.0), (1.0, 2.0)]), None);
    }
}
