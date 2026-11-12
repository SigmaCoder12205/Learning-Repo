//! Executable entry point; implementation lives in focused modules.
mod accounts;
mod app;
mod benchmark;
mod cli;
mod comparison;
mod diagnostics;
mod experiment;
mod history;
mod hotspots;
mod html;
mod insights;
mod metrics;
mod profiles;
mod python;
mod report;
mod runner;
mod runtime;
mod stats;
mod storage;
mod suite;

fn main() {
    let code = match app::run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("bench: {error}");
            2
        }
    };
    std::process::exit(code);
}
