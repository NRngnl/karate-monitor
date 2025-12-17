//! Colored output formatting for logs

use crate::config::DisplayConfig;
use crate::log_parser::{ApiLogEntry, LogLevel, LogType};
use colored::Colorize;

/// Formatter for log output
pub struct LogFormatter {
    config: DisplayConfig,
    show_timestamps: bool,
}

impl LogFormatter {
    pub fn new(config: DisplayConfig) -> Self {
        Self {
            show_timestamps: config.show_timestamps,
            config,
        }
    }

    /// Format an API log entry with colors and prefixes
    pub fn format_api_log(&self, entry: &ApiLogEntry, raw_json: &str) -> String {
        let log_type = entry.log_type();
        let prefix = self.get_api_prefix(&log_type, entry);

        let formatted = match log_type {
            LogType::ApiError => self.format_error_log(raw_json),
            LogType::ApiSql => self.format_sql_log(raw_json, entry),
            LogType::ApiBodyDump => self.format_body_dump(raw_json),
            LogType::ApiRequest => self.format_request_log(raw_json, entry),
            _ => self.format_general_log(raw_json),
        };

        format!("{}{} {}", prefix, self.config.api_prefix, formatted)
    }

    /// Format a Karate log line
    pub fn format_karate_log(&self, line: &str, log_type: &LogType) -> String {
        let prefix = self.get_karate_prefix(log_type, line);
        let formatted = match log_type {
            LogType::KarateFailure => line.red().to_string(),
            LogType::KarateSummary => {
                if line.contains("failed:") && !line.contains("failed: 0") && !line.contains("failed:  0") {
                    line.red().bold().to_string()
                } else if line.contains("passed:") {
                    line.green().to_string()
                } else {
                    line.bright_white().to_string()
                }
            }
            _ => line.bright_white().to_string(),
        };

        format!("{}{} {}", prefix, self.config.karate_prefix, formatted)
    }

    fn get_api_prefix(&self, log_type: &LogType, entry: &ApiLogEntry) -> String {
        match log_type {
            LogType::ApiError => format!("{}", self.config.error_prefix),
            LogType::ApiSql => {
                if entry.err.is_some() || entry.level == "ERROR" {
                    format!("{}", self.config.sql_prefix)
                } else {
                    format!("{}", self.config.sql_prefix)
                }
            }
            LogType::ApiBodyDump => "📄".to_string(),
            _ => {
                if entry.log_level() == LogLevel::Error {
                    format!("{}", self.config.error_prefix)
                } else {
                    format!("{}", self.config.success_prefix)
                }
            }
        }
    }

    fn get_karate_prefix(&self, log_type: &LogType, line: &str) -> String {
        match log_type {
            LogType::KarateFailure => format!("{}", self.config.error_prefix),
            LogType::KarateSummary => {
                if line.contains("failed:") && !line.contains("failed: 0") && !line.contains("failed:  0") {
                    format!("{}", self.config.error_prefix)
                } else {
                    format!("{}", self.config.success_prefix)
                }
            }
            _ => "🔶".to_string(),
        }
    }

    fn format_error_log(&self, json: &str) -> String {
        json.red().to_string()
    }

    fn format_sql_log(&self, json: &str, entry: &ApiLogEntry) -> String {
        // Highlight SQL query in yellow, rows_affected in green
        let mut result = json.bright_blue().to_string();

        if let Some(sql) = &entry.sql {
            let highlighted_sql = sql.bright_yellow().to_string();
            result = result.replace(sql, &highlighted_sql);
        }

        if let Some(rows) = entry.rows_affected {
            let rows_str = format!("\"rows_affected\":{}", rows);
            let highlighted = rows_str.bright_green().bold().to_string();
            result = result.replace(&rows_str, &highlighted);
        }

        if entry.err.is_some() || entry.level == "ERROR" {
            result = json.red().to_string();
        }

        result
    }

    fn format_body_dump(&self, json: &str) -> String {
        json.green().to_string()
    }

    fn format_request_log(&self, json: &str, entry: &ApiLogEntry) -> String {
        let mut result = json.bright_white().dimmed().to_string();

        // Highlight status code based on value
        if let Some(status) = entry.status {
            let status_str = format!("\"status\":{}", status);
            let highlighted = if status >= 400 {
                status_str.red().bold().to_string()
            } else {
                status_str.green().to_string()
            };
            result = result.replace(&status_str, &highlighted);
        }

        result
    }

    fn format_general_log(&self, json: &str) -> String {
        // Highlight msg field in cyan
        json.bright_white().dimmed().to_string()
    }

    /// Format a separator line
    pub fn format_separator(&self) -> String {
        "─".repeat(60).bright_black().to_string()
    }

    /// Format a failure header
    pub fn format_failure_header(&self, feature: &str) -> String {
        format!(
            "\n{} {} {}\n{}",
            "╔".red(),
            format!("FAILED: {}", feature).red().bold(),
            "╗".red(),
            "╚".red()
        )
    }

    /// Format correlated logs header
    pub fn format_correlated_header(&self, request_id: &str) -> String {
        format!(
            "\n{} {}: {} {}\n",
            "┌─".bright_yellow(),
            "Related API Logs".bright_yellow().bold(),
            request_id.bright_cyan(),
            "─┐".bright_yellow()
        )
    }

    /// Format correlated logs footer
    pub fn format_correlated_footer(&self) -> String {
        format!("{}\n", "└───────────────────────────────────────┘".bright_yellow())
    }

    /// Format a custom header with a specific title
    pub fn format_custom_header(&self, title: &str, id: &str) -> String {
        format!(
            "\n{} {}: {} {}\n",
            "┌─".bright_yellow(),
            title.bright_yellow().bold(),
            id.bright_cyan(),
            "─┐".bright_yellow()
        )
    }
}
