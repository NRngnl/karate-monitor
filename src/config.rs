//! Configuration module for loading TOML/JSON config files

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Failed to parse TOML: {0}")]
    TomlError(#[from] toml::de::Error),
    #[error("Failed to parse JSON: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Unsupported config format: {0}")]
    UnsupportedFormat(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub karate: KarateConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub analysis: AnalysisConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    #[serde(default = "default_api_command")]
    pub command: String,
    #[serde(default = "default_health_url")]
    pub health_url: String,
    #[serde(default = "default_health_timeout")]
    pub health_timeout_secs: u64,
    #[serde(default = "default_health_interval")]
    pub health_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KarateConfig {
    #[serde(default = "default_jar_path")]
    pub jar_path: String,
    #[serde(default = "default_classpath")]
    pub classpath: Vec<String>,
    #[serde(default = "default_threads")]
    pub threads: u32,
    #[serde(default = "default_output_format")]
    pub output_format: String,
    #[serde(default = "default_report_dir")]
    pub report_dir: String,
    #[serde(default = "default_test_path")]
    pub default_test_path: String,
    #[serde(default)]
    pub use_compact_object_headers: bool,
    #[serde(default)]
    pub use_zgc: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_level")]
    pub level: String,
    #[serde(default)]
    pub include_patterns: Vec<String>,
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    #[serde(default = "default_true")]
    pub colors: bool,
    #[serde(default)]
    pub export_path: String,
    #[serde(default = "default_export_format")]
    pub export_format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    #[serde(default = "default_api_prefix")]
    pub api_prefix: String,
    #[serde(default = "default_karate_prefix")]
    pub karate_prefix: String,
    #[serde(default = "default_sql_prefix")]
    pub sql_prefix: String,
    #[serde(default = "default_error_prefix")]
    pub error_prefix: String,
    #[serde(default = "default_success_prefix")]
    pub success_prefix: String,
    #[serde(default)]
    pub show_timestamps: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    #[serde(default = "default_true")]
    pub show_test_summary: bool,
    #[serde(default = "default_true")]
    pub track_sql: bool,
    #[serde(default)]
    pub show_sql_stats: bool,
    #[serde(default)]
    pub failed_only: bool,
}

// Default value functions
fn default_api_command() -> String {
    "/go/bin/api".to_string()
}
fn default_health_url() -> String {
    "http://localhost:1323/".to_string()
}
fn default_health_timeout() -> u64 {
    30
}
fn default_health_interval() -> u64 {
    1
}
fn default_jar_path() -> String {
    "/app/karate.jar".to_string()
}
fn default_classpath() -> Vec<String> {
    vec![
        "mysql-connector-j.jar".to_string(),
        "/mocks".to_string(),
        "/shareutils".to_string(),
        "/app".to_string(),
    ]
}
fn default_threads() -> u32 {
    1
}
fn default_output_format() -> String {
    "~html,cucumber:json".to_string()
}
fn default_report_dir() -> String {
    "/tmp/report".to_string()
}
fn default_test_path() -> String {
    "/tests".to_string()
}
fn default_level() -> String {
    "ALL".to_string()
}
fn default_true() -> bool {
    true
}
fn default_export_format() -> String {
    "json".to_string()
}
fn default_api_prefix() -> String {
    "🔷".to_string()
}
fn default_karate_prefix() -> String {
    "🔶".to_string()
}
fn default_sql_prefix() -> String {
    "🗃️".to_string()
}
fn default_error_prefix() -> String {
    "❌".to_string()
}
fn default_success_prefix() -> String {
    "✅".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api: ApiConfig::default(),
            karate: KarateConfig::default(),
            logging: LoggingConfig::default(),
            display: DisplayConfig::default(),
            analysis: AnalysisConfig::default(),
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            command: default_api_command(),
            health_url: default_health_url(),
            health_timeout_secs: default_health_timeout(),
            health_interval_secs: default_health_interval(),
        }
    }
}

impl Default for KarateConfig {
    fn default() -> Self {
        Self {
            jar_path: default_jar_path(),
            classpath: default_classpath(),
            threads: default_threads(),
            output_format: default_output_format(),
            report_dir: default_report_dir(),
            default_test_path: default_test_path(),
            use_compact_object_headers: false,
            use_zgc: false,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_level(),
            include_patterns: vec![],
            exclude_patterns: vec![],
            colors: true,
            export_path: String::new(),
            export_format: default_export_format(),
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            api_prefix: default_api_prefix(),
            karate_prefix: default_karate_prefix(),
            sql_prefix: default_sql_prefix(),
            error_prefix: default_error_prefix(),
            success_prefix: default_success_prefix(),
            show_timestamps: false,
        }
    }
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            show_test_summary: true,
            track_sql: true,
            show_sql_stats: false,
            failed_only: false,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;

        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("toml");

        match extension {
            "toml" => Ok(toml::from_str(&content)?),
            "json" => Ok(serde_json::from_str(&content)?),
            ext => Err(ConfigError::UnsupportedFormat(ext.to_string())),
        }
    }
}
