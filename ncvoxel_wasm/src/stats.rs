//! Continuous-variable `stats.json` (port of theloderma `stats_json.py`).

use serde::{Deserialize, Serialize};

/// Producer subsamples above ~2.14M voxels (the cap observed in the Esri reference output).
const SAMPLE_CAP: u64 = 2_139_904;
const DIST_BINS: usize = 256;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub data_range: [f64; 2],
    pub distribution_range: [f64; 2],
    pub distribution: Vec<f64>,
    pub mean: f64,
    pub sampled_sum: f64,
    pub sampled_count: u64,
    pub sampled_sum_sqr: f64,
    /// Omitted when the variance numerator is 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub std_dev: Option<f64>,
}

impl Stats {
    pub fn empty() -> Self {
        Self {
            data_range: [0.0, 0.0],
            distribution_range: [0.0, 0.0],
            distribution: vec![1.0],
            mean: 0.0,
            sampled_sum: 0.0,
            sampled_count: 0,
            sampled_sum_sqr: 0.0,
            std_dev: None,
        }
    }
}

/// Calls `f` once per valid (decoded, non-fill, non-NaN) voxel, in storage order.
pub type Visit<'a> = &'a dyn Fn(&mut dyn FnMut(f64));

/// Stats for one timestep. `visit` is called up to three times, so the decoded slab never has to
/// be materialised. `shape` is `[nz, ny, nx]`.
pub fn compute(visit: Visit, shape: [usize; 3]) -> Stats {
    let mut n_total = 0u64;
    visit(&mut |_| n_total += 1);
    if n_total == 0 {
        return Stats::empty();
    }
    let subsampled = n_total > SAMPLE_CAP;
    let stride = if subsampled {
        n_total.div_ceil(SAMPLE_CAP)
    } else {
        1
    };

    // Pass 2: moments over the (possibly strided) sample.
    let (mut idx, mut n) = (0u64, 0u64);
    let (mut mn, mut mx) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    visit(&mut |v| {
        if idx % stride == 0 {
            n += 1;
            mn = mn.min(v);
            mx = mx.max(v);
            s1 += v;
            s2 += v * v;
        }
        idx += 1;
    });

    let distribution = if mn == mx {
        vec![1.0]
    } else {
        let mut hist = vec![0u64; DIST_BINS];
        let scale = DIST_BINS as f64 / (mx - mn);
        let mut idx = 0u64;
        visit(&mut |v| {
            if idx % stride == 0 {
                let b = (((v - mn) * scale) as usize).min(DIST_BINS - 1);
                hist[b] += 1;
            }
            idx += 1;
        });
        let total = hist.iter().sum::<u64>() as f64;
        hist.iter()
            .map(|&h| if total == 0.0 { 0.0 } else { h as f64 / total })
            .collect()
    };

    let nf = n as f64;
    let mean = s1 / nf;
    let var_num = s2 - nf * mean * mean;

    let sampled_count = if subsampled {
        n
    } else {
        let [nz, ny, nx] = shape;
        let pad = |k: usize| k + usize::from(k > 32);
        let total = (pad(nx) * pad(ny) * pad(nz)) as u64;
        let fill = (nx * ny * nz) as u64 - n_total;
        total.saturating_sub(fill)
    };

    Stats {
        data_range: [mn, mx],
        distribution_range: [mn, mx],
        distribution,
        mean,
        sampled_sum: s1,
        sampled_count,
        sampled_sum_sqr: s2,
        std_dev: (var_num > 0.0 && n > 1).then(|| (var_num / (nf - 1.0)).sqrt()),
    }
}

/// Combine per-timestep stats into the variable-level aggregate.
pub fn aggregate(per_ts: &[Stats]) -> Stats {
    if per_ts.len() == 1 {
        return per_ts[0].clone();
    }
    let mn = per_ts
        .iter()
        .map(|s| s.data_range[0])
        .fold(f64::INFINITY, f64::min);
    let mx = per_ts
        .iter()
        .map(|s| s.data_range[1])
        .fold(f64::NEG_INFINITY, f64::max);
    let s1: f64 = per_ts.iter().map(|s| s.sampled_sum).sum();
    let s2: f64 = per_ts.iter().map(|s| s.sampled_sum_sqr).sum();
    let n: u64 = per_ts.iter().map(|s| s.sampled_count).sum();
    let nf = n as f64;
    let mean = if n > 0 { s1 / nf } else { 0.0 };
    let var_num = s2 - nf * mean * mean;

    let distribution = if mn == mx {
        vec![1.0]
    } else {
        // Re-bin each timestep's histogram into the global range by bin centre.
        let mut bins = vec![0.0f64; DIST_BINS];
        let to_bin =
            |v: f64| (((v - mn) / (mx - mn) * DIST_BINS as f64) as usize).min(DIST_BINS - 1);
        for s in per_ts {
            let w = s.sampled_count as f64;
            if s.distribution.len() == 1 {
                bins[to_bin(s.data_range[0])] += w;
                continue;
            }
            let [lo, hi] = s.data_range;
            let len = s.distribution.len() as f64;
            for (i, &p) in s.distribution.iter().enumerate() {
                if p != 0.0 {
                    bins[to_bin(lo + (i as f64 + 0.5) * (hi - lo) / len)] += p * w;
                }
            }
        }
        let total: f64 = bins.iter().sum();
        bins.iter()
            .map(|&b| if total == 0.0 { 0.0 } else { b / total })
            .collect()
    };

    Stats {
        data_range: [mn, mx],
        distribution_range: [mn, mx],
        distribution,
        mean,
        sampled_sum: s1,
        sampled_count: n,
        sampled_sum_sqr: s2,
        std_dev: (var_num > 0.0 && n > 1).then(|| (var_num / (nf - 1.0)).sqrt()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(values: &[f64], shape: [usize; 3]) -> Stats {
        compute(&|f| values.iter().for_each(|&v| f(v)), shape)
    }

    #[test]
    fn constant_data_has_single_bin_and_no_std_dev() {
        let s = run(&[2.5; 8], [2, 2, 2]);
        assert_eq!(s.distribution, vec![1.0]);
        assert_eq!(s.std_dev, None);
        assert_eq!(s.data_range, [2.5, 2.5]);
        assert_eq!(s.sampled_count, 8);
        assert!(serde_json::to_string(&s).unwrap().find("stdDev").is_none());
    }

    #[test]
    fn moments_and_histogram() {
        let vals: Vec<f64> = (0..10).map(f64::from).collect();
        let s = run(&vals, [1, 2, 5]);
        assert_eq!(s.data_range, [0.0, 9.0]);
        assert_eq!(s.mean, 4.5);
        assert_eq!(s.sampled_sum, 45.0);
        assert_eq!(s.sampled_sum_sqr, 285.0);
        assert!((s.std_dev.unwrap() - (82.5f64 / 9.0).sqrt()).abs() < 1e-12);
        assert_eq!(s.distribution.len(), 256);
        assert!((s.distribution.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        // max value lands in the last bin
        assert!(s.distribution[255] > 0.0);
    }

    #[test]
    fn fill_voxels_reduce_sampled_count() {
        // 2×2×2 grid with 3 voxels excluded by the caller.
        let s = run(&[1.0, 2.0, 3.0, 4.0, 5.0], [2, 2, 2]);
        assert_eq!(s.sampled_count, 5);
    }

    #[test]
    fn empty_input() {
        assert_eq!(run(&[], [1, 1, 1]), Stats::empty());
    }

    #[test]
    fn aggregate_single_is_identity_and_multi_sums() {
        let a = run(&[1.0, 2.0, 3.0], [1, 1, 3]);
        let b = run(&[5.0, 6.0, 7.0], [1, 1, 3]);
        assert_eq!(aggregate(std::slice::from_ref(&a)), a);
        let g = aggregate(&[a, b]);
        assert_eq!(g.data_range, [1.0, 7.0]);
        assert_eq!(g.sampled_count, 6);
        assert_eq!(g.sampled_sum, 24.0);
        assert!((g.distribution.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    }
}
