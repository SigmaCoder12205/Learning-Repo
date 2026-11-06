//! Shared command-line options for single workloads and suite entries.
use crate::report::View;
use clap::Parser;
use std::{ffi::OsString, path::PathBuf};
fn positive(s: &str) -> Result<f64, String> {
    let n: f64 = s.parse().map_err(|_| "expected a number")?;
    if n.is_finite() && n > 0.0 {
        Ok(n)
    } else {
        Err("must be finite and greater than zero".into())
    }
}
#[derive(Parser, Debug, Clone)]
#[command(
    version,
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true,
    about = "Native Rust/C process benchmarks for Linux",
    after_help = "Examples:\n  cppbench ./build/program\n  cppbench ./program --runs 30 --metrics timing,memory -- --size 1000\n  cppbench --source main.cpp --cxxflag=-march=native"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Option<crate::accounts::Command>,
    /// Executable path to benchmark
    #[arg(conflicts_with_all = ["executable", "source"])]
    pub(crate) program: Option<PathBuf>,
    /// Existing executable (alternative to positional path)
    #[arg(long, conflicts_with = "source")]
    pub(crate) executable: Option<PathBuf>,
    /// C++ translation units to compile once
    #[arg(long, num_args = 1..)]
    pub(crate) source: Vec<PathBuf>,
    #[arg(long, default_value = "g++")]
    pub(crate) compiler: PathBuf,
    #[arg(long = "std", default_value = "c++17")]
    pub(crate) standard: String,
    #[arg(long, default_value = "2", value_parser = ["0", "1", "2", "3", "s", "g", "fast"])]
    pub(crate) opt: String,
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) cxxflag: Vec<OsString>,
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) ldflag: Vec<OsString>,
    #[arg(long, default_value = "120", value_parser = positive)]
    pub(crate) compile_timeout: f64,
    #[arg(long, default_value = "10", value_parser = clap::value_parser!(u32).range(1..))]
    pub(crate) runs: u32,
    #[arg(long, default_value = "1")]
    pub(crate) warmup: u32,
    /// Deadline in seconds per invocation
    #[arg(long, default_value = "10", value_parser = positive)]
    pub(crate) timeout: f64,
    /// Combined stdout/stderr limit in MiB
    #[arg(long, default_value = "64", value_parser = positive)]
    pub(crate) output_limit: f64,
    #[arg(long)]
    pub(crate) input: Option<PathBuf>,
    #[arg(long)]
    pub(crate) expect_output: Option<PathBuf>,
    #[arg(long, default_value = "0", allow_negative_numbers = true)]
    pub(crate) expect_exit: i32,
    #[arg(long, default_value = ".")]
    pub(crate) cwd: PathBuf,
    /// Repeatable environment override
    #[arg(long, value_name = "KEY=VALUE")]
    pub(crate) env: Vec<String>,
    #[arg(long, default_value = "1", value_parser = positive)]
    pub(crate) units: f64,
    /// Comma-separated metric names or groups
    #[arg(long)]
    pub(crate) metrics: Option<String>,
    /// Text report detail (exports retain all selected data)
    #[arg(long, value_enum, default_value = "simple")]
    pub(crate) view: View,
    /// Show resource ranges and CPU breakdown
    #[arg(long, conflicts_with_all = ["advanced", "view"])]
    pub(crate) intermediate: bool,
    /// Show all statistics, configuration, and individual runs
    #[arg(long, conflicts_with_all = ["intermediate", "view"])]
    pub(crate) advanced: bool,
    #[arg(long)]
    pub(crate) list_metrics: bool,
    #[arg(long, default_value = "text", value_parser = ["text", "json", "csv", "html"])]
    pub(crate) format: String,
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    #[arg(long)]
    pub(crate) keep_going: bool,
    /// Extra replay with CPU hardware counters (user-space events)
    #[arg(long)]
    pub(crate) perf: bool,
    /// Extra replay monitored by intel_gpu_top (device-wide activity)
    #[arg(long)]
    pub(crate) gpu: bool,
    /// Extra replay with heap allocation profiling
    #[arg(long)]
    pub(crate) heaptrack: bool,
    /// Extra replay with Valgrind Memcheck
    #[arg(long)]
    pub(crate) valgrind: bool,
    /// Run all five optional diagnostic passes after the baseline
    #[arg(long)]
    pub(crate) all_tools: bool,
    /// Seconds allowed per diagnostic or analysis command
    #[arg(long, default_value = "120", value_parser = positive)]
    pub(crate) diagnostic_timeout: f64,
    /// Parent directory for retained diagnostic artifacts
    #[arg(long, default_value = "cppbench-artifacts")]
    pub(crate) artifacts_dir: PathBuf,
    /// Comma-separated perf stat events
    #[arg(
        long,
        default_value = "cycles:u,instructions:u,cache-references:u,cache-misses:u,branches:u,branch-misses:u"
    )]
    pub(crate) perf_events: String,
    /// Intel GPU sampling interval in milliseconds
    #[arg(long, default_value = "100", value_parser = clap::value_parser!(u32).range(10..=60000))]
    pub(crate) gpu_interval_ms: u32,
    /// intel_gpu_top device selector, e.g. drm:/dev/dri/card1
    #[arg(long)]
    pub(crate) gpu_device: Option<String>,
    /// Run named benchmarks from a JSON suite
    #[arg(long, conflicts_with_all = ["program", "executable", "source", "recover", "check_tools"])]
    pub(crate) suite: Option<PathBuf>,
    /// Name stored with this experiment
    #[arg(long)]
    pub(crate) name: Option<String>,
    /// Save a successful experiment as a JSON comparison baseline
    #[arg(long)]
    pub(crate) save_baseline: Option<PathBuf>,
    /// Compare against a saved JSON report
    #[arg(long)]
    pub(crate) compare: Option<PathBuf>,
    /// Allowed worsening, e.g. --regression wall=5 --regression rss=10
    #[arg(long, requires = "compare")]
    pub(crate) regression: Vec<String>,
    /// Compare despite different workloads/hosts (differences remain visible)
    #[arg(long, requires = "compare")]
    pub(crate) allow_mismatch: bool,
    /// Extend measurements until the relative precision target is reached
    #[arg(long)]
    pub(crate) adaptive: bool,
    /// Maximum measured invocations when extending the experiment
    #[arg(long, default_value = "100", value_parser = clap::value_parser!(u32).range(1..))]
    pub(crate) max_runs: u32,
    /// Desired approximate 95% mean interval half-width as percent of mean
    #[arg(long, default_value = "5", value_parser = positive)]
    pub(crate) precision: f64,
    /// Minimum total measured wall time in seconds (across invocations)
    #[arg(long, value_parser = positive)]
    pub(crate) min_time: Option<f64>,
    /// Pin target processes to a CPU list, e.g. 0,2-3
    #[arg(long)]
    pub(crate) cpu_affinity: Option<String>,
    /// Show run progress, elapsed time, and estimated remaining work on stderr
    #[arg(long)]
    pub(crate) progress: bool,
    /// Append durable completed-run records to a new JSONL journal
    #[arg(long, conflicts_with = "recover")]
    pub(crate) checkpoint: Option<PathBuf>,
    /// Recover a report from a JSONL journal without executing the target
    #[arg(long, conflicts_with_all = ["program", "executable", "source", "suite", "check_tools"])]
    pub(crate) recover: Option<PathBuf>,
    /// Check installed optional tools and counter access without a target
    #[arg(long, conflicts_with_all = ["program", "executable", "source", "suite", "recover"])]
    pub(crate) check_tools: bool,
    /// Extra CPU sampling pass with function hotspots and a flamegraph
    #[arg(long)]
    pub(crate) hotspots: bool,
    /// List the 10 domain profiles, 30 focus flags and application metric keys
    #[arg(long)]
    pub(crate) list_profiles: bool,
    /// Fail when any selected domain metric is unavailable
    #[arg(long)]
    pub(crate) require_profile_data: bool,
    /// Presented-frame budget in milliseconds for game/mobile telemetry
    #[arg(long, default_value = "16.666667", value_parser = positive)]
    pub(crate) frame_budget_ms: f64,
    /// Control-cycle deadline in milliseconds for embedded telemetry
    #[arg(long, value_parser = positive)]
    pub(crate) deadline_ms: Option<f64>,
    #[arg(skip)]
    pub(crate) profile_flags: Vec<String>,
    /// Arguments passed verbatim to the target after --
    #[arg(last = true)]
    pub(crate) args: Vec<OsString>,
}
