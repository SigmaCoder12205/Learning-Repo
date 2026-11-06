//! valgrind replay, parsing and format fixtures.
use super::{command_run, find_tool, invocation_failed, read_report};
use crate::runner::RunOptions;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;

pub(super) fn run(target: &[OsString], opts: &RunOptions<'_>, dir: &Path) -> Result<Value, String> {
    let tool = find_tool("valgrind", opts.env)?;
    let path = dir.join("valgrind.xml");
    let mut command = vec![
        tool,
        "--tool=memcheck".into(),
        "--xml=yes".into(),
        format!(
            "--xml-file={}",
            path.display().to_string().replace('%', "%%")
        )
        .into(),
        "--leak-check=full".into(),
        "--show-leak-kinds=all".into(),
        "--track-origins=yes".into(),
        "--trace-children=no".into(),
        "--child-silent-after-fork=yes".into(),
        "--".into(),
    ];
    command.extend_from_slice(target);
    let row = command_run(&command, opts, dir, "valgrind")?;
    if let Some(value) = invocation_failed(&row) {
        return Ok(value);
    }
    let mut result = parse_valgrind(&read_report(&path)?)?;
    result["invocation"] = row;
    result["raw_file"] = json!(path);
    result["scope"] = json!("Memcheck replay; exec children are not traced; still-reachable memory is reported but not treated as an error");
    Ok(result)
}
fn parse_valgrind(text: &str) -> Result<Value, String> {
    let doc = roxmltree::Document::parse(text)
        .map_err(|e| format!("invalid/incomplete Valgrind XML: {e}"))?;
    if doc.root_element().tag_name().name() != "valgrindoutput"
        || !doc
            .descendants()
            .any(|n| n.has_tag_name("state") && n.text() == Some("FINISHED"))
    {
        return Err("Valgrind did not finish a valid Memcheck report".into());
    }
    let child_text = |node: roxmltree::Node<'_, '_>, tag: &str| -> String {
        node.children()
            .find(|n| n.has_tag_name(tag))
            .and_then(|n| n.text())
            .unwrap_or("")
            .into()
    };
    let mut counts = BTreeMap::new();
    for pair in doc.descendants().filter(|n| {
        n.has_tag_name("pair") && n.parent().is_some_and(|p| p.has_tag_name("errorcounts"))
    }) {
        counts.insert(
            child_text(pair, "unique"),
            child_text(pair, "count").parse::<u64>().unwrap_or(1),
        );
    }
    let mut kinds = BTreeMap::<String, u64>::new();
    let mut leaks = BTreeMap::<String, u64>::new();
    let mut errors = Vec::new();
    let mut actionable = 0_u64;
    let mut contexts = 0_u64;
    for node in doc.descendants().filter(|n| n.has_tag_name("error")) {
        let kind = child_text(node, "kind");
        let count = *counts.get(&child_text(node, "unique")).unwrap_or(&1);
        *kinds.entry(kind.clone()).or_default() += count;
        contexts += 1;
        if kind != "Leak_StillReachable" {
            actionable += 1;
        }
        if let Some(bytes) = node
            .descendants()
            .find(|n| n.has_tag_name("leakedbytes"))
            .and_then(|n| n.text())
            .and_then(|s| s.parse::<u64>().ok())
        {
            *leaks.entry(kind.clone()).or_default() += bytes;
        }
        if errors.len() < 20 {
            let description = node
                .children()
                .find(|n| n.has_tag_name("what"))
                .and_then(|n| n.text())
                .or_else(|| {
                    node.descendants()
                        .find(|n| n.has_tag_name("text"))
                        .and_then(|n| n.text())
                })
                .unwrap_or("");
            let frames: Vec<_> = node
                .descendants()
                .filter(|n| n.has_tag_name("frame"))
                .take(12)
                .map(|n| {
                    json!({"function": child_text(n, "fn"), "file": child_text(n, "file"),
                    "directory": child_text(n, "dir"), "line": child_text(n, "line")})
                })
                .collect();
            errors.push(json!({"kind": kind, "occurrences": count, "description": description, "frames": frames}));
        }
    }
    Ok(
        json!({"status": if actionable == 0 { "ok" } else { "findings" },
        "error_contexts": contexts, "actionable_contexts": actionable, "occurrences_by_kind": kinds,
        "leaked_bytes_by_kind": leaks, "errors": errors, "errors_truncated": contexts > 20}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn valgrind_reports_errors_leaks_and_contexts() {
        let xml = r#"<valgrindoutput><status><state>FINISHED</state></status>
        <error><unique>0</unique><kind>InvalidRead</kind><what>bad read</what></error>
        <error><unique>1</unique><kind>Leak_DefinitelyLost</kind><xwhat><text>lost</text><leakedbytes>64</leakedbytes></xwhat></error>
        <error><unique>2</unique><kind>Leak_StillReachable</kind><xwhat><leakedbytes>8</leakedbytes></xwhat></error>
        <errorcounts><pair><unique>0</unique><count>4</count></pair></errorcounts></valgrindoutput>"#;
        let r = parse_valgrind(xml).unwrap();
        assert_eq!(r["status"], "findings");
        assert_eq!(r["actionable_contexts"], 2);
        assert_eq!(r["occurrences_by_kind"]["InvalidRead"], 4);
        assert_eq!(r["leaked_bytes_by_kind"]["Leak_DefinitelyLost"], 64);
        assert!(parse_valgrind("<valgrindoutput>").is_err());
    }
}
