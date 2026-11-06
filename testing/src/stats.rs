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
