//! Atomic JSON report writes and append-only checkpoint recovery.
use crate::experiment::refresh;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufRead, BufReader, Write},
    path::Path,
};
pub fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut temp, value).map_err(|e| e.to_string())?;
    temp.write_all(b"\n").map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
pub struct Journal(Option<File>);
impl Journal {
    pub fn open(path: Option<&Path>) -> Result<Self, String> {
        Ok(Self(
            path.map(|p| {
                File::options()
                    .write(true)
                    .create_new(true)
                    .open(p)
                    .map_err(|e| format!("checkpoint {}: {e}", p.display()))
            })
            .transpose()?,
        ))
    }
    pub fn emit(&mut self, value: Value) -> Result<(), String> {
        if let Some(file) = &mut self.0 {
            serde_json::to_writer(&mut *file, &value).map_err(|e| e.to_string())?;
            file.write_all(b"\n").map_err(|e| e.to_string())?;
            file.sync_data().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
pub fn recover(path: &Path) -> Result<Value, String> {
    fn validate_report(report: &Value) -> Result<(), String> {
        if let Some(cases) = report["cases"].as_array() {
            for case in cases {
                validate_report(case)?;
            }
            return Ok(());
        }
        if !report.is_object()
            || !report["runs"].is_array()
            || !report["warmups"].is_array()
            || !report["selected_metrics"]
                .as_array()
                .is_some_and(|a| a.iter().all(Value::is_string))
        {
            return Err("invalid checkpoint report structure".into());
        }
        if report["error"].is_string() {
            return Ok(());
        }
        if !report["warnings"]
            .as_array()
            .is_some_and(|a| a.iter().all(Value::is_string))
            || !report["summary"].is_object()
        {
            return Err("checkpoint report lacks warnings or summary".into());
        }
        for row in report["runs"]
            .as_array()
            .unwrap()
            .iter()
            .chain(report["warmups"].as_array().unwrap())
        {
            if !row["status"].is_string() || !row["metrics"].is_object() {
                return Err("invalid checkpoint sample".into());
            }
        }
        for stats in report["summary"].as_object().unwrap().values() {
            for key in [
                "mean",
                "median",
                "min",
                "max",
                "stdev",
                "p90",
                "p95",
                "cv_percent",
            ] {
                if stats[key].as_f64().is_none() {
                    return Err("invalid checkpoint statistics".into());
                }
            }
        }
        Ok(())
    }
    let mut reader = BufReader::new(File::open(path).map_err(|e| e.to_string())?);
    let mut reports = BTreeMap::<String, Value>::new();
    let mut order = Vec::new();
    let mut suite = None;
    let mut final_report = None;
    let mut line = String::new();
    let mut number = 0;
    loop {
        line.clear();
        if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            break;
        }
        number += 1;
        if !line.ends_with('\n') {
            break;
        } // Torn final record after termination.
        let event: Value = serde_json::from_str(&line)
            .map_err(|e| format!("invalid checkpoint line {number}: {e}"))?;
        if matches!(
            event["event"].as_str(),
            Some("suite_start" | "start" | "report" | "final")
        ) {
            validate_report(&event["report"])?;
        }
        match event["event"].as_str() {
            Some("suite_start") => suite = Some(event["report"].clone()),
            Some("start") => {
                let r = event["report"].clone();
                let name = r["name"]
                    .as_str()
                    .ok_or("checkpoint start lacks name")?
                    .to_string();
                order.push(name.clone());
                reports.insert(name, r);
            }
            Some("sample") => {
                if !event["row"]["status"].is_string() || !event["row"]["metrics"].is_object() {
                    return Err("invalid checkpoint sample".into());
                }
                let name = event["name"]
                    .as_str()
                    .ok_or("checkpoint sample lacks name")?;
                let r = reports.get_mut(name).ok_or("sample before start")?;
                let phase = event["phase"]
                    .as_str()
                    .filter(|p| *p == "runs" || *p == "warmups")
                    .ok_or("invalid phase")?;
                r[phase]
                    .as_array_mut()
                    .ok_or("invalid start record")?
                    .push(event["row"].clone());
            }
            Some("report") => {
                let r = event["report"].clone();
                let name = r["name"].as_str().ok_or("report lacks name")?.to_string();
                if !order.contains(&name) {
                    order.push(name.clone());
                }
                reports.insert(name, r);
            }
            Some("final") => final_report = Some(event["report"].clone()),
            _ => return Err(format!("unknown checkpoint event on line {number}")),
        }
    }
    if let Some(report) = final_report {
        return Ok(report);
    }
    for report in reports.values_mut() {
        refresh(report);
        if report["completed"] != true {
            report["stop_reason"] = json!("recovered_partial");
            report["measurement_passed"] = json!(false);
        }
    }
    if let Some(mut suite) = suite {
        suite["cases"] = json!(order
            .into_iter()
            .filter_map(|n| reports.remove(&n))
            .collect::<Vec<_>>());
        suite["completed"] = json!(false);
        suite["recovered"] = json!(true);
        return Ok(suite);
    }
    let mut report = reports
        .into_values()
        .next()
        .ok_or("checkpoint has no measurements")?;
    report["recovered"] = json!(true);
    Ok(report)
}
