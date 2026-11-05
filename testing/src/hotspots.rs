use crate::{diagnostics, runner::RunOptions};
use serde_json::{json, Value};
use std::{collections::BTreeMap, ffi::OsString, fmt::Write, fs, path::Path};

pub fn run(target: &[OsString], opts: &RunOptions<'_>, dir: &Path) -> Result<Value, String> {
    let perf = diagnostics::find_tool("perf", opts.env)?;
    let data = dir.join("perf.data");
    let mut command = vec![
        perf.clone(),
        "record".into(),
        "-q".into(),
        "--all-user".into(),
        "--user-callchains".into(),
        "-e".into(),
        "cycles:u".into(),
        "-F".into(),
        "99".into(),
        "--call-graph".into(),
        "dwarf".into(),
        "-o".into(),
        data.clone().into_os_string(),
        "--".into(),
    ];
    command.extend_from_slice(target);
    let row = diagnostics::command_run(&command, opts, dir, "hotspots-record")?;
    if let Some(value) = diagnostics::invocation_failed(&row) {
        return Ok(value);
    }
    let analysis_opts = RunOptions {
        input: None,
        expected: None,
        expected_exit: 0,
        ..*opts
    };
    let command = vec![
        perf,
        "script".into(),
        "--no-inline".into(),
        "-F".into(),
        "ip,sym,dso,srcline".into(),
        "-i".into(),
        data.clone().into_os_string(),
    ];
    let analysis = diagnostics::command_run(&command, &analysis_opts, dir, "hotspots-stacks")?;
    if let Some(value) = diagnostics::invocation_failed(&analysis) {
        return Ok(value);
    }
    let mut result = parse(&diagnostics::read_report(
        &dir.join("hotspots-stacks.stdout"),
    )?)?;
    let svg = flamegraph(&result);
    let svg_path = dir.join("flamegraph.svg");
    fs::write(&svg_path, svg).map_err(|e| e.to_string())?;
    let mut folded = String::new();
    for stack in result["stacks"].as_array().unwrap() {
        let names: Vec<_> = stack["frames"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap_or("").replace([';', '\n', '\r'], " "))
            .collect();
        let _ = writeln!(folded, "{} {}", names.join(";"), stack["samples"]);
    }
    fs::write(dir.join("stacks.folded"), folded).map_err(|e| e.to_string())?;
    result["invocation"] = row;
    result["analysis_invocation"] = analysis;
    result["raw_file"] = json!(data);
    result["flamegraph_file"] = json!(svg_path);
    result["scope"]=json!("user-space sampled call stacks from one separate replay at requested 99 Hz; self percentages are sample shares, not exact function timings; source locations require debug symbols");
    Ok(result)
}
pub fn parse(text: &str) -> Result<Value, String> {
    let mut stacks = BTreeMap::<Vec<String>, u64>::new();
    let mut current = Vec::new();
    let mut sources = BTreeMap::new();
    let finish = |current: &mut Vec<String>, stacks: &mut BTreeMap<Vec<String>, u64>| {
        if !current.is_empty() {
            current.reverse();
            *stacks.entry(std::mem::take(current)).or_default() += 1;
        }
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            finish(&mut current, &mut stacks);
            continue;
        }
        // perf prints srcline on a separate line in current versions.
        if line.rsplit_once(':').is_some_and(|(file, location)| {
            file != "??"
                && location
                    .split_whitespace()
                    .next()
                    .is_some_and(|n| n.parse::<u64>().is_ok())
        }) && !line.contains(" (")
        {
            if let Some(frame) = current.last() {
                sources.insert(frame.clone(), line.to_string());
            }
            continue;
        }
        let Some((address, rest)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        if !address.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let rest = rest.trim();
        let (symbol, tail) = rest.split_once(" (").unwrap_or((rest, ""));
        let symbol = symbol.split("+0x").next().unwrap_or(symbol).to_string();
        let (object, source) = tail.split_once(')').unwrap_or((tail, ""));
        let label = format!("{symbol} ({object})");
        if !source.trim().is_empty() {
            sources.insert(label.clone(), source.trim().to_string());
        }
        if current.len() < 128 {
            current.push(label);
        }
    }
    finish(&mut current, &mut stacks);
    let total: u64 = stacks.values().sum();
    if total == 0 {
        return Err(
            "no CPU stack samples; increase the workload duration or check perf permissions".into(),
        );
    }
    let mut leaves = BTreeMap::<String, u64>::new();
    for (frames, count) in &stacks {
        *leaves.entry(frames.last().unwrap().clone()).or_default() += count;
    }
    let mut tops: Vec<_> = leaves.into_iter().collect();
    tops.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let functions:Vec<_>=tops.into_iter().take(50).map(|(name,count)|json!({"function":name,"self_samples":count,"self_percent":count as f64/total as f64*100.,"source":sources.get(&name)})).collect();
    let stacks: Vec<_> = stacks
        .into_iter()
        .map(|(frames, samples)| json!({"frames":frames,"samples":samples}))
        .collect();
    Ok(json!({"status":"ok","sample_count":total,"functions":functions,"stacks":stacks}))
}
#[derive(Default)]
struct Node {
    count: u64,
    children: BTreeMap<String, Node>,
}
pub fn flamegraph(report: &Value) -> String {
    let mut root = Node::default();
    let mut depth = 1;
    if let Some(stacks) = report["stacks"].as_array() {
        for stack in stacks {
            let count = stack["samples"].as_u64().unwrap_or(0);
            let Some(frames) = stack["frames"].as_array() else {
                continue;
            };
            depth = depth.max(frames.len());
            root.count += count;
            let mut node = &mut root;
            for frame in frames {
                node = node
                    .children
                    .entry(frame.as_str().unwrap_or("unknown").into())
                    .or_default();
                node.count += count;
            }
        }
    }
    let height = depth * 24 + 45;
    let mut svg=format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1200 {height}\" role=\"img\" aria-label=\"CPU sample flamegraph\"><style>text{{font:12px monospace;pointer-events:none}}rect:hover{{stroke:#111;stroke-width:2}}</style><text x=\"8\" y=\"18\">CPU sample flamegraph — hover for function and sample count</text>");
    fn draw(node: &Node, x: f64, width: f64, level: usize, height: usize, out: &mut String) {
        let mut offset = x;
        for (name, child) in &node.children {
            let w = width * child.count as f64 / node.count.max(1) as f64;
            if w >= 0.5 {
                let y = height - 25 - level * 24;
                let hue = 20 + name.bytes().fold(0u32, |s, b| s.wrapping_add(b as u32)) % 35;
                let _=write!(out,"<g><title>{}: {} samples</title><rect x=\"{offset:.2}\" y=\"{y}\" width=\"{w:.2}\" height=\"22\" fill=\"hsl({hue},80%,70%)\"/>",crate::html::escape(name),child.count);
                if w > 35. {
                    let label: String = name.chars().take((w / 7.).max(0.) as usize).collect();
                    let _ = write!(
                        out,
                        "<text x=\"{:.2}\" y=\"{}\">{}</text>",
                        offset + 3.,
                        y + 15,
                        crate::html::escape(&label)
                    );
                }
                out.push_str("</g>");
                draw(child, offset, w, level + 1, height, out);
            }
            offset += w;
        }
    }
    draw(&root, 0., 1200., 0, height, &mut svg);
    svg.push_str("</svg>");
    svg
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_leaf_counts_and_escapes_svg() {
        let text=" aa work (/app) /src/a.cpp:3\n bb main (/app)\n\n aa work (/app)\n bb main (/app)\n\n cc <script> (/app)\n bb main (/app)\n";
        let p = parse(text).unwrap();
        assert_eq!(p["sample_count"], 3);
        assert_eq!(p["functions"][0]["self_samples"], 2);
        assert!(flamegraph(&p).contains("&lt;script&gt;"));
        assert!(!flamegraph(&p).contains("<script>"));
    }
    #[test]
    fn recognizes_separate_perf_source_lines() {
        let p = parse(include_str!("../tests/fixtures/perf_stacks.txt")).unwrap();
        assert_eq!(p["sample_count"], 2);
        assert_eq!(p["functions"][0]["source"], "workload.cpp:9");
    }
}
