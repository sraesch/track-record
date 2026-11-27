//! Statistics across samples (FR-12, FR-12a).
//!
//! Values are `Option<f64>`: `None` is "no value" (e.g. a threshold that was
//! never reached). Aggregates use the samples that have a value and count
//! the ones that have none. Non-finite values count as no value.

/// Aggregate of the values of one group of samples.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    /// Samples with a value.
    pub n: usize,
    /// Samples without a value (FR-12a).
    pub n_no_value: usize,
    pub mean: Option<f64>,
    pub median: Option<f64>,
    /// Sample standard deviation (n - 1); `None` for fewer than 2 values.
    pub stddev: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// `(p, value)` for each requested percentile p in [0, 100].
    pub percentiles: Vec<(f64, Option<f64>)>,
    /// Confidence interval of the mean (Student's t); `None` for fewer than
    /// 2 values.
    pub ci: Option<(f64, f64)>,
}

/// Summarizes values. `confidence` is the level of the interval, e.g. 0.95.
pub fn summarize(values: &[Option<f64>], percentiles: &[f64], confidence: f64) -> Summary {
    let mut v: Vec<f64> = finite(values).collect();
    let n_no_value = values.len() - v.len();
    v.sort_by(f64::total_cmp);
    let n = v.len();
    let mean = (n > 0).then(|| mean_sorted(&v));
    let stddev = (n > 1).then(|| {
        let m = mean.unwrap_or(0.0);
        // Scaled by the largest deviation, so that squares of large values
        // do not overflow.
        let scale = v.iter().map(|x| (x - m).abs()).fold(0.0f64, f64::max);
        if scale == 0.0 {
            return 0.0;
        }
        let sum = v.iter().map(|x| ((x - m) / scale).powi(2)).sum::<f64>();
        scale * (sum / (n - 1) as f64).sqrt()
    });
    let ci = match (mean, stddev) {
        (Some(m), Some(s)) => {
            let t = t_quantile(0.5 + confidence / 2.0, (n - 1) as f64);
            let half = t * s / (n as f64).sqrt();
            Some((m - half, m + half))
        }
        _ => None,
    };
    Summary {
        n,
        n_no_value,
        mean,
        median: percentile_sorted(&v, 50.0),
        stddev,
        min: v.first().copied(),
        max: v.last().copied(),
        percentiles: percentiles
            .iter()
            .map(|p| (*p, percentile_sorted(&v, *p)))
            .collect(),
        ci,
    }
}

/// Mean with Kahan-Neumaier summation, so long runs of samples stay exact.
fn mean_sorted(v: &[f64]) -> f64 {
    let n = v.len() as f64;
    let sum = neumaier(v.iter().copied());
    if sum.is_finite() {
        return sum / n;
    }
    // The sum overflows; the mean of finite values does not.
    let scale = largest(v);
    scale * (neumaier(v.iter().map(|x| x / scale)) / n)
}

/// The largest magnitude of the values.
fn largest(v: &[f64]) -> f64 {
    v.iter().map(|x| x.abs()).fold(0.0, f64::max)
}

/// Sum with Kahan-Neumaier summation.
fn neumaier(values: impl IntoIterator<Item = f64>) -> f64 {
    let (mut sum, mut c) = (0.0f64, 0.0f64);
    for x in values {
        let t = sum + x;
        if sum.abs() >= x.abs() {
            c += (sum - t) + x;
        } else {
            c += (x - t) + sum;
        }
        sum = t;
    }
    sum + c
}

/// The values that are values: finite ones.
fn finite(values: &[Option<f64>]) -> impl DoubleEndedIterator<Item = f64> + '_ {
    values.iter().filter_map(|x| x.filter(|x| x.is_finite()))
}

/// Sum of the values; no value if there are none (FR-12a).
pub fn sum(values: &[Option<f64>]) -> Option<f64> {
    let v: Vec<f64> = finite(values).collect();
    if v.is_empty() {
        return None;
    }
    let sum = neumaier(v.iter().copied());
    if sum.is_finite() {
        return Some(sum);
    }
    // An intermediate sum overflowed; the total may not.
    let scale = largest(&v);
    Some(scale * neumaier(v.iter().map(|x| x / scale))).filter(|s| s.is_finite())
}

/// Geometric mean of the values: no value if there are none or one is
/// negative, 0 if one is 0.
pub fn geomean(values: &[Option<f64>]) -> Option<f64> {
    let v: Vec<f64> = finite(values).collect();
    if v.is_empty() || v.iter().any(|x| *x < 0.0) {
        return None;
    }
    if v.contains(&0.0) {
        return Some(0.0);
    }
    let mean_ln = neumaier(v.iter().map(|x| x.ln())) / v.len() as f64;
    Some(mean_ln.exp()).filter(|g| g.is_finite())
}

/// The first value, skipping no value.
pub fn first(values: &[Option<f64>]) -> Option<f64> {
    finite(values).next()
}

/// The last value, skipping no value.
pub fn last(values: &[Option<f64>]) -> Option<f64> {
    finite(values).next_back()
}

/// Percentile of sorted values with linear interpolation between closest
/// ranks (the common "type 7" definition). `p` is in [0, 100].
pub fn percentile_sorted(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() || !(0.0..=100.0).contains(&p) {
        return None;
    }
    let rank = p / 100.0 * (sorted.len() - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    Some(sorted[lo] + (sorted[hi] - sorted[lo]) * (rank - lo as f64))
}

/// Percentile of unsorted values that may contain no-value entries.
pub fn percentile(values: &[f64], p: f64) -> Option<f64> {
    let mut v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    v.sort_by(f64::total_cmp);
    percentile_sorted(&v, p)
}

/// Quantile of Student's t distribution with `df` degrees of freedom.
pub fn t_quantile(p: f64, df: f64) -> f64 {
    if !(0.0..1.0).contains(&p) || p == 0.0 || df <= 0.0 {
        return f64::NAN;
    }
    if p == 0.5 {
        return 0.0;
    }
    if p < 0.5 {
        return -t_quantile(1.0 - p, df);
    }
    // Bisection on the CDF; the CDF is monotonic and cheap to evaluate.
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    while t_cdf(hi, df) < p {
        hi *= 2.0;
        if hi > 1e12 {
            return f64::INFINITY;
        }
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if t_cdf(mid, df) < p {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi - lo <= 1e-13 * hi.max(1.0) {
            break;
        }
    }
    0.5 * (lo + hi)
}

/// CDF of Student's t distribution.
pub fn t_cdf(t: f64, df: f64) -> f64 {
    let x = df / (df + t * t);
    let tail = 0.5 * incomplete_beta(df / 2.0, 0.5, x);
    if t >= 0.0 { 1.0 - tail } else { tail }
}

fn ln_gamma(x: f64) -> f64 {
    // Lanczos approximation (g = 7, n = 9).
    const C: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        let pi = std::f64::consts::PI;
        return (pi / (pi * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = C[0];
    let t = x + 7.5;
    for (i, c) in C.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// Regularized incomplete beta function I_x(a, b).
fn incomplete_beta(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let ln_front = ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln();
    if x < (a + 1.0) / (a + b + 2.0) {
        ln_front.exp() * beta_cf(a, b, x) / a
    } else {
        1.0 - ln_front.exp() * beta_cf(b, a, 1.0 - x) / b
    }
}

/// Continued fraction for the incomplete beta function (modified Lentz).
fn beta_cf(a: f64, b: f64, x: f64) -> f64 {
    const TINY: f64 = 1e-300;
    let (qab, qap, qam) = (a + b, a + 1.0, a - 1.0);
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < TINY {
        d = TINY;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..400 {
        let m = m as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-15 {
            break;
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn t_quantiles_match_tables() {
        for (df, expected) in [
            (1.0, 12.706_204_736),
            (2.0, 4.302_652_730),
            (5.0, 2.570_581_836),
            (10.0, 2.228_138_852),
            (30.0, 2.042_272_456),
            (1000.0, 1.962_339_081),
        ] {
            let t = t_quantile(0.975, df);
            assert!(close(t, expected, 1e-6), "df {df}: {t} != {expected}");
        }
        assert!(close(t_quantile(0.95, 4.0), 2.131_846_786, 1e-6));
        assert!(close(t_quantile(0.025, 10.0), -2.228_138_852, 1e-6));
        assert!(close(t_cdf(0.0, 3.0), 0.5, 1e-12));
    }

    #[test]
    fn summary_of_known_values() {
        let v = [
            Some(2.0),
            Some(4.0),
            Some(4.0),
            Some(4.0),
            Some(5.0),
            Some(5.0),
            Some(7.0),
            Some(9.0),
        ];
        let s = summarize(&v, &[25.0, 90.0], 0.95);
        assert_eq!(s.n, 8);
        assert_eq!(s.n_no_value, 0);
        assert_eq!(s.mean, Some(5.0));
        assert_eq!(s.median, Some(4.5));
        assert_eq!(s.min, Some(2.0));
        assert_eq!(s.max, Some(9.0));
        // Sample variance 32/7.
        assert!(close(s.stddev.unwrap(), (32.0f64 / 7.0).sqrt(), 1e-12));
        assert_eq!(s.percentiles[0], (25.0, Some(4.0)));
        assert!(close(s.percentiles[1].1.unwrap(), 7.6, 1e-12));
        let (lo, hi) = s.ci.unwrap();
        let half = 2.364_624_252 * (32.0f64 / 7.0).sqrt() / 8.0f64.sqrt();
        assert!(close(lo, 5.0 - half, 1e-6) && close(hi, 5.0 + half, 1e-6));
    }

    #[test]
    fn no_value_is_counted_not_aggregated() {
        let s = summarize(&[Some(1.0), None, Some(3.0), Some(f64::NAN)], &[], 0.95);
        assert_eq!((s.n, s.n_no_value), (2, 2));
        assert_eq!(s.mean, Some(2.0));

        let s = summarize(&[None, None], &[50.0], 0.95);
        assert_eq!((s.n, s.n_no_value), (0, 2));
        assert_eq!(s.mean, None);
        assert_eq!(s.percentiles, vec![(50.0, None)]);
        assert_eq!(s.ci, None);

        let s = summarize(&[Some(3.0)], &[], 0.95);
        assert_eq!(
            (s.mean, s.median, s.stddev, s.ci),
            (Some(3.0), Some(3.0), None, None)
        );
    }

    #[test]
    fn large_values_do_not_overflow_the_mean_or_the_stddev() {
        let s = summarize(&[Some(1e308), Some(1e308)], &[], 0.95);
        assert_eq!(s.mean, Some(1e308));
        let s = summarize(&[Some(f64::MAX); 3], &[], 0.95);
        assert_eq!(s.mean, Some(f64::MAX));
        assert_eq!(s.stddev, Some(0.0));
        let s = summarize(&[Some(1e308), Some(-1e308)], &[], 0.95);
        assert_eq!(s.mean, Some(0.0));
        let expected = 1e308 * 2.0f64.sqrt();
        assert!(close(s.stddev.unwrap(), expected, expected * 1e-12));
    }

    #[test]
    fn sums_skip_no_value_and_have_none_without_values() {
        assert_eq!(
            sum(&[Some(1.0), None, Some(2.5), Some(f64::NAN)]),
            Some(3.5)
        );
        assert_eq!(sum(&[None, None]), None);
        assert_eq!(sum(&[]), None);
        assert_eq!(sum(&[Some(0.0)]), Some(0.0));
        // Neumaier keeps the small terms.
        assert_eq!(sum(&[Some(1e16), Some(1.0), Some(-1e16)]), Some(1.0));
        assert_eq!(sum(&[Some(f64::MAX), Some(f64::MAX)]), None);
        // An overflow on the way that a later value cancels.
        assert_eq!(sum(&[Some(1e308), Some(1e308), Some(-1e308)]), Some(1e308));
        assert_eq!(sum(&[Some(1e308), Some(-1e308), Some(1e308)]), Some(1e308));
    }

    #[test]
    fn geomean_has_no_value_for_negatives() {
        assert!(close(
            geomean(&[Some(2.0), Some(8.0), None]).unwrap(),
            4.0,
            1e-12
        ));
        assert_eq!(geomean(&[Some(2.0), Some(0.0)]), Some(0.0));
        assert_eq!(geomean(&[Some(2.0), Some(-1.0)]), None);
        assert_eq!(geomean(&[Some(0.0), Some(-1.0)]), None);
        assert_eq!(geomean(&[None]), None);
        assert_eq!(geomean(&[]), None);
    }

    #[test]
    fn first_and_last_skip_no_value() {
        let v = [None, Some(1.0), Some(f64::INFINITY), Some(2.0), None];
        assert_eq!(first(&v), Some(1.0));
        assert_eq!(last(&v), Some(2.0));
        assert_eq!(first(&[None]), None);
        assert_eq!(last(&[]), None);
    }

    #[test]
    fn percentiles_interpolate() {
        let v = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(percentile_sorted(&v, 0.0), Some(1.0));
        assert_eq!(percentile_sorted(&v, 100.0), Some(4.0));
        assert_eq!(percentile_sorted(&v, 50.0), Some(2.5));
        assert_eq!(percentile_sorted(&v, 101.0), None);
        assert_eq!(percentile(&[4.0, f64::NAN, 1.0], 50.0), Some(2.5));
    }
}
