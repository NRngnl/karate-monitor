//! Analysis module for test summaries and SQL statistics

use crate::log_parser::ApiLogEntry;
use colored::Colorize;
use std::collections::HashMap;

/// SQL query statistics
pub struct SqlStats {
    pub total_queries: u32,
    pub queries_by_type: HashMap<String, u32>,
    pub total_rows_affected: i64,
    pub error_count: u32,
    pub total_elapsed_ms: f64,
    pub slowest_queries: Vec<SqlQuery>,
}

#[derive(Clone)]
pub struct SqlQuery {
    pub sql: String,
    pub elapsed_ms: f64,
    pub rows_affected: i64,
    pub uri: Option<String>,
}

impl SqlStats {
    pub fn new() -> Self {
        Self {
            total_queries: 0,
            queries_by_type: HashMap::new(),
            total_rows_affected: 0,
            error_count: 0,
            total_elapsed_ms: 0.0,
            slowest_queries: Vec::new(),
        }
    }

    /// Track an SQL query from a log entry
    pub fn track_query(&mut self, entry: &ApiLogEntry) {
        if let Some(sql) = &entry.sql {
            self.total_queries += 1;

            // Parse query type
            let query_type = sql
                .trim()
                .split_whitespace()
                .next()
                .unwrap_or("UNKNOWN")
                .to_uppercase();
            *self.queries_by_type.entry(query_type).or_insert(0) += 1;

            // Track rows affected
            if let Some(rows) = entry.rows_affected {
                self.total_rows_affected += rows;
            }

            // Track errors
            if entry.err.is_some() || entry.level == "ERROR" {
                self.error_count += 1;
            }

            // Parse elapsed time
            let elapsed = parse_elapsed(&entry.elapsed);
            self.total_elapsed_ms += elapsed;

            // Track slowest queries (keep top 5)
            let query = SqlQuery {
                sql: sql.clone(),
                elapsed_ms: elapsed,
                rows_affected: entry.rows_affected.unwrap_or(0),
                uri: entry.uri.clone(),
            };

            self.slowest_queries.push(query);
            self.slowest_queries
                .sort_by(|a, b| b.elapsed_ms.partial_cmp(&a.elapsed_ms).unwrap());
            self.slowest_queries.truncate(5);
        }
    }

    /// Print SQL statistics summary
    pub fn print_summary(&self) {
        if self.total_queries == 0 {
            return;
        }

        println!();
        println!("{}", "📊 SQL Statistics".bright_cyan().bold());
        println!("{}", "─".repeat(40).bright_black());
        println!(
            "  Total Queries: {}",
            self.total_queries.to_string().bright_white()
        );
        println!(
            "  Total Rows Affected: {}",
            self.total_rows_affected.to_string().bright_white()
        );
        println!(
            "  Query Errors: {}",
            if self.error_count > 0 {
                self.error_count.to_string().red()
            } else {
                self.error_count.to_string().green()
            }
        );
        println!(
            "  Total Time: {:.2}ms",
            self.total_elapsed_ms
        );

        if !self.queries_by_type.is_empty() {
            println!();
            println!("  {}", "By Type:".bright_yellow());
            for (query_type, count) in &self.queries_by_type {
                println!("    {}: {}", query_type, count);
            }
        }

        if !self.slowest_queries.is_empty() {
            println!();
            println!("  {}", "Slowest Queries:".bright_yellow());
            for (i, query) in self.slowest_queries.iter().take(5).enumerate() {
                let truncated = if query.sql.len() > 60 {
                    format!("{}...", &query.sql[..60])
                } else {
                    query.sql.clone()
                };
                println!(
                    "    {}. {:.2}ms - {}",
                    i + 1,
                    query.elapsed_ms,
                    truncated.bright_black()
                );
            }
        }
    }
}

impl Default for SqlStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Test summary tracking
pub struct TestSummary {
    pub total_features: u32,
    pub total_scenarios: u32,
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub failed_features: Vec<FailedFeature>,
}

#[derive(Clone)]
pub struct FailedFeature {
    pub feature_file: String,
    pub line_number: Option<u32>,
    pub error_message: String,
    pub url: Option<String>,
}

impl TestSummary {
    pub fn new() -> Self {
        Self {
            total_features: 0,
            total_scenarios: 0,
            passed: 0,
            failed: 0,
            skipped: 0,
            failed_features: Vec::new(),
        }
    }

    /// Update summary from Karate output line
    pub fn update_from_line(&mut self, line: &str) {
        // Parse scenario summary line
        // Example: "scenarios:  2 | passed:  1 | failed:  1 | time: 0.4675"
        if let Some(result) = crate::log_parser::parse_karate_summary(line) {
            self.total_scenarios = result.total_scenarios;
            self.passed = result.passed;
            self.failed = result.failed;
        }

        // Parse feature count
        // Example: "features:     1 | skipped:    0 | efficiency: 0.33"
        if line.contains("features:") {
            if let Some(count) = extract_number_after("features:", line) {
                self.total_features = count;
            }
            if let Some(count) = extract_number_after("skipped:", line) {
                self.skipped = count;
            }
        }
    }

    /// Track a failed feature
    pub fn track_failure(&mut self, feature: &str, error: &str, url: Option<String>) {
        // Extract line number from feature string (e.g., "file.feature:40")
        let (file, line_num) = if let Some(pos) = feature.rfind(':') {
            let line = feature[pos + 1..].parse().ok();
            (feature[..pos].to_string(), line)
        } else {
            (feature.to_string(), None)
        };

        self.failed_features.push(FailedFeature {
            feature_file: file,
            line_number: line_num,
            error_message: error.to_string(),
            url,
        });
    }

    /// Print test summary
    pub fn print_summary(&self) {
        println!();
        println!("{}", "🥋 Test Summary".bright_cyan().bold());
        println!("{}", "─".repeat(40).bright_black());

        println!(
            "  Features: {}",
            self.total_features.to_string().bright_white()
        );
        println!(
            "  Scenarios: {} total, {} passed, {} failed",
            self.total_scenarios.to_string().bright_white(),
            self.passed.to_string().green(),
            if self.failed > 0 {
                self.failed.to_string().red().bold()
            } else {
                self.failed.to_string().green()
            }
        );

        if !self.failed_features.is_empty() {
            println!();
            println!("  {}", "Failed Tests:".red().bold());
            for failure in &self.failed_features {
                println!(
                    "    {} {}{}",
                    "✗".red(),
                    failure.feature_file.bright_white(),
                    failure
                        .line_number
                        .map(|n| format!(":{}", n))
                        .unwrap_or_default()
                        .bright_black()
                );
                if !failure.error_message.is_empty() {
                    let truncated = if failure.error_message.len() > 80 {
                        format!("{}...", &failure.error_message[..80])
                    } else {
                        failure.error_message.clone()
                    };
                    println!("      {}", truncated.bright_black());
                }
            }
        }
    }
}

impl Default for TestSummary {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse elapsed time string like "1.235ms" to milliseconds
fn parse_elapsed(elapsed: &Option<String>) -> f64 {
    elapsed
        .as_ref()
        .and_then(|e| {
            let s = e.trim_end_matches("ms").trim_end_matches("s");
            s.parse::<f64>().ok()
        })
        .unwrap_or(0.0)
}

/// Extract a number after a label in a line
fn extract_number_after(label: &str, line: &str) -> Option<u32> {
    let pos = line.find(label)?;
    let rest = &line[pos + label.len()..];
    let trimmed = rest.trim_start();
    let num_str: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
    num_str.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_elapsed() {
        assert_eq!(parse_elapsed(&Some("1.235ms".to_string())), 1.235);
        assert_eq!(parse_elapsed(&Some("0.5ms".to_string())), 0.5);
        assert_eq!(parse_elapsed(&None), 0.0);
    }

    #[test]
    fn test_extract_number_after() {
        let line = "features:     1 | skipped:    0 | efficiency: 0.33";
        assert_eq!(extract_number_after("features:", line), Some(1));
        assert_eq!(extract_number_after("skipped:", line), Some(0));
    }
}
