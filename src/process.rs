//! Process management for API and Karate test execution

use crate::analysis::{SqlStats, TestSummary};
use crate::config::Config;
use crate::correlation::RequestCorrelator;
use crate::export::{ExportFormat, LogExporter};
use crate::filter::LogFilter;
use crate::formatter::LogFormatter;
use crate::log_parser::{
    extract_failure_url, parse_karate_line, ApiLogEntry, LogType,
};
use colored::Colorize;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

/// Manages API and Karate test processes
pub struct ProcessManager {
    config: Config,
    correlator: Arc<Mutex<RequestCorrelator>>,
    sql_stats: Arc<Mutex<SqlStats>>,
    test_summary: Arc<Mutex<TestSummary>>,
    formatter: LogFormatter,
    filter: LogFilter,
    exporter: Option<LogExporter>,
}

impl ProcessManager {
    pub fn new(
        config: Config,
        correlator: Arc<Mutex<RequestCorrelator>>,
        sql_stats: Arc<Mutex<SqlStats>>,
        test_summary: Arc<Mutex<TestSummary>>,
    ) -> Self {
        let formatter = LogFormatter::new(config.display.clone());
        let filter = LogFilter::new(
            &config.logging.level,
            &config.logging.include_patterns,
            &config.logging.exclude_patterns,
        );

        let exporter = LogExporter::new(
            &config.logging.export_path,
            ExportFormat::from_str(&config.logging.export_format),
        )
        .ok()
        .flatten();

        Self {
            config,
            correlator,
            sql_stats,
            test_summary,
            formatter,
            filter,
            exporter,
        }
    }

    /// Run the full test suite
    pub async fn run(&mut self, test_paths: &[String]) -> Result<i32, Box<dyn std::error::Error>> {
        // Start the API server
        println!(
            "{} Starting API server: {}",
            "🚀".bright_green(),
            self.config.api.command.bright_yellow()
        );

        let mut api_process = self.start_api().await?;

        // Set up signal handling for cleanup
        let api_pid = api_process.id();

        // Wait for API to be ready
        if !self.wait_for_api().await {
            eprintln!("{} API failed to start", "❌".red());
            let _ = api_process.kill().await;
            return Ok(1);
        }

        println!("{} API is ready", "✅".green());
        println!();

        // Start processing API logs in background
        let api_stdout = api_process.stdout.take();
        let api_stderr = api_process.stderr.take();

        let correlator_clone = self.correlator.clone();
        let sql_stats_clone = self.sql_stats.clone();
        let config_clone = self.config.clone();
        let formatter_clone = LogFormatter::new(self.config.display.clone());
        let filter_clone = LogFilter::new(
            &self.config.logging.level,
            &self.config.logging.include_patterns,
            &self.config.logging.exclude_patterns,
        );

        // Spawn API stdout handler
        let stdout_handle = if let Some(stdout) = api_stdout {
            Some(tokio::spawn(async move {
                process_api_output(
                    stdout,
                    correlator_clone,
                    sql_stats_clone,
                    &config_clone,
                    formatter_clone,
                    filter_clone,
                )
                .await
            }))
        } else {
            None
        };

        // Spawn API stderr handler
        let stderr_handle = if let Some(stderr) = api_stderr {
            Some(tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    eprintln!("{} {}", "❌🔷".red(), line.red());
                }
            }))
        } else {
            None
        };

        // Run Karate tests
        let exit_code = self.run_karate(test_paths).await?;

        // Clean up API process
        println!();
        println!(
            "{} Stopping API (pid {:?})…",
            "ℹ️".bright_blue(),
            api_pid
        );
        let _ = api_process.kill().await;

        // Wait for log handlers to finish
        if let Some(handle) = stdout_handle {
            let _ = handle.await;
        }
        if let Some(handle) = stderr_handle {
            let _ = handle.await;
        }

        // Finalize export
        if let Some(exporter) = self.exporter.take() {
            let _ = exporter.finish();
        }

        Ok(exit_code)
    }

    /// Start the API server process
    async fn start_api(&self) -> Result<Child, Box<dyn std::error::Error>> {
        let child = Command::new(&self.config.api.command)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        Ok(child)
    }

    /// Wait for the API to become healthy
    async fn wait_for_api(&self) -> bool {
        let timeout = self.config.api.health_timeout_secs;
        let interval = self.config.api.health_interval_secs;

        for i in 1..=timeout {
            match reqwest_health_check(&self.config.api.health_url).await {
                Ok(true) => return true,
                _ => {
                    println!(
                        "{} waiting for API ({}/{})…",
                        "⏳".bright_yellow(),
                        i,
                        timeout
                    );
                    sleep(Duration::from_secs(interval)).await;
                }
            }
        }

        false
    }

    /// Run Karate tests
    async fn run_karate(&mut self, test_paths: &[String]) -> Result<i32, Box<dyn std::error::Error>> {
        // Build classpath
        let classpath = std::iter::once(self.config.karate.jar_path.clone())
            .chain(self.config.karate.classpath.iter().cloned())
            .collect::<Vec<_>>()
            .join(":");

        // Build command
        let mut cmd = Command::new("java");
        cmd.arg("-cp")
            .arg(&classpath)
            .arg("com.intuit.karate.Main")
            .arg("-T")
            .arg(self.config.karate.threads.to_string())
            .arg("-f")
            .arg(&self.config.karate.output_format)
            .arg("-o")
            .arg(&self.config.karate.report_dir);

        for path in test_paths {
            cmd.arg(path);
        }

        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir("/app");

        println!(
            "{} Running Karate tests: {}",
            "🥋".bright_cyan(),
            test_paths.join(", ").bright_yellow()
        );
        println!();

        let mut child = cmd.spawn()?;

        // Process Karate output
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let correlator_for_karate = self.correlator.clone();
        let test_summary_clone = self.test_summary.clone();
        let failed_only = self.config.analysis.failed_only;
        let formatter = LogFormatter::new(self.config.display.clone());

        // Process stdout
        if let Some(stdout) = stdout {
            let mut reader = BufReader::new(stdout).lines();
            let mut pending_failure_url: Option<String> = None;
            let mut current_feature: Option<String> = None;
            
        // Buffer for batch logs to group them (raw_line, parsed_entry)
            let mut batch_buffer: Vec<(String, Option<ApiLogEntry>)> = Vec::new();

            while let Ok(Some(line)) = reader.next_line().await {
                // Check if this is a batch log line
                if line.contains("📦") {
                    // Try to extract and parse JSON part
                    let mut parsed_entry = None;
                    let mut log_content = line.clone();

                    if let Some(start_idx) = line.find('{') {
                        let json_part = &line[start_idx..];
                        if let Some(entry) = ApiLogEntry::parse(json_part) {
                            parsed_entry = Some(entry);
                            log_content = json_part.to_string();
                        }
                    }
                    
                    batch_buffer.push((log_content, parsed_entry));
                    continue; // Don't print yet, wait for group end
                }

                // If we have buffered batch logs and now see a non-batch line, print the buffer
                if !batch_buffer.is_empty() {
                    // Find first request_id available in the batch
                    let request_id = batch_buffer.iter()
                        .find_map(|(_, entry)| entry.as_ref().and_then(|e| e.request_id.clone()))
                        .unwrap_or_else(|| "Batch Job".to_string());

                    println!("{}", formatter.format_custom_header("Captured Batch Logs", &request_id));
                    for (content, entry) in &batch_buffer {
                        if let Some(e) = entry {
                            println!("  {} {}", "📦", formatter.format_api_log(e, content));
                        } else {
                            // Print non-JSON batch logs simply
                            println!("  {} {}", "📦".bright_blue(), content.trim());
                        }
                    }
                    println!("{}", formatter.format_correlated_footer());
                    batch_buffer.clear();
                }

                let log_type = parse_karate_line(&line);

                // Track current feature file name
                // Example: "feature: ../tests/fetch_perio_chart.feature"
                if line.contains("feature:") && line.contains(".feature") {
                    if let Some(start) = line.find("feature:") {
                        let feature_part = line[start + 8..].trim();
                        current_feature = Some(feature_part.to_string());
                    }
                }

                // Check for failure URL
                if let Some(url) = extract_failure_url(&line) {
                    pending_failure_url = Some(url);
                }

                // Track test summary
                if log_type == LogType::KarateSummary {
                    let mut summary = test_summary_clone.lock().await;
                    summary.update_from_line(&line);
                }

                // In failed-only mode, we need to correlate and show API logs
                if log_type == LogType::KarateFailure || line.contains("failed features:") {
                    let correlator = correlator_for_karate.lock().await;
                    let mut showed_logs = false;

                    // Try to find logs by URL if we have one
                    if let Some(ref url) = pending_failure_url {
                        if let Some((request_id, logs)) = correlator.find_matching_logs_by_url(url) {
                            println!("{}", formatter.format_correlated_header(request_id));
                            for (raw_json, entry) in logs {
                                println!("  {}", formatter.format_api_log(entry, raw_json));
                            }
                            println!("{}", formatter.format_correlated_footer());
                            showed_logs = true;
                        }
                        pending_failure_url = None;
                    }

                    // Fallback: if no URL-based logs, show all logs from most recent request
                    if !showed_logs && failed_only {
                        if let Some((request_id, logs)) = correlator.get_last_request_logs(100) {
                            println!("\n{}", formatter.format_correlated_header(request_id));
                            for (raw_json, entry) in logs {
                                println!("  {}", formatter.format_api_log(entry, raw_json));
                            }
                            println!("{}", formatter.format_correlated_footer());
                        }
                    }
                }

                // Determine what to show based on mode
                if failed_only {
                    // Per-feature scenario summary (combine with feature name)
                    // Example: "scenarios:  4 | passed:  4 | failed:  0 | time: 1.0661"
                    if line.contains("scenarios:") && line.contains("passed:") && line.contains("time:") && !line.contains("threads") {
                        let prefix = if line.contains("failed:  0") || line.contains("failed: 0") {
                            format!("{}", "✅".green())
                        } else {
                            format!("{}", "❌".red())
                        };
                        
                        let feature_name = current_feature.as_deref().unwrap_or("unknown");
                        // Extract just the filename from path
                        let short_name = feature_name.rsplit('/').next().unwrap_or(feature_name);
                        
                        println!("{} {} {}", prefix, short_name.bright_white(), line.trim());
                    }
                    // Show failures
                    else if log_type == LogType::KarateFailure {
                        let formatted = formatter.format_karate_log(&line, &log_type);
                        println!("{}", formatted);
                    }
                    // Show failed features details (the >>> block)
                    else if line.contains(">>> failed features:") {
                        let formatted = formatter.format_karate_log(&line, &log_type);
                        println!("{}", formatted);
                    }
                    // Skip everything else (final summary is handled by TestSummary)
                } else {
                    // Normal mode: show everything
                    let formatted = formatter.format_karate_log(&line, &log_type);
                    println!("{}", formatted);
                }
            }
            
            // Flush any remaining batch logs at the end
            if !batch_buffer.is_empty() {
                let request_id = batch_buffer.iter()
                    .find_map(|(_, entry)| entry.as_ref().and_then(|e| e.request_id.clone()))
                    .unwrap_or_else(|| "Batch Job".to_string());

                println!("{}", formatter.format_custom_header("Captured Batch Logs", &request_id));
                for (content, entry) in &batch_buffer {
                    if let Some(e) = entry {
                        println!("  {} {}", "📦", formatter.format_api_log(e, content));
                    } else {
                        println!("  {} {}", "📦".bright_blue(), content.trim());
                    }
                }
                println!("{}", formatter.format_correlated_footer());
            }
        }

        // Process stderr
        if let Some(stderr) = stderr {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                eprintln!("{} {}", "❌🔶".red(), line.red());
            }
        }

        // Wait for process to complete
        let status = child.wait().await?;
        let exit_code = status.code().unwrap_or(1);

        Ok(exit_code)
    }
}

/// Process API output stream
async fn process_api_output(
    stdout: tokio::process::ChildStdout,
    correlator: Arc<Mutex<RequestCorrelator>>,
    sql_stats: Arc<Mutex<SqlStats>>,
    config: &Config,
    formatter: LogFormatter,
    filter: LogFilter,
) {
    let mut reader = BufReader::new(stdout).lines();

    while let Ok(Some(line)) = reader.next_line().await {
        // Try to parse as JSON
        if let Some(entry) = ApiLogEntry::parse(&line) {
            // Track SQL statistics
            if config.analysis.track_sql && entry.sql.is_some() {
                let mut stats = sql_stats.lock().await;
                stats.track_query(&entry);
            }

            // Buffer for correlation (in failed-only mode)
            if config.analysis.failed_only {
                let mut corr = correlator.lock().await;
                corr.buffer_api_log(line.clone(), entry.clone());
            }

            // Apply filter and format
            if !config.analysis.failed_only && filter.should_include_api(&entry) {
                let formatted = formatter.format_api_log(&entry, &line);
                println!("{}", formatted);
            }
        } else {
            // Non-JSON line, print as-is if not in failed-only mode
            if !config.analysis.failed_only {
                println!("{} {}", "🔷".bright_blue(), line);
            }
        }
    }
}

/// Simple health check using TCP connection (to avoid reqwest dependency)
async fn reqwest_health_check(url: &str) -> Result<bool, Box<dyn std::error::Error>> {
    // Parse URL to get host and port
    let url = url::Url::parse(url)?;
    let host = url.host_str().unwrap_or("localhost");
    let port = url.port().unwrap_or(1323);

    // Try to connect
    match tokio::net::TcpStream::connect(format!("{}:{}", host, port)).await {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}
