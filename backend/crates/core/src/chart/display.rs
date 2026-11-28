//! Preparing the result of a chart for display (ADR 0022, "Evaluation"):
//! lines are downsampled on the server, as the series views do.

use super::frame::Frame;

/// The points of a line to keep for display, at most `max_points` (at
/// least 2), in order: the minimum and maximum of each bucket of
/// consecutive points, as [`crate::series::Series::downsample`] keeps them.
/// Where points without a value lie between two kept points, the first of
/// them is kept too, so that the line keeps its gap there; buckets are
/// fewer then, to make room. With 2 points, the minimum and maximum take
/// priority over a gap. All points if there are not more than
/// `max_points`.
pub fn downsample_indices(y: &[Option<f64>], max_points: usize) -> Vec<usize> {
    let max_points = max_points.max(2);
    let n = y.len();
    if n <= max_points {
        return (0..n).collect();
    }
    let value = |i: usize| y[i].filter(|v| v.is_finite());
    let gaps = max_points >= 3 && (0..n).any(|i| value(i).is_none());
    // Each bucket keeps up to two extrema and, with gaps, a gap before
    // each of them; the first has none before it: 4 * buckets - 1 points.
    let buckets = if gaps {
        (max_points + 1) / 4
    } else {
        max_points / 2
    };
    let mut out = Vec::with_capacity(max_points);
    let mut last: Option<usize> = None;
    for b in 0..buckets {
        let (start, end) = (b * n / buckets, (b + 1) * n / buckets);
        let mut lo: Option<usize> = None;
        let mut hi: Option<usize> = None;
        for i in start..end {
            let Some(v) = value(i) else { continue };
            if lo.is_none_or(|j| v < value(j).unwrap_or(f64::NAN)) {
                lo = Some(i);
            }
            if hi.is_none_or(|j| v > value(j).unwrap_or(f64::NAN)) {
                hi = Some(i);
            }
        }
        let mut extrema: Vec<usize> = lo.into_iter().chain(hi).collect();
        extrema.sort_unstable();
        extrema.dedup();
        for k in extrema {
            if gaps
                && let Some(p) = last
                && let Some(g) = (p + 1..k).find(|&i| value(i).is_none())
            {
                out.push(g);
            }
            out.push(k);
            last = Some(k);
        }
    }
    out
}

/// Downsamples a frame of a line by its column `y` ([`downsample_indices`]).
/// The kept points keep the values of every column and their attributes.
/// A frame without that column (a match partner that was missing) is a
/// line without values: it keeps no points once it has too many.
pub fn downsample(frame: &mut Frame, y: &str, max_points: usize) {
    if frame.len() <= max_points.max(2) {
        return;
    }
    let keep = match frame.column_values(y) {
        Some(values) => downsample_indices(values, max_points),
        None => Vec::new(),
    };
    frame.select_points(&keep);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::frame::{Attributes, Column};

    #[test]
    fn short_lines_stay() {
        assert_eq!(
            downsample_indices(&[Some(1.0), None, Some(2.0)], 3),
            [0, 1, 2]
        );
        assert_eq!(downsample_indices(&[Some(1.0)], 2), [0]);
        assert!(downsample_indices(&[], 2).is_empty());
    }

    #[test]
    fn keeps_minimum_and_maximum_in_order() {
        let y: Vec<Option<f64>> = [5.0, 1.0, 9.0, 3.0, 4.0, 8.0, 0.0, 2.0]
            .into_iter()
            .map(Some)
            .collect();
        // Two buckets of four points.
        assert_eq!(downsample_indices(&y, 4), [1, 2, 5, 6]);
    }

    #[test]
    fn the_limit_holds_for_two_points_with_a_gap() {
        // Minimum and maximum take priority.
        let y = [Some(1.0), None, Some(2.0), Some(0.5)];
        assert_eq!(downsample_indices(&y, 2), [2, 3]);
        assert!(downsample_indices(&[None, None, None], 2).is_empty());
    }

    #[test]
    fn a_gap_between_kept_points_stays_a_gap() {
        // The gap between 1 and 3 is kept, not the one before the line.
        assert_eq!(
            downsample_indices(&[None, Some(1.0), None, Some(3.0)], 3),
            [1, 2, 3]
        );
    }

    #[test]
    fn no_kept_segment_crosses_a_missing_value() {
        // A small deterministic generator: values with about a fifth missing.
        let mut seed: u64 = 7;
        let mut next = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            seed >> 33
        };
        for _ in 0..300 {
            let n = 1 + (next() % 200) as usize;
            let y: Vec<Option<f64>> = (0..n)
                .map(|_| (next() % 5 != 0).then(|| (next() % 100) as f64))
                .collect();
            let max = 2 + (next() % 20) as usize;
            let keep = downsample_indices(&y, max);
            assert!(keep.len() <= max, "{} > {max}", keep.len());
            assert!(keep.windows(2).all(|w| w[0] < w[1]));
            if max >= 3 {
                for w in keep.windows(2) {
                    if y[w[0]].is_some() && y[w[1]].is_some() {
                        assert!(
                            (w[0] + 1..w[1]).all(|i| y[i].is_some()),
                            "{w:?} crosses a gap in {y:?} (max {max})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn keeps_gaps_and_never_exceeds_the_limit() {
        let mut y: Vec<Option<f64>> = (0..1000).map(|i| Some(f64::from(i % 7))).collect();
        y[500] = None;
        let keep = downsample_indices(&y, 30);
        assert!(keep.len() <= 30, "{}", keep.len());
        assert!(keep.contains(&500));
        assert!(keep.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn downsampling_a_frame_moves_every_column() {
        let n = 100;
        let mut frame = Frame::new(
            Attributes::new(),
            vec![Column::x("s", "time"), Column::new("y", "ms")],
            vec![
                (0..n).map(|i| Some(f64::from(i))).collect(),
                (0..n).map(|i| Some(f64::from(i * i % 13))).collect(),
            ],
        );
        frame.point_attrs = Some(vec![Attributes::new(); n as usize]);
        downsample(&mut frame, "y", 10);
        assert!(frame.len() <= 10);
        frame.check().unwrap();
        // x still matches y point by point.
        for (x, y) in frame.values[0].iter().zip(&frame.values[1]) {
            let x = x.unwrap();
            assert_eq!(y.unwrap(), (x * x) % 13.0);
        }
    }

    #[test]
    fn a_frame_without_the_column_has_no_line_to_keep() {
        let mut frame = Frame::new(
            Attributes::new(),
            vec![Column::new("mean", "ms")],
            vec![(0..10).map(|i| Some(f64::from(i))).collect()],
        );
        let short = Frame {
            values: vec![vec![Some(1.0)]],
            ..frame.clone()
        };
        downsample(&mut frame, "y", 2);
        assert!(frame.is_empty());
        frame.check().unwrap();
        // Within the limit, it stays as it is.
        let mut kept = short.clone();
        downsample(&mut kept, "y", 2);
        assert_eq!(kept, short);
    }
}
