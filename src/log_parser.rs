//! Log parser for API JSON logs and Karate test output

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a parsed log entry from the Go API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiLogEntry {
    #[serde(default)]
    pub time: Option<String>,
    #[serde(default)]
    pub level: String,
    #[serde(default)]
    pub msg: String,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub status: Option<u16>,
    #[serde(default)]
    pub latency_human: Option<String>,
    #[serde(default)]
    pub sql: Option<String>,
    #[serde(default)]
    pub elapsed: Option<String>,
    #[serde(default)]
    pub rows_affected: Option<i64>,
    #[serde(default)]
    pub err: Option<String>,
    #[serde(default)]
    pub func: Option<String>,
    #[serde(default)]
    pub office_id: Option<i64>,
    #[serde(default)]
    pub user_id: Option<i64>,
    #[serde(default)]
    pub request_body: Option<serde_json::Value>,
    #[serde(default)]
    pub response_body: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Log level enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "DEBUG" => LogLevel::Debug,
            "INFO" => LogLevel::Info,
            "WARN" | "WARNING" => LogLevel::Warn,
            "ERROR" => LogLevel::Error,
            _ => LogLevel::Info,
        }
    }
}

/// Represents a parsed Karate test result line
#[derive(Debug, Clone)]
pub struct KarateTestResult {
    pub total_scenarios: u32,
    pub passed: u32,
    pub failed: u32,
}

/// Represents failed test information extracted from Karate output
#[derive(Debug, Clone)]
pub struct KarateFailure {
    pub feature_file: String,
    pub line_number: u32,
    pub assertion: String,
    pub url: Option<String>,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub response: Option<String>,
}

/// Identifies the type of log entry
#[derive(Debug, Clone, PartialEq)]
pub enum LogType {
    ApiRequest,
    ApiSql,
    ApiError,
    ApiBodyDump,
    ApiGeneral,
    KarateScenarioStart,
    KarateScenarioEnd,
    KarateFailure,
    KarateInfo,
    KarateSummary,
}

impl ApiLogEntry {
    /// Parse a JSON line into an ApiLogEntry
    pub fn parse(line: &str) -> Option<Self> {
        serde_json::from_str(line).ok()
    }

    /// Get the log level
    pub fn log_level(&self) -> LogLevel {
        LogLevel::from_str(&self.level)
    }

    /// Determine the type of log entry
    pub fn log_type(&self) -> LogType {
        if self.msg.contains("SQL") || self.sql.is_some() {
            if self.level == "ERROR" || self.err.is_some() {
                return LogType::ApiError;
            }
            return LogType::ApiSql;
        }

        if self.msg.contains("request / response body dump") {
            return LogType::ApiBodyDump;
        }

        if self.msg == "REQUEST" {
            return LogType::ApiRequest;
        }

        if self.level == "ERROR" {
            return LogType::ApiError;
        }

        LogType::ApiGeneral
    }

    /// Check if this is a final request log (has status)
    pub fn is_request_summary(&self) -> bool {
        self.msg == "REQUEST" && self.status.is_some()
    }

    /// Get the full URI with query string for matching
    pub fn get_full_uri(&self) -> Option<String> {
        self.uri.clone()
    }

    /// Parse the timestamp
    pub fn parse_time(&self) -> Option<DateTime<Utc>> {
        self.time.as_ref().and_then(|t| t.parse().ok())
    }
}

/// Parse Karate output line
pub fn parse_karate_line(line: &str) -> LogType {
    let trimmed = line.trim();

    // Check for scenario result summary
    if trimmed.contains("scenarios:") && trimmed.contains("passed:") && trimmed.contains("failed:") {
        return LogType::KarateSummary;
    }

    // Check for failure indicators
    if trimmed.contains("status code was:") && trimmed.contains("expected:") {
        return LogType::KarateFailure;
    }

    if trimmed.starts_with("Scenario:") || trimmed.contains(".feature:") {
        if trimmed.contains("failed") {
            return LogType::KarateFailure;
        }
        return LogType::KarateScenarioStart;
    }

    // Check for test summary lines
    if trimmed.starts_with("features:") || trimmed.starts_with("elapsed:") {
        return LogType::KarateSummary;
    }

    LogType::KarateInfo
}

/// Extract test results from Karate summary line
/// Example: "scenarios:  2 | passed:  1 | failed:  1 | time: 0.4675"
pub fn parse_karate_summary(line: &str) -> Option<KarateTestResult> {
    let re = regex::Regex::new(r"scenarios:\s*(\d+)\s*\|\s*passed:\s*(\d+)\s*\|\s*failed:\s*(\d+)")
        .ok()?;

    let caps = re.captures(line)?;

    Some(KarateTestResult {
        total_scenarios: caps.get(1)?.as_str().parse().ok()?,
        passed: caps.get(2)?.as_str().parse().ok()?,
        failed: caps.get(3)?.as_str().parse().ok()?,
    })
}

/// Extract URL from Karate failure line
/// Example: "status code was: 200, expected: 400, response time in milliseconds: 6, url: http://localhost:1323/api/v1/karte/outcome?patientID=1"
pub fn extract_failure_url(line: &str) -> Option<String> {
    let re = regex::Regex::new(r"url:\s*(https?://[^\s,]+)").ok()?;
    let caps = re.captures(line)?;
    Some(caps.get(1)?.as_str().to_string())
}

/// Extract path and query from full URL
/// Example: "http://localhost:1323/api/v1/karte/outcome?patientID=1" -> "/api/v1/karte/outcome?patientID=1"
pub fn extract_path_query(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let path = parsed.path();
    let query = parsed.query();

    match query {
        Some(q) => Some(format!("{}?{}", path, q)),
        None => Some(path.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_api_log() {
        let json = r#"{"time":"2025-12-16T08:48:57.008508381Z","level":"INFO","msg":"REQUEST","request_id":"abc123","uri":"/api/v1/test","status":200}"#;
        let entry = ApiLogEntry::parse(json).unwrap();
        assert_eq!(entry.level, "INFO");
        assert_eq!(entry.msg, "REQUEST");
        assert_eq!(entry.request_id.as_deref(), Some("abc123"));
    }

    #[test]
    fn test_parse_karate_summary() {
        let line = "scenarios:  2 | passed:  1 | failed:  1 | time: 0.4675";
        let result = parse_karate_summary(line).unwrap();
        assert_eq!(result.total_scenarios, 2);
        assert_eq!(result.passed, 1);
        assert_eq!(result.failed, 1);
    }

    #[test]
    fn test_extract_failure_url() {
        let line = "status code was: 200, expected: 400, response time in milliseconds: 6, url: http://localhost:1323/api/v1/karte/outcome?patientID=1, response:";
        let url = extract_failure_url(line).unwrap();
        assert_eq!(url, "http://localhost:1323/api/v1/karte/outcome?patientID=1");
    }

    #[test]
    fn test_extract_path_query() {
        let url = "http://localhost:1323/api/v1/karte/outcome?patientID=1";
        let path = extract_path_query(url).unwrap();
        assert_eq!(path, "/api/v1/karte/outcome?patientID=1");
    }
}
