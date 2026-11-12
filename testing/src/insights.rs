//! Distribution summaries and terminal-friendly explanations; samples are never removed.
use crate::stats::percentile;
use serde_json::{json, Value};
use std::fmt::Write;

pub(crate) fn refresh(report: &mut Value) {
    let Some(runs) = report["runs"].as_array_mut() else {
        return;
    };
    let mut values: Vec<_> = runs
        .iter()
        .filter(|r| r["status"] == "ok")
        .filter_map(|r| r["metrics"]["wall"].as_f64())
        .filter(|v| v.is_finite() && *v >= 0.)
        .collect();
    if values.is_empty() {
        return;
    }
    values.sort_by(f64::total_cmp);
    let median = percentile(&values, 0.5);
    let mut deviations: Vec<_> = values.iter().map(|v| (v - median).abs()).collect();
    deviations.sort_by(f64::total_cmp);
    let q1 = percentile(&values, 0.25);
    let q3 = percentile(&values, 0.75);
    let (low, high) = (q1 - 1.5 * (q3 - q1), q3 + 1.5 * (q3 - q1));
    let mut outliers = Vec::new();
    for row in runs.iter_mut() {
        row["wall_outlier"] = if row["status"] == "ok" && values.len() >= 4 {
            row["metrics"]["wall"]
                .as_f64()
                .map(|v| json!(v < low || v > high))
                .unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        if row["wall_outlier"] == true {
            outliers.push(row["run"].clone());
        }
    }
    let (min, max) = (values[0], values[values.len() - 1]);
    let bins = if min == max { 1 } else { 8.min(values.len()) };
    let mut counts = vec![0usize; bins];
    for &value in &values {
        let index = if min == max {
            0
        } else {
            (((value - min) / (max - min) * bins as f64) as usize).min(bins - 1)
        };
        counts[index] += 1;
    }
    let histogram: Vec<_> = counts
        .iter()
        .enumerate()
        .map(|(i, &count)| {
            json!({
        "low_seconds":min+(max-min)*i as f64/bins as f64,
        "high_seconds":min+(max-min)*(i+1) as f64/bins as f64,
        "upper_inclusive":i+1==bins,"count":count})
        })
        .collect();
    let paired: Vec<_> = runs
        .iter()
        .filter(|r| r["status"] == "ok")
        .filter_map(|r| {
            Some((
                r["metrics"]["wall"].as_f64()?,
                r["metrics"]["cpu"].as_f64()?,
            ))
        })
        .collect();
    let total_wall: f64 = paired.iter().map(|p| p.0).sum();
    let total_cpu: f64 = paired.iter().map(|p| p.1).sum();
    report["wall_analysis"] = json!({"sample_count":values.len(),"median_absolute_deviation_seconds":percentile(&deviations,0.5),
        "outlier_method":"Tukey 1.5 IQR; at least four successful samples; all samples retained",
        "outlier_run_numbers":outliers,"outlier_fences_seconds":if values.len()>=4 {json!({"low":low,"high":high})}else{Value::Null},
        "histogram":histogram,"cpu_wall_ratio":if total_wall>0. {json!(total_cpu/total_wall)} else {Value::Null}});
}

pub(crate) fn enable_histogram(report: &mut Value) {
    if let Some(cases) = report["cases"].as_array_mut() {
        for case in cases {
            enable_histogram(case);
        }
    } else {
        report["presentation"] = json!({"histogram":true});
        refresh(report);
    }
}

pub(crate) fn render(report: &Value) -> String {
    let mut out = String::new();
    if let Some(kind) = report["runtime"]["kind"].as_str() {
        let _ = writeln!(
            out,
            "Runtime: {kind} ({})",
            report["runtime"]["evidence"].as_str().unwrap_or("unknown")
        );
        if let Some(interpreter) = report["runtime"]["declared_interpreter"].as_str() {
            let _ = writeln!(out, "Declared interpreter: {interpreter:?}");
        }
    }
    let analysis = &report["wall_analysis"];
    if let Some(ratio) = analysis["cpu_wall_ratio"].as_f64() {
        let _ = writeln!(out, "CPU / wall time: {ratio:.2} core equivalents (summed CPU divided by elapsed time; not an I/O-wait measurement).");
    }
    if let Some(mad) = analysis["median_absolute_deviation_seconds"].as_f64() {
        let _ = writeln!(out, "Timing spread (median absolute deviation): {:.3} ms; outlier run numbers: {} (retained).", mad*1000., analysis["outlier_run_numbers"]);
        if analysis["outlier_fences_seconds"].is_null() {
            out.push_str("Outlier labels need at least four successful samples.\n");
        }
    }
    if report["summary"]["wall"]["cv_percent"]
        .as_f64()
        .is_some_and(|v| v > 10.)
    {
        out.push_str("Timing variability exceeds 10%; inspect individual runs and background activity before drawing conclusions.\n");
    }
    if report["presentation"]["histogram"] == true {
        out.push_str("\nWall-time histogram (successful runs, including outliers)\n");
        if let Some(bins) = analysis["histogram"].as_array() {
            let peak = bins
                .iter()
                .filter_map(|b| b["count"].as_u64())
                .max()
                .unwrap_or(1)
                .max(1);
            for bin in bins {
                let count = bin["count"].as_u64().unwrap_or(0);
                let length = (count * 32).div_ceil(peak) as usize;
                let _ = writeln!(
                    out,
                    "  [{:10.3}, {:10.3}{} ms | {:<32} {count}",
                    bin["low_seconds"].as_f64().unwrap_or(0.) * 1000.,
                    bin["high_seconds"].as_f64().unwrap_or(0.) * 1000.,
                    if bin["upper_inclusive"] == true {
                        ']'
                    } else {
                        ')'
                    },
                    "#".repeat(length)
                );
            }
        } else {
            out.push_str("  Unavailable: select wall timing and collect successful runs.\n");
        }
    }
    let calibration = &report["python_overhead"];
    if let Some(status) = calibration["status"].as_str() {
        let _ = writeln!(
            out,
            "\nPython startup calibration: {status} (separate empty-script processes)"
        );
        if let Some(value) = calibration["summary"]["wall"]["median"].as_f64() {
            let _ = writeln!(out, "  Empty-script median: {:.3} ms", value * 1000.);
        }
        if let Some(value) = calibration["estimated_workload_wall_seconds"].as_f64() {
            let _ = writeln!(out, "  Full median minus empty-script median: {:.3} ms (estimate; may be negative due to noise)", value*1000.);
        }
        if let Some(reason) = calibration["reason"].as_str() {
            let _ = writeln!(out, "  {reason:?}");
        }
        out.push_str("  Full-process timing and RSS remain unchanged; this does not isolate imports or function execution.\n");
    }
    if let Some(changes) = report["comparison"]["advisory"]["changes"].as_array() {
        for change in changes.iter().filter(|c| c["regression"] == true) {
            let _ = writeln!(
                out,
                "Regression warning: {} exceeded {}% (warning only).",
                change["metric"], change["threshold_percent"]
            );
        }
    }
    if let Some(differences) = report["comparison"]["advisory"]["differences"].as_array() {
        for difference in differences {
            let _ = writeln!(out, "Comparison note: {difference}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn histogram_and_outliers_keep_successes_and_ignore_failures() {
        let mut report = json!({"runs":[
            {"run":1,"status":"ok","metrics":{"wall":1.,"cpu":0.5}},
            {"run":2,"status":"ok","metrics":{"wall":1.}},
            {"run":3,"status":"ok","metrics":{"wall":1.}},
            {"run":4,"status":"ok","metrics":{"wall":1.}},
            {"run":5,"status":"ok","metrics":{"wall":20.}},
            {"run":6,"status":"timeout","metrics":{"wall":100.}}]});
        refresh(&mut report);
        assert_eq!(report["wall_analysis"]["outlier_run_numbers"], json!([5]));
        assert_eq!(
            report["wall_analysis"]["median_absolute_deviation_seconds"],
            0.
        );
        assert_eq!(report["wall_analysis"]["cpu_wall_ratio"], 0.5);
        assert!(report["runs"][5]["wall_outlier"].is_null());
        assert_eq!(
            report["wall_analysis"]["histogram"]
                .as_array()
                .unwrap()
                .iter()
                .map(|b| b["count"].as_u64().unwrap())
                .sum::<u64>(),
            5
        );
        report["runs"] = json!([{"run":1,"status":"ok","metrics":{"wall":0.}}]);
        enable_histogram(&mut report);
        assert_eq!(
            report["wall_analysis"]["histogram"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(render(&report).contains("#"));
    }
}
