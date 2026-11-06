//! Baseline metric IDs, units and CLI metric-group selection.
pub const METRICS: [(&str, &str, &str); 15] = [
    (
        "wall",
        "s",
        "Elapsed process time including launch and exit",
    ),
    ("user", "s", "User-mode CPU time"),
    ("system", "s", "Kernel-mode CPU time"),
    ("cpu", "s", "Total user + system CPU time"),
    ("cpu_percent", "%", "CPU / wall time; can exceed 100%"),
    ("rss", "KiB", "Linux peak resident memory"),
    ("minor_faults", "count", "Page faults without disk I/O"),
    ("major_faults", "count", "Page faults requiring disk I/O"),
    (
        "input_blocks",
        "count",
        "Filesystem input operations, not bytes",
    ),
    (
        "output_blocks",
        "count",
        "Filesystem output operations, not bytes",
    ),
    ("voluntary_switches", "count", "Voluntary context switches"),
    (
        "involuntary_switches",
        "count",
        "Involuntary context switches",
    ),
    ("stdout_bytes", "bytes", "Captured stdout size"),
    ("stderr_bytes", "bytes", "Captured stderr size"),
    (
        "throughput",
        "units/s",
        "Declared work units / elapsed time",
    ),
];
pub(crate) fn select_metrics(value: &str) -> Result<Vec<usize>, String> {
    let mut result = Vec::new();
    for token in value.split(',').map(str::trim) {
        let group = match token {
            "all" => (0..15).collect(),
            "timing" => vec![0, 1, 2, 3, 4, 14],
            "memory" => vec![5, 6, 7],
            "io" => vec![8, 9, 12, 13],
            "scheduling" => vec![10, 11],
            name => vec![METRICS
                .iter()
                .position(|m| m.0 == name)
                .ok_or_else(|| format!("unknown metric/group: {name}; use --list-metrics"))?],
        };
        for index in group {
            if !result.contains(&index) {
                result.push(index);
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection() {
        assert_eq!(
            select_metrics("wall,memory,wall").unwrap(),
            vec![0, 5, 6, 7]
        );
        assert!(select_metrics("unknown").is_err());
        assert_eq!(select_metrics("all").unwrap().len(), 15);
    }
}
