//! Series operations (FR-17, FR-19): interpolation, reductions, transforms,
//! resampling onto a common grid, aggregation across samples and
//! downsampling for display.
//!
//! A series is a list of (x, y) points with non-decreasing x. x may repeat;
//! a repeated x is a jump, and the value at that x is the one of its last
//! point. Points are stored as f32 and evaluated as f64.
//!
//! Intermediate series may contain non-finite y values (e.g. `log10` of a
//! negative value). Such points count as missing: reductions skip them, and
//! [`Series::finite`] drops them before a series is returned to a client.

use std::fmt;

/// How values between points are obtained (FR-17).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Interp {
    /// Straight line between neighbouring points; no value outside the
    /// series.
    #[default]
    Linear,
    /// A value is held until the next point and beyond the last one.
    Step,
}

impl Interp {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "linear" => Some(Self::Linear),
            "step" => Some(Self::Step),
            _ => None,
        }
    }
}

impl fmt::Display for Interp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Linear => "linear",
            Self::Step => "step",
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Series {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
}

/// Reductions over the points of a series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointReduction {
    Min,
    Max,
    Mean,
}

impl Series {
    /// A series from x and y of equal length. x must not decrease.
    pub fn new(x: Vec<f64>, y: Vec<f64>) -> Self {
        debug_assert_eq!(x.len(), y.len());
        Self { x, y }
    }

    pub fn from_f32(x: &[f32], y: &[f32]) -> Self {
        Self::new(
            x.iter().map(|v| f64::from(*v)).collect(),
            y.iter().map(|v| f64::from(*v)).collect(),
        )
    }

    pub fn len(&self) -> usize {
        self.x.len()
    }

    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }

    /// True if x never decreases and all x are finite.
    pub fn is_ordered(&self) -> bool {
        self.x.iter().all(|x| x.is_finite()) && self.x.windows(2).all(|w| w[0] <= w[1])
    }

    /// The series without points whose y is not finite.
    pub fn finite(&self) -> Series {
        let (x, y) = self
            .x
            .iter()
            .zip(&self.y)
            .filter(|(_, y)| y.is_finite())
            .map(|(x, y)| (*x, *y))
            .unzip();
        Series { x, y }
    }

    /// Applies `f` to every y.
    pub fn map(&self, f: impl Fn(f64) -> f64) -> Series {
        Series::new(self.x.clone(), self.y.iter().map(|y| f(*y)).collect())
    }

    /// Converts x with a factor (unit conversion).
    pub fn scale_x(&self, factor: f64) -> Series {
        if factor == 1.0 {
            return self.clone();
        }
        Series::new(self.x.iter().map(|x| x * factor).collect(), self.y.clone())
    }

    /// Combines two series point by point. Both must have the same x values
    /// (FR-17 pointwise math); `None` otherwise.
    pub fn zip(&self, other: &Series, f: impl Fn(f64, f64) -> f64) -> Option<Series> {
        if self.x.len() != other.x.len()
            || !self.x.iter().zip(&other.x).all(|(a, b)| same_x(*a, *b))
        {
            return None;
        }
        Some(Series::new(
            self.x.clone(),
            self.y
                .iter()
                .zip(&other.y)
                .map(|(a, b)| f(*a, *b))
                .collect(),
        ))
    }

    /// Index of the last point with x <= `at`, if any.
    fn last_at_or_before(&self, at: f64) -> Option<usize> {
        self.x.partition_point(|x| *x <= at).checked_sub(1)
    }

    /// The value at `at`. Before the first point there is no value. After
    /// the last point only step interpolation has one.
    pub fn value_at(&self, at: f64, interp: Interp) -> Option<f64> {
        let i = self.last_at_or_before(at)?;
        let v = if self.x[i] == at || interp == Interp::Step {
            self.y[i]
        } else {
            let j = i + 1;
            if j >= self.len() {
                return None;
            }
            let t = (at - self.x[i]) / (self.x[j] - self.x[i]);
            self.y[i] + t * (self.y[j] - self.y[i])
        };
        v.is_finite().then_some(v)
    }

    /// Min, max or mean of the finite points with x in [from, to].
    pub fn reduce(&self, op: PointReduction, from: Option<f64>, to: Option<f64>) -> Option<f64> {
        let values = self
            .x
            .iter()
            .zip(&self.y)
            .filter(|(x, y)| {
                y.is_finite() && from.is_none_or(|f| **x >= f) && to.is_none_or(|t| **x <= t)
            })
            .map(|(_, y)| *y);
        match op {
            PointReduction::Min => values.reduce(f64::min),
            PointReduction::Max => values.reduce(f64::max),
            PointReduction::Mean => {
                let (sum, n) = values.fold((0.0, 0usize), |(s, n), y| (s + y, n + 1));
                (n > 0).then(|| sum / n as f64)
            }
        }
    }

    /// Area under the curve over [from, to] (defaults: the series' extent).
    /// Linear integrates trapezoids and stops at the last point; step holds
    /// each value until the next point and the last one up to `to`.
    /// Segments with a non-finite end are skipped.
    pub fn auc(&self, from: Option<f64>, to: Option<f64>, interp: Interp) -> Option<f64> {
        let (first, last) = (*self.x.first()?, *self.x.last()?);
        let lo = from.map_or(first, |f| f.max(first));
        let hi = match interp {
            Interp::Linear => to.map_or(last, |t| t.min(last)),
            Interp::Step => to.unwrap_or(last),
        };
        if lo.partial_cmp(&hi).is_none_or(|o| o.is_gt()) {
            return None;
        }
        let mut sum = 0.0;
        for i in 0..self.len() {
            let a = self.x[i];
            let (b, yb) = match self.x.get(i + 1) {
                Some(b) => (*b, self.y[i + 1]),
                None if interp == Interp::Step => (hi.max(a), self.y[i]),
                None => break,
            };
            let (ca, cb) = (a.max(lo), b.min(hi));
            if cb <= ca {
                continue;
            }
            let ya = self.y[i];
            let contribution = match interp {
                Interp::Step => ya * (cb - ca),
                Interp::Linear => {
                    let at = |x: f64| ya + (yb - ya) * (x - a) / (b - a);
                    0.5 * (at(ca) + at(cb)) * (cb - ca)
                }
            };
            if contribution.is_finite() {
                sum += contribution;
            }
        }
        Some(sum)
    }

    /// The first x at or after `from` (default: the first point) at which y
    /// reaches `threshold` (y >= threshold). `None` if it never does
    /// (FR-12a).
    pub fn time_to(&self, threshold: f64, from: Option<f64>, interp: Interp) -> Option<f64> {
        let first = *self.x.first()?;
        let start = from.map_or(first, |f| f.max(first));
        if self.value_at(start, interp).is_some_and(|v| v >= threshold) {
            return Some(start);
        }
        for i in 0..self.len().saturating_sub(1) {
            let (a, b) = (self.x[i], self.x[i + 1]);
            let yb = self.y[i + 1];
            if b < start || !yb.is_finite() || yb < threshold {
                continue;
            }
            if interp == Interp::Step || b == a {
                return Some(b.max(start));
            }
            let ya = self.y[i];
            if !ya.is_finite() {
                return Some(b);
            }
            let a2 = a.max(start);
            let ya2 = ya + (yb - ya) * (a2 - a) / (b - a);
            if ya2 >= threshold {
                return Some(a2);
            }
            return Some(a2 + (threshold - ya2) / (yb - ya2) * (b - a2));
        }
        None
    }

    /// Derivative: for each pair of points with increasing x, the slope at
    /// the later x. Pairs with equal x are skipped.
    pub fn rate(&self) -> Series {
        let mut out = Series::default();
        for i in 1..self.len() {
            let dx = self.x[i] - self.x[i - 1];
            if dx > 0.0 {
                out.x.push(self.x[i]);
                out.y.push((self.y[i] - self.y[i - 1]) / dx);
            }
        }
        out
    }

    /// Running sum of y. Missing points keep the sum and stay missing.
    pub fn cumsum(&self) -> Series {
        let mut sum = 0.0;
        let y = self
            .y
            .iter()
            .map(|y| {
                if y.is_finite() {
                    sum += y;
                    sum
                } else {
                    f64::NAN
                }
            })
            .collect();
        Series::new(self.x.clone(), y)
    }

    /// Trailing moving average: the mean of the points with x in
    /// (x - window, x]. `window` must be positive.
    pub fn moving_avg(&self, window: f64) -> Option<Series> {
        if window.is_nan() || window <= 0.0 {
            return None;
        }
        let mut out = Vec::with_capacity(self.len());
        let (mut start, mut sum, mut n) = (0usize, 0.0, 0usize);
        for i in 0..self.len() {
            if self.y[i].is_finite() {
                sum += self.y[i];
                n += 1;
            }
            while self.x[start] <= self.x[i] - window {
                if self.y[start].is_finite() {
                    sum -= self.y[start];
                    n -= 1;
                }
                start += 1;
            }
            out.push(if n > 0 { sum / n as f64 } else { f64::NAN });
        }
        Some(Series::new(self.x.clone(), out))
    }

    /// log10 of x; points with x <= 0 are dropped.
    pub fn log10_x(&self) -> Series {
        let (x, y) = self
            .x
            .iter()
            .zip(&self.y)
            .filter(|(x, _)| **x > 0.0)
            .map(|(x, y)| (x.log10(), *y))
            .unzip();
        Series { x, y }
    }

    /// Re-keys the series by another series of the same sample: each point
    /// (x, y) becomes (by(x), y), e.g. quality over time and bytes over time
    /// give quality over bytes. Points where `by` has no value are dropped.
    /// Fails if the new x would decrease.
    pub fn rekey(&self, by: &Series, interp: Interp) -> Result<Series, String> {
        let mut out = Series::default();
        for (x, y) in self.x.iter().zip(&self.y) {
            if let Some(nx) = by.value_at(*x, interp) {
                if out.x.last().is_some_and(|last| nx < *last) {
                    return Err("the series used as new x decreases".to_owned());
                }
                out.x.push(nx);
                out.y.push(*y);
            }
        }
        Ok(out)
    }

    /// Values on a grid (FR-19).
    pub fn resample(&self, grid: &[f64], interp: Interp) -> Vec<Option<f64>> {
        grid.iter().map(|x| self.value_at(*x, interp)).collect()
    }

    /// Reduces the series to at most `max_points` for display, keeping the
    /// minimum and maximum of each bucket of consecutive points, in order.
    pub fn downsample(&self, max_points: usize) -> Series {
        let max_points = max_points.max(2);
        if self.len() <= max_points {
            return self.clone();
        }
        let buckets = max_points / 2;
        let n = self.len();
        let mut out = Series::default();
        for b in 0..buckets {
            let (start, end) = (b * n / buckets, (b + 1) * n / buckets);
            let mut lo: Option<usize> = None;
            let mut hi: Option<usize> = None;
            for i in start..end {
                if !self.y[i].is_finite() {
                    continue;
                }
                if lo.is_none_or(|j| self.y[i] < self.y[j]) {
                    lo = Some(i);
                }
                if hi.is_none_or(|j| self.y[i] > self.y[j]) {
                    hi = Some(i);
                }
            }
            let mut keep: Vec<usize> = lo.into_iter().chain(hi).collect();
            keep.sort_unstable();
            keep.dedup();
            for i in keep {
                out.x.push(self.x[i]);
                out.y.push(self.y[i]);
            }
        }
        out
    }
}

/// x values compare equal if they are the same up to rounding of a unit
/// conversion.
fn same_x(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= 1e-9 * a.abs().max(b.abs())
}

/// `points` evenly spaced values from `from` to `to` (both included).
pub fn grid(from: f64, to: f64, points: usize) -> Vec<f64> {
    match points {
        0 => vec![],
        1 => vec![from],
        _ if from == to => vec![from; points],
        _ => {
            let (lo, hi) = (from.min(to), from.max(to));
            let last = (points - 1) as f64;
            // `to - from` can overflow; then work on the halves, which
            // only loses precision below the smallest normal numbers.
            let s = if (to - from).is_finite() { 1.0 } else { 0.5 };
            let (a, d) = (from * s, to * s - from * s);
            // `d * i / last` makes every interior point that is exactly
            // representable exact (a step at 29 on 0..100 stays at 29);
            // where `d * last` overflows, `d * (i / last)` instead. One
            // formula for the whole grid, and every step of it rounds
            // monotonically, so the grid never runs backward.
            let product = (d * last).is_finite();
            (0..points)
                .map(|i| {
                    // The ends exactly, and never beyond them.
                    if i == 0 {
                        return from;
                    }
                    if i == points - 1 {
                        return to;
                    }
                    let i = i as f64;
                    let step = if product {
                        d * i / last
                    } else {
                        d * (i / last)
                    };
                    ((a + step) / s).clamp(lo, hi)
                })
                .collect()
        }
    }
}

/// The x range covered by any of the series, or `None` if all are empty.
pub fn extent<'a>(series: impl IntoIterator<Item = &'a Series>) -> Option<(f64, f64)> {
    series
        .into_iter()
        .filter_map(|s| Some((*s.x.first()?, *s.x.last()?)))
        .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
}

/// Aggregate of several resampled series on one grid: per grid point the
/// number of series with a value and the requested percentiles.
#[derive(Debug, Clone, PartialEq)]
pub struct Band {
    pub n: Vec<usize>,
    /// One vector per requested percentile, each with one value per grid
    /// point.
    pub percentiles: Vec<Vec<Option<f64>>>,
}

/// Aggregates resampled series across samples (FR-19), e.g. the median
/// series with a percentile band.
pub fn aggregate_across(rows: &[Vec<Option<f64>>], percentiles: &[f64]) -> Band {
    let len = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut band = Band {
        n: Vec::with_capacity(len),
        percentiles: vec![Vec::with_capacity(len); percentiles.len()],
    };
    let mut column = Vec::with_capacity(rows.len());
    for i in 0..len {
        column.clear();
        column.extend(rows.iter().filter_map(|r| r.get(i).copied().flatten()));
        column.retain(|v| v.is_finite());
        column.sort_by(f64::total_cmp);
        band.n.push(column.len());
        for (k, p) in percentiles.iter().enumerate() {
            band.percentiles[k].push(crate::stats::percentile_sorted(&column, *p));
        }
    }
    band
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn s(points: &[(f64, f64)]) -> Series {
        Series::new(
            points.iter().map(|p| p.0).collect(),
            points.iter().map(|p| p.1).collect(),
        )
    }

    #[test]
    fn interpolation() {
        let a = s(&[(0.0, 0.0), (1.0, 10.0), (1.0, 20.0), (3.0, 40.0)]);
        assert_eq!(a.value_at(-0.1, Interp::Linear), None);
        assert_eq!(a.value_at(-0.1, Interp::Step), None);
        assert_eq!(a.value_at(0.5, Interp::Linear), Some(5.0));
        assert_eq!(a.value_at(0.5, Interp::Step), Some(0.0));
        // A repeated x is a jump: the last point at that x wins.
        assert_eq!(a.value_at(1.0, Interp::Linear), Some(20.0));
        assert_eq!(a.value_at(2.0, Interp::Linear), Some(30.0));
        assert_eq!(a.value_at(3.0, Interp::Linear), Some(40.0));
        assert_eq!(a.value_at(4.0, Interp::Linear), None);
        assert_eq!(a.value_at(4.0, Interp::Step), Some(40.0));
    }

    #[test]
    fn reductions_use_points_in_range() {
        let a = s(&[(0.0, 4.0), (1.0, 2.0), (2.0, f64::NAN), (3.0, 9.0)]);
        assert_eq!(a.reduce(PointReduction::Min, None, None), Some(2.0));
        assert_eq!(a.reduce(PointReduction::Max, None, None), Some(9.0));
        assert_eq!(a.reduce(PointReduction::Mean, None, None), Some(5.0));
        assert_eq!(
            a.reduce(PointReduction::Max, Some(0.5), Some(2.5)),
            Some(2.0)
        );
        assert_eq!(a.reduce(PointReduction::Max, Some(5.0), None), None);
    }

    #[test]
    fn area_under_curve() {
        let a = s(&[(0.0, 0.0), (2.0, 2.0), (4.0, 2.0)]);
        assert_eq!(a.auc(None, None, Interp::Linear), Some(6.0));
        assert_eq!(a.auc(Some(1.0), Some(3.0), Interp::Linear), Some(3.5));
        assert_eq!(a.auc(None, None, Interp::Step), Some(4.0));
        // Step holds the last value up to `to`; linear stops at the last point.
        assert_eq!(a.auc(None, Some(6.0), Interp::Step), Some(8.0));
        assert_eq!(a.auc(None, Some(6.0), Interp::Linear), Some(6.0));
        assert_eq!(a.auc(Some(3.0), Some(1.0), Interp::Linear), None);
        assert_eq!(Series::default().auc(None, None, Interp::Linear), None);
        // Jumps have no width.
        let b = s(&[(0.0, 1.0), (1.0, 1.0), (1.0, 3.0), (2.0, 3.0)]);
        assert_eq!(b.auc(None, None, Interp::Linear), Some(4.0));
    }

    #[test]
    fn time_to_threshold() {
        let a = s(&[(0.0, 0.0), (1.0, 0.5), (2.0, 1.0)]);
        assert_eq!(a.time_to(0.9, None, Interp::Linear), Some(1.8));
        assert_eq!(a.time_to(0.9, None, Interp::Step), Some(2.0));
        assert_eq!(a.time_to(0.0, None, Interp::Linear), Some(0.0));
        assert_eq!(a.time_to(1.1, None, Interp::Linear), None, "never reached");
        // Starting at an offset inside a segment.
        assert_eq!(a.time_to(0.25, Some(0.5), Interp::Linear), Some(0.5));
        assert_eq!(a.time_to(0.75, Some(1.5), Interp::Linear), Some(1.5));
        let b = s(&[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0), (3.0, 1.0)]);
        assert_eq!(b.time_to(1.0, Some(1.5), Interp::Linear), Some(3.0));
        assert_eq!(b.time_to(0.5, Some(1.2), Interp::Linear), Some(1.2));
        // A jump reaches the threshold at the jump.
        let c = s(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)]);
        assert_eq!(c.time_to(0.5, None, Interp::Linear), Some(1.0));
    }

    #[test]
    fn transforms() {
        let a = s(&[(0.0, 0.0), (1.0, 2.0), (1.0, 3.0), (3.0, 7.0)]);
        assert_eq!(a.rate(), s(&[(1.0, 2.0), (3.0, 2.0)]));
        assert_eq!(a.cumsum().y, vec![0.0, 2.0, 5.0, 12.0]);
        let m = a.moving_avg(1.5).unwrap();
        assert_eq!(m.y, vec![0.0, 1.0, 5.0 / 3.0, 7.0]);
        assert_eq!(a.moving_avg(0.0), None);
        let l = s(&[(0.0, 1.0), (10.0, 2.0), (100.0, 3.0)]).log10_x();
        assert_eq!(l, s(&[(1.0, 2.0), (2.0, 3.0)]));
    }

    #[test]
    fn rekey_by_another_series() {
        let quality = s(&[(0.0, 0.1), (1.0, 0.5), (2.0, 0.9)]);
        let bytes = s(&[(0.5, 100.0), (1.5, 300.0)]);
        let r = quality.rekey(&bytes, Interp::Linear).unwrap();
        assert_eq!(r, s(&[(200.0, 0.5)]));
        let r = quality.rekey(&bytes, Interp::Step).unwrap();
        assert_eq!(r, s(&[(100.0, 0.5), (300.0, 0.9)]));
        let falling = s(&[(0.0, 5.0), (2.0, 1.0)]);
        assert!(quality.rekey(&falling, Interp::Linear).is_err());
    }

    #[test]
    fn pointwise_needs_equal_x() {
        let a = s(&[(0.0, 1.0), (1.0, 2.0)]);
        let b = s(&[(0.0, 10.0), (1.0, 20.0)]);
        assert_eq!(a.zip(&b, |p, q| p + q).unwrap().y, vec![11.0, 22.0]);
        let c = s(&[(0.0, 10.0), (2.0, 20.0)]);
        assert!(a.zip(&c, |p, q| p + q).is_none());
        // Equal after a unit conversion (ms -> s).
        let d = s(&[(0.0, 1.0), (1000.0, 1.0)]).scale_x(1e-3);
        assert!(a.zip(&d, |p, q| p * q).is_some());
    }

    #[test]
    fn resample_and_aggregate() {
        let a = s(&[(0.0, 0.0), (2.0, 2.0)]);
        let b = s(&[(0.0, 2.0), (1.0, 4.0)]);
        // The ends are exact, also where rounding would miss them.
        let g = grid(0.2, 1.0, 4);
        assert_eq!((g[0], g[3]), (0.2, 1.0));
        assert!(g[1] > 0.2 && g[2] < 1.0 && g[1] < g[2]);
        let g = grid(0.0, 1e308, 3);
        assert!(g.iter().all(|x| x.is_finite()) && g[2] == 1e308);
        assert!(grid(100.0, 100.0, 200).iter().all(|x| *x == 100.0));
        let g = grid(-1e308, 1e308, 3);
        assert_eq!(g, [-1e308, 0.0, 1e308]);
        // Representable interior points are exact: a step at 29 stays there.
        let g = grid(0.0, 100.0, 101);
        assert!(g.iter().enumerate().all(|(i, x)| *x == i as f64));
        let step = s(&[(0.0, 0.0), (29.0, 1.0), (100.0, 1.0)]);
        assert_eq!(step.resample(&g, Interp::Step)[29], Some(1.0));
        // The grid never runs backward, also on a range of a few ulps and
        // where the width or its multiples overflow.
        for (from, to, n) in [
            (10.0, 10.000000000000002, 8),
            (0.1, 0.7, 1000),
            (-f64::MAX, f64::MAX, 1001),
            (0.0, f64::MAX, 100_000),
            (1.0, 0.0, 7),
        ] {
            let g = grid(from, to, n);
            assert_eq!((g[0], g[n - 1]), (from, to));
            let sign = if to >= from { 1.0 } else { -1.0 };
            assert!(
                g.windows(2).all(|w| sign * (w[1] - w[0]) >= 0.0),
                "{from}..{to} ({n}): {g:?}"
            );
        }
        let g = grid(0.0, 2.0, 3);
        assert_eq!(g, vec![0.0, 1.0, 2.0]);
        let rows = vec![
            a.resample(&g, Interp::Linear),
            b.resample(&g, Interp::Linear),
        ];
        assert_eq!(rows[1], vec![Some(2.0), Some(4.0), None]);
        let band = aggregate_across(&rows, &[50.0, 100.0]);
        assert_eq!(band.n, vec![2, 2, 1]);
        assert_eq!(band.percentiles[0], vec![Some(1.0), Some(2.5), Some(2.0)]);
        assert_eq!(band.percentiles[1], vec![Some(2.0), Some(4.0), Some(2.0)]);
        assert_eq!(extent([&a, &b, &Series::default()]), Some((0.0, 2.0)));
    }

    #[test]
    fn downsample_keeps_extremes() {
        let a = Series::new(
            (0..100).map(f64::from).collect(),
            (0..100).map(|i| f64::from((i * 37) % 101)).collect(),
        );
        let d = a.downsample(10);
        assert!(d.len() <= 10);
        assert_eq!(a.downsample(1000), a);
    }

    proptest! {
        #[test]
        fn downsample_preserves_min_max_and_order(
            ys in proptest::collection::vec(-1e6f64..1e6, 1..500),
            max_points in 2usize..100,
        ) {
            let xs: Vec<f64> = (0..ys.len()).map(|i| i as f64).collect();
            let a = Series::new(xs, ys.clone());
            let d = a.downsample(max_points);
            prop_assert!(d.len() <= max_points.max(2) || d.len() == a.len());
            prop_assert!(d.is_ordered());
            let min = ys.iter().copied().fold(f64::INFINITY, f64::min);
            let max = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            prop_assert!(d.y.contains(&min));
            prop_assert!(d.y.contains(&max));
        }

        #[test]
        fn time_to_result_reaches_threshold(
            ys in proptest::collection::vec(0f64..1.0, 2..50),
            threshold in 0f64..1.0,
        ) {
            let xs: Vec<f64> = (0..ys.len()).map(|i| i as f64).collect();
            let a = Series::new(xs, ys);
            if let Some(t) = a.time_to(threshold, None, Interp::Linear) {
                let v = a.value_at(t, Interp::Linear).unwrap();
                prop_assert!(v >= threshold - 1e-9, "value {} at {} < {}", v, t, threshold);
            } else {
                prop_assert!(a.y.iter().all(|y| *y < threshold));
            }
        }
    }
}
