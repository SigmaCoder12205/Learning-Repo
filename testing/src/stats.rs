//! Descriptive statistics and the adaptive stopping precision estimate.
use serde_json::{json, Value};
pub(crate) fn percentile(values: &[f64], fraction: f64) -> f64 {
    let index = (values.len() - 1) as f64 * fraction;
    let low = index.floor() as usize;
    let high = (low + 1).min(values.len() - 1);
    values[low] + (values[high] - values[low]) * (index - low as f64)
}
pub(crate) fn summarize(values: Vec<f64>) -> Value {
    let mut result = summarize_basic(values.clone());
    let extra = intervals(&values);
    result
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    result
}
pub(crate) fn summarize_basic(mut values: Vec<f64>) -> Value {
    values.sort_by(f64::total_cmp);
    let count = values.len();
    let mean = values.iter().sum::<f64>() / count as f64;
    let stdev = if count > 1 {
        (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (count - 1) as f64).sqrt()
    } else {
        0.0
    };
    json!({"count": count, "min": values[0], "max": values[count-1], "mean": mean,
        "median": percentile(&values, 0.5), "stdev": stdev, "p90": percentile(&values, 0.9),
        "p95": percentile(&values, 0.95), "cv_percent": if mean != 0.0 { stdev / mean * 100.0 } else { 0.0 }})
}
pub fn precision_met(values: &[f64], target: f64) -> bool {
    if values.len() < 5 {
        return false;
    }
    let s = summarize_basic(values.to_vec());
    let mean = s["mean"].as_f64().unwrap_or(0.);
    mean > 0.
        && 196. * s["stdev"].as_f64().unwrap_or(f64::INFINITY) / (values.len() as f64).sqrt() / mean
            <= target
}
/// Two-sided Mann-Whitney U (Wilcoxon rank-sum) test of `current` against `baseline`.
///
/// Small samples without ties use the exact null distribution; otherwise a normal
/// approximation with tie and continuity corrections. `probability_current_lower` is
/// P(current < baseline) + P(equal)/2, the common-language effect size.
pub fn mann_whitney(current: &[f64], baseline: &[f64]) -> Value {
    let (n1, n2) = (current.len(), baseline.len());
    if n1 < 2 || n2 < 2 {
        return json!({"status":"insufficient_samples","n_current":n1,"n_baseline":n2,
            "reason":"needs at least two successful runs on each side"});
    }
    let mut pooled: Vec<(f64, bool)> = current
        .iter()
        .map(|&v| (v, true))
        .chain(baseline.iter().map(|&v| (v, false)))
        .collect();
    pooled.sort_by(|a, b| a.0.total_cmp(&b.0));
    let n = pooled.len();
    let mut rank_current = 0.0;
    let mut tie_term = 0.0;
    let mut i = 0;
    while i < n {
        let mut j = i;
        while j + 1 < n && pooled[j + 1].0 == pooled[i].0 {
            j += 1;
        }
        let average = (i + j) as f64 / 2.0 + 1.0;
        let tied = (j - i + 1) as f64;
        tie_term += tied.powi(3) - tied;
        rank_current += pooled[i..=j].iter().filter(|p| p.1).count() as f64 * average;
        i = j + 1;
    }
    let (f1, f2) = (n1 as f64, n2 as f64);
    // U counts pairs where current > baseline (ties count one half).
    let u = rank_current - f1 * (f1 + 1.0) / 2.0;
    let mean = f1 * f2 / 2.0;
    let (p_value, method) = if tie_term == 0.0 && n1 * n2 <= 400 {
        // Exact: number of rank arrangements giving each U, by dynamic programming.
        let max = n1 * n2;
        let mut table = vec![vec![vec![0f64; max + 1]; n2 + 1]; n1 + 1];
        for row in table[0].iter_mut() {
            row[0] = 1.0;
        }
        for a in 1..=n1 {
            table[a][0][0] = 1.0;
            for b in 1..=n2 {
                for k in 0..=a * b {
                    let with_a_largest = if k >= b { table[a - 1][b][k - b] } else { 0.0 };
                    table[a][b][k] = with_a_largest + table[a][b - 1][k];
                }
            }
        }
        let counts = &table[n1][n2];
        let total: f64 = counts.iter().sum();
        let low = u.min(f1 * f2 - u).round() as usize;
        let tail: f64 = counts[..=low].iter().sum::<f64>() / total;
        ((2.0 * tail).min(1.0), "exact Mann-Whitney U, two-sided")
    } else {
        let variance =
            f1 * f2 / 12.0 * ((f1 + f2 + 1.0) - tie_term / ((f1 + f2) * (f1 + f2 - 1.0)));
        if variance <= 0.0 {
            (1.0, "normal approximation (all values tied)")
        } else {
            let z = ((u - mean).abs() - 0.5).max(0.0) / variance.sqrt();
            (
                (2.0 * normal_upper_tail(z)).min(1.0),
                "normal approximation with tie and continuity corrections, two-sided",
            )
        }
    };
    json!({"status":"ok","test":"mann_whitney_u","u_current_greater":u,"n_current":n1,"n_baseline":n2,
        "p_value":p_value,"method":method,"probability_current_lower":1.0 - u / (f1 * f2),
        "significant_at_0_05":p_value < 0.05,
        "scope":"Rank test on successful measured runs; independent runs assumed, so drift or load during one experiment can still mislead"})
}

/// Upper tail of the standard normal, via the complementary error function
/// (W. J. Cody's rational approximation as given in Numerical Recipes, |error| < 1.2e-7).
fn normal_upper_tail(z: f64) -> f64 {
    let x = z / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.5 * x.abs());
    let poly = -x * x - 1.26551223
        + t * (1.00002368
            + t * (0.37409196
                + t * (0.09678418
                    + t * (-0.18628806
                        + t * (0.27886807
                            + t * (-1.13520398
                                + t * (1.48851587 + t * (-0.82215223 + t * 0.17087277))))))));
    let erfc = t * poly.exp();
    0.5 * if x >= 0.0 { erfc } else { 2.0 - erfc }
}

pub fn intervals(values: &[f64]) -> Value {
    if values.len() < 2 {
        return json!({"mean_ci95": null, "method": "insufficient samples", "outlier_count": 0});
    }
    let mut random = 0x4d595df4d0f33173_u64;
    let mut means = Vec::with_capacity(1000);
    for _ in 0..1000 {
        let mut mean = 0.;
        for _ in values {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            mean += values[(random as usize) % values.len()] / values.len() as f64;
        }
        means.push(mean);
    }
    means.sort_by(f64::total_cmp);
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let q1 = percentile(&sorted, 0.25);
    let q3 = percentile(&sorted, 0.75);
    let iqr = q3 - q1;
    json!({"mean_ci95": {"low": percentile(&means, 0.025), "high": percentile(&means, 0.975)},
        "method": "deterministic percentile bootstrap, 1000 resamples; descriptive, especially with adaptive stopping",
        "outlier_count": values.iter().filter(|&&v| v < q1-1.5*iqr || v > q3+1.5*iqr).count()})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn statistics() {
        let s = summarize(vec![5., 1., 4., 2., 3.]);
        assert_eq!(s["count"], 5);
        assert_eq!(s["median"], 3.);
        assert!((s["p95"].as_f64().unwrap() - 4.8).abs() < 1e-10);
        assert!((s["stdev"].as_f64().unwrap() - 1.5811388300841898).abs() < 1e-10);
        assert_eq!(summarize(vec![0.])["cv_percent"], 0.);
    }
    #[test]
    fn mann_whitney_matches_reference_values() {
        // Exact: complete separation of 5 vs 5 gives p = 2/252 (scipy: 0.007937).
        let r = mann_whitney(&[1., 2., 3., 4., 5.], &[6., 7., 8., 9., 10.]);
        assert!((r["p_value"].as_f64().unwrap() - 2. / 252.).abs() < 1e-12);
        assert_eq!(r["probability_current_lower"], 1.0);
        assert_eq!(r["u_current_greater"], 0.0);
        // Balanced interleaving: scipy.stats.mannwhitneyu exact p = 1.0.
        let r = mann_whitney(&[1., 4., 5., 8.], &[2., 3., 6., 7.]);
        assert_eq!(r["p_value"], 1.0, "{r}");
        // Exact, U = 2 with 4 vs 4: scipy gives 0.114285714.
        let r = mann_whitney(&[1., 2., 3., 6.], &[4., 5., 7., 8.]);
        assert!(
            (r["p_value"].as_f64().unwrap() - 0.114285714).abs() < 1e-8,
            "{r}"
        );
        // Ties use the normal approximation; scipy (asymptotic, continuity) gives 0.0181009.
        let r = mann_whitney(&[1., 1., 2., 3., 3., 4.], &[3., 4., 4., 5., 5., 6.]);
        assert!(r["method"].as_str().unwrap().starts_with("normal"));
        assert!(
            (r["p_value"].as_f64().unwrap() - 0.0181009).abs() < 1e-5,
            "{r}"
        );
        assert_eq!(
            mann_whitney(&[1.], &[2., 3.])["status"],
            "insufficient_samples"
        );
        assert_eq!(mann_whitney(&[2.; 4], &[2.; 4])["p_value"], 1.0);
        assert!((normal_upper_tail(1.959964) - 0.025).abs() < 1e-6);
    }
    #[test]
    fn uncertainty_and_adaptive_stopping_keep_outliers() {
        assert!(intervals(&[1.])["mean_ci95"].is_null());
        assert_eq!(
            intervals(&[2.; 5])["mean_ci95"],
            json!({"low":2.,"high":2.})
        );
        let values = [1., 1., 1., 1., 1., 20.];
        assert_eq!(intervals(&values)["outlier_count"], 1);
        assert!(!precision_met(&values, 5.));
        assert!(!precision_met(&[2.; 4], 5.));
        assert!(precision_met(&[2.; 5], 5.));
    }
}
