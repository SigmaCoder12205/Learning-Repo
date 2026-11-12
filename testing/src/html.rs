//! Offline report: no remote assets, raw user strings are escaped before rendering.
use serde_json::Value;
use std::fmt::Write;

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn number(v: &Value, scale: f64, suffix: &str) -> String {
    v.as_f64()
        .filter(|v| v.is_finite())
        .map(|v| format!("{:.3} {suffix}", v * scale))
        .unwrap_or_else(|| "Unavailable".into())
}
fn chart(report: &Value) -> String {
    let Some(runs) = report["runs"].as_array() else {
        return String::new();
    };
    let values: Vec<_> = runs
        .iter()
        .filter(|r| r["status"] == "ok")
        .filter_map(|r| r["metrics"]["wall"].as_f64())
        .take(200)
        .collect();
    if values.is_empty() {
        return "<p>Time chart unavailable for this metric selection.</p>".into();
    }
    let max = values
        .iter()
        .copied()
        .reduce(f64::max)
        .unwrap_or(1.)
        .max(f64::MIN_POSITIVE);
    let width = 900. / values.len() as f64;
    let mut svg=String::from("<svg class=timing viewBox=\"0 0 940 210\" role=\"img\" aria-label=\"Elapsed time per successful run\"><line x1=\"20\" y1=\"180\" x2=\"920\" y2=\"180\" stroke=\"#94a3b8\"/>");
    for (i, value) in values.iter().enumerate() {
        let h = value / max * 150.;
        let _=write!(svg,"<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{h:.2}\" rx=\"2\" fill=\"#2563eb\"><title>Successful sample {}: {:.3} ms</title></rect>",20.+i as f64*width,180.-h,(width-2.).max(0.1),i+1,value*1000.);
    }
    let _=write!(svg,"<text x=\"20\" y=\"16\">Scale maximum: {:.3} ms</text><text x=\"20\" y=\"202\">Successful samples in order (first 200 shown)</text></svg>",max*1000.);
    svg
}
fn section(report: &Value) -> String {
    let name = report["name"].as_str().unwrap_or("Benchmark");
    let mut out = format!(
        "<section class=benchmark data-name=\"{}\"><h2>{}</h2>",
        escape(&name.to_lowercase()),
        escape(name)
    );
    if !report["error"].is_null() {
        let _ = write!(
            out,
            "<p class=bad>{}</p></section>",
            escape(report["error"].as_str().unwrap_or("Failed"))
        );
        return out;
    }
    let _ = write!(
        out,
        "<p class=muted>{} successful / {} measured runs · completed: {} · {}</p>",
        report["successful_runs"],
        report["runs"].as_array().map(Vec::len).unwrap_or(0),
        report["completed"],
        escape(report["stop_reason"].as_str().unwrap_or(""))
    );
    out.push_str("<div class=cards>");
    let target_gpu = report["gpu"]["target"]["engines_percent"]
        .as_object()
        .filter(|v| !v.is_empty());
    let gpu = target_gpu.or_else(|| report["gpu"]["engines_percent"].as_object());
    let busiest = gpu.and_then(|engines| {
        engines
            .iter()
            .filter(|(_, stats)| stats["mean"].is_number())
            .max_by(|a, b| {
                a.1["mean"]
                    .as_f64()
                    .unwrap()
                    .total_cmp(&b.1["mean"].as_f64().unwrap())
            })
    });
    let gpu_value = busiest
        .map(|(_, stats)| number(&stats["mean"], 1., "%"))
        .unwrap_or_else(|| "Unavailable".into());
    for (label, value) in [
        (
            "Average time",
            number(&report["summary"]["wall"]["mean"], 1000., "ms"),
        ),
        (
            "Highest peak memory",
            number(&report["summary"]["rss"]["max"], 1. / 1024., "MiB"),
        ),
        (
            "Average CPU",
            number(&report["summary"]["cpu_percent"]["mean"], 1., "%"),
        ),
        (
            if target_gpu.is_some() {
                "Target GPU"
            } else {
                "GPU (device-wide)"
            },
            gpu_value,
        ),
    ] {
        let _ = write!(
            out,
            "<div class=card><span>{label}</span><strong>{}</strong></div>",
            escape(&value)
        );
    }
    out.push_str("</div>");
    let insights = crate::insights::render(report);
    if !insights.is_empty() {
        let _ = write!(out, "<details open><summary>Runtime and measurement insights</summary><pre>{}</pre></details>", escape(&insights));
    }
    if let Some((name, _)) = busiest {
        let _ = write!(out, "<p class=muted>GPU card: highest mean engine activity ({}), from a separate replay. All engines are listed in the full results.</p>", escape(name));
    }
    if let Some(results) = report["diagnostics"]["results"]
        .as_object()
        .filter(|r| !r.is_empty())
    {
        out.push_str("<p>Diagnostics: ");
        for (name, result) in results {
            let status = result["status"].as_str().unwrap_or("unavailable");
            let _ = write!(
                out,
                "<span class={}>{}: {}</span> · ",
                if status == "ok" { "good" } else { "bad" },
                escape(name),
                escape(status)
            );
        }
        out.push_str("</p>");
    }
    if let Some(warnings) = report["warnings"].as_array() {
        for warning in warnings {
            let _ = write!(
                out,
                "<p class=muted>{}</p>",
                escape(warning.as_str().unwrap_or(""))
            );
        }
    }
    out.push_str(&chart(report));
    if let Some(ci) = report["summary"]["wall"]["mean_ci95"].as_object() {
        let _ = write!(
            out,
            "<p>Mean time bootstrap interval (95%, descriptive): {}–{}. Outliers retained: {}.</p>",
            number(&ci["low"], 1000., "ms"),
            number(&ci["high"], 1000., "ms"),
            report["summary"]["wall"]["outlier_count"]
        );
    }
    if report["comparison"].is_object() {
        let _ = write!(
            out,
            "<h3>Baseline comparison: {}</h3>",
            escape(report["comparison"]["status"].as_str().unwrap_or("unknown"))
        );
        out.push_str("<table><thead><tr><th>Metric</th><th>Baseline</th><th>Current</th><th>Change</th><th>Regression</th></tr></thead><tbody>");
        if let Some(changes) = report["comparison"]["changes"].as_array() {
            for change in changes {
                let _ = write!(
                    out,
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td class={}>{}</td></tr>",
                    escape(change["metric"].as_str().unwrap_or("")),
                    change["baseline"],
                    change["current"],
                    number(&change["change_percent"], 1., "%"),
                    if change["regression"] == true {
                        "bad"
                    } else {
                        "good"
                    },
                    change["regression"]
                );
            }
        }
        out.push_str("</tbody></table>");
        if let Some(differences) = report["comparison"]["differences"].as_array() {
            for difference in differences {
                let _ = write!(
                    out,
                    "<p class=bad>{}</p>",
                    escape(difference.as_str().unwrap_or(""))
                );
            }
        }
    }
    let hotspots = &report["diagnostics"]["results"]["hotspots"];
    if report["profiles"].is_object() {
        let _ = write!(
            out,
            "<h3>Domain-specific results</h3><pre>{}</pre>",
            escape(&crate::profiles::render(report, true))
        );
    }
    if hotspots["status"] == "ok" {
        out.push_str("<h3>CPU function samples</h3><div class=flamegraph>");
        out.push_str(&crate::hotspots::flamegraph(hotspots));
        out.push_str("</div><p>Width represents sample count, not elapsed time. Hover over a frame for details.</p>");
    }
    let selected: Vec<_> = report["selected_metrics"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|n| {
                    crate::metrics::METRICS
                        .iter()
                        .position(|m| Some(m.0) == n.as_str())
                })
                .collect()
        })
        .unwrap_or_default();
    let text = crate::report::render(
        report,
        "text",
        &selected,
        crate::report::View::Advanced,
        false,
    );
    let _=write!(out,"<details><summary>Full metrics, diagnostics, and individual runs</summary><pre>{}</pre></details></section>",escape(&text));
    out
}
pub fn render(report: &Value) -> String {
    let mut out = String::from(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>bench report</title><style>
:root{font:16px system-ui,sans-serif;color:#172033;background:#edf2f7}body{margin:0}header{background:#0f172a;color:white;padding:32px max(24px,calc((100% - 1120px)/2))}header p{color:#cbd5e1}main{max-width:1120px;margin:28px auto;padding:0 20px}h1{margin:0}h2{margin-top:0}section{background:white;border:1px solid #dbe3ef;border-radius:12px;padding:24px;margin-bottom:24px}button,input{font:inherit;padding:10px 14px;border:1px solid #94a3b8;border-radius:6px}button{cursor:pointer;background:#2563eb;color:white;border:0}.toolbar{display:flex;gap:12px;flex-wrap:wrap;margin-bottom:24px}.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:16px}.card{background:#f1f5f9;border-radius:8px;padding:18px}.card span{display:block;font-size:.85rem;color:#475569}.card strong{display:block;font-size:1.5rem;margin-top:8px}.muted{color:#64748b}svg.timing{width:100%;margin-top:24px}svg text{font:12px system-ui;fill:#334155}rect:hover{opacity:.75}table{border-collapse:collapse;width:100%;font-variant-numeric:tabular-nums}th,td{text-align:left;padding:10px;border-bottom:1px solid #e2e8f0}.bad{color:#b91c1c}.good{color:#166534}pre{overflow:auto;background:#0f172a;color:#dbeafe;padding:18px;border-radius:8px;font-size:12px}summary{cursor:pointer;padding:14px 0;font-weight:600}.flamegraph{overflow:auto}.flamegraph svg{min-width:700px;width:100%}[hidden]{display:none!important}@media print{button,input{display:none}header{background:white;color:black}section{break-inside:avoid}}
</style></head><body><header><h1>bench</h1><p>Measurements, comparisons, and diagnostic evidence</p></header><main><div class="toolbar"><input id="filter" type="search" placeholder="Filter benchmarks" aria-label="Filter benchmarks"><button id="download">Download JSON</button><button onclick="window.print()">Print report</button></div>"#,
    );
    if let Some(cases) = report["cases"].as_array() {
        if report["comparison"].is_object() {
            let _ = write!(
                out,
                "<p>Suite comparison: {}</p>",
                escape(&report["comparison"].to_string())
            );
        }
        for case in cases {
            out.push_str(&section(case));
        }
    } else {
        out.push_str(&section(report));
    }
    let data = serde_json::to_string(report)
        .unwrap()
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026");
    let _ = write!(
        out,
        "</main><script id=report-data type=application/json>{data}</script>"
    );
    out.push_str(r#"<script>document.getElementById('filter').addEventListener('input',function(){const q=this.value.toLowerCase();document.querySelectorAll('.benchmark').forEach(el=>el.hidden=!el.dataset.name.includes(q));});document.getElementById('download').addEventListener('click',()=>{const data=JSON.parse(document.getElementById('report-data').textContent);const url=URL.createObjectURL(new Blob([JSON.stringify(data,null,2)],{type:'application/json'}));const a=document.createElement('a');a.href=url;a.download='bench-report.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);});</script></body></html>"#);
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escaping() {
        assert_eq!(escape("<script>&\"'"), "&lt;script&gt;&amp;&quot;&#39;");
    }
}
