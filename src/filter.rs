//! Log filtering based on level and patterns

use crate::log_parser::{ApiLogEntry, LogLevel};
use regex::Regex;

/// Filter configuration for log entries
pub struct LogFilter {
    pub level: Option<LogLevel>,
    pub include_patterns: Vec<Regex>,
    pub exclude_patterns: Vec<Regex>,
}

impl LogFilter {
    /// Create a new filter from configuration
    pub fn new(level: &str, include: &[String], exclude: &[String]) -> Self {
        let level = match level.to_uppercase().as_str() {
            "DEBUG" => Some(LogLevel::Debug),
            "INFO" => Some(LogLevel::Info),
            "WARN" => Some(LogLevel::Warn),
            "ERROR" => Some(LogLevel::Error),
            "ALL" | _ => None,
        };

        let include_patterns = include
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        let exclude_patterns = exclude
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        Self {
            level,
            include_patterns,
            exclude_patterns,
        }
    }

    /// Check if an API log entry should be included
    pub fn should_include_api(&self, entry: &ApiLogEntry) -> bool {
        // Check level filter
        if let Some(min_level) = self.level {
            if entry.log_level() < min_level {
                return false;
            }
        }

        // Create searchable text from the entry
        let searchable = format!(
            "{} {} {} {}",
            entry.msg,
            entry.uri.as_deref().unwrap_or(""),
            entry.sql.as_deref().unwrap_or(""),
            entry.err.as_deref().unwrap_or("")
        );

        // Check exclude patterns first
        for pattern in &self.exclude_patterns {
            if pattern.is_match(&searchable) {
                return false;
            }
        }

        // Check include patterns (if any exist, at least one must match)
        if !self.include_patterns.is_empty() {
            return self.include_patterns.iter().any(|p| p.is_match(&searchable));
        }

        true
    }

    /// Check if a raw line should be included
    pub fn should_include_line(&self, line: &str) -> bool {
        // Check exclude patterns first
        for pattern in &self.exclude_patterns {
            if pattern.is_match(line) {
                return false;
            }
        }

        // Check include patterns
        if !self.include_patterns.is_empty() {
            return self.include_patterns.iter().any(|p| p.is_match(line));
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_filter() {
        let filter = LogFilter::new("WARN", &[], &[]);

        let info_log = ApiLogEntry {
            level: "INFO".to_string(),
            msg: "test".to_string(),
            ..Default::default()
        };

        let error_log = ApiLogEntry {
            level: "ERROR".to_string(),
            msg: "test".to_string(),
            ..Default::default()
        };

        assert!(!filter.should_include_api(&info_log));
        assert!(filter.should_include_api(&error_log));
    }

    #[test]
    fn test_exclude_pattern() {
        let filter = LogFilter::new("ALL", &[], &["health.*check".to_string()]);

        let health_log = ApiLogEntry {
            level: "INFO".to_string(),
            msg: "health check".to_string(),
            ..Default::default()
        };

        let normal_log = ApiLogEntry {
            level: "INFO".to_string(),
            msg: "normal request".to_string(),
            ..Default::default()
        };

        assert!(!filter.should_include_api(&health_log));
        assert!(filter.should_include_api(&normal_log));
    }
}

impl Default for ApiLogEntry {
    fn default() -> Self {
        Self {
            time: None,
            level: "INFO".to_string(),
            msg: String::new(),
            request_id: None,
            uri: None,
            method: None,
            status: None,
            latency_human: None,
            sql: None,
            elapsed: None,
            rows_affected: None,
            err: None,
            func: None,
            office_id: None,
            user_id: None,
            request_body: None,
            response_body: None,
            extra: std::collections::HashMap::new(),
        }
    }
}
