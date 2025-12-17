//! Karate Monitor - E2E Test Monitoring and Log Analysis Tool
//!
//! This tool replaces the shell-based log processing for Karate E2E tests,
//! providing configurable log filtering, test result summaries, SQL analysis,
//! and log persistence.

mod analysis;
mod config;
mod correlation;
mod export;
mod filter;
mod formatter;
mod log_parser;
mod process;

use clap::Parser;
use colored::Colorize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use config::Config;
use correlation::RequestCorrelator;
use process::ProcessManager;

/// Karate E2E Test Monitor
#[derive(Parser, Debug)]
#[command(name = "karate-monitor")]
#[command(version, about = "Monitor and analyze Karate E2E tests")]
struct Args {
    /// Path to configuration file (TOML or JSON)
    #[arg(short, long, default_value = "/app/karate-monitor.toml")]
    config: PathBuf,

    /// Log level filter (overrides config): DEBUG, INFO, WARN, ERROR, ALL
    #[arg(short, long)]
    level: Option<String>,

    /// Include pattern (regex, can be specified multiple times)
    #[arg(long)]
    include: Vec<String>,

    /// Exclude pattern (regex, can be specified multiple times)
    #[arg(long)]
    exclude: Vec<String>,

    /// Disable colors (for CI environments)
    #[arg(long)]
    no_color: bool,

    /// Export logs to file
    #[arg(long)]
    export: Option<PathBuf>,

    /// Show SQL statistics at the end
    #[arg(long)]
    sql_stats: bool,

    /// Only show logs for failed scenarios (correlates API logs by request_id)
    #[arg(long)]
    failed_only: bool,

    /// Test paths to run (defaults to /tests)
    #[arg(trailing_var_arg = true)]
    tests: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Disable colors if requested or if NO_COLOR env is set
    if args.no_color || std::env::var("NO_COLOR").is_ok() {
        colored::control::set_override(false);
    }

    // Load configuration
    let mut config = if args.config.exists() {
        Config::load(&args.config)?
    } else {
        eprintln!(
            "{} Config file not found at {:?}, using defaults",
            "⚠️".yellow(),
            args.config
        );
        Config::default()
    };

    // Apply CLI overrides
    if let Some(level) = &args.level {
        config.logging.level = level.clone();
    }
    if !args.include.is_empty() {
        config.logging.include_patterns = args.include.clone();
    }
    if !args.exclude.is_empty() {
        config.logging.exclude_patterns = args.exclude.clone();
    }
    if args.sql_stats {
        config.analysis.show_sql_stats = true;
    }
    if args.failed_only {
        config.analysis.failed_only = true;
    }
    if let Some(export_path) = &args.export {
        config.logging.export_path = export_path.to_string_lossy().to_string();
    }

    // Determine test paths
    let test_paths: Vec<String> = if args.tests.is_empty() {
        vec![config.karate.default_test_path.clone()]
    } else {
        args.tests.clone()
    };

    println!("{}", "═".repeat(60).bright_blue());
    println!(
        "{} {}",
        "🥋".bright_yellow(),
        "Karate Monitor v0.1.0".bright_white().bold()
    );
    println!("{}", "═".repeat(60).bright_blue());
    println!();

    // Create shared state
    let correlator = Arc::new(Mutex::new(RequestCorrelator::new()));
    let sql_stats = Arc::new(Mutex::new(analysis::SqlStats::new()));
    let test_summary = Arc::new(Mutex::new(analysis::TestSummary::new()));

    // Create process manager
    let mut process_manager = ProcessManager::new(
        config.clone(),
        correlator.clone(),
        sql_stats.clone(),
        test_summary.clone(),
    );

    // Run the test suite
    let exit_code = process_manager.run(&test_paths).await?;

    // Print summaries
    println!();
    println!("{}", "═".repeat(60).bright_blue());

    if config.analysis.show_sql_stats {
        let stats = sql_stats.lock().await;
        stats.print_summary();
    }

    if config.analysis.show_test_summary {
        let summary = test_summary.lock().await;
        summary.print_summary();
    }

    println!("{}", "═".repeat(60).bright_blue());

    std::process::exit(exit_code);
}
