//! Request correlation for failed-only mode

use crate::log_parser::{extract_path_query, ApiLogEntry};
use std::collections::HashMap;

/// Correlates API logs with Karate test results using request_id
pub struct RequestCorrelator {
    /// Buffer: request_id -> Vec<(raw_json, parsed_entry)>
    request_logs: HashMap<String, Vec<(String, ApiLogEntry)>>,
    /// Mapping: URL path+query -> request_id (from REQUEST log)
    url_to_request_id: HashMap<String, String>,
    /// Track all URLs that have been seen for potential matching
    seen_urls: Vec<(String, String)>, // (full_uri, request_id)
    /// Track the most recent request_id (for fallback when no URL in failure)
    last_request_id: Option<String>,
}

impl RequestCorrelator {
    pub fn new() -> Self {
        Self {
            request_logs: HashMap::new(),
            url_to_request_id: HashMap::new(),
            seen_urls: Vec::new(),
            last_request_id: None,
        }
    }

    /// Buffer an API log entry, grouped by request_id
    pub fn buffer_api_log(&mut self, raw_json: String, entry: ApiLogEntry) {
        if let Some(request_id) = &entry.request_id {
            self.request_logs
                .entry(request_id.clone())
                .or_default()
                .push((raw_json, entry.clone()));

            // Track this as the most recent request
            self.last_request_id = Some(request_id.clone());

            // If this is a REQUEST log (final request summary), map URL -> request_id
            if entry.is_request_summary() {
                if let Some(uri) = &entry.uri {
                    self.url_to_request_id
                        .insert(uri.clone(), request_id.clone());
                    self.seen_urls.push((uri.clone(), request_id.clone()));
                }
            }
        }
    }

    /// Get all buffered logs for a URL that failed
    /// Returns the request_id and all associated logs
    pub fn get_failed_request_logs(
        &self,
        full_url: &str,
    ) -> Option<(&str, &Vec<(String, ApiLogEntry)>)> {
        // Extract path+query from the full URL
        let path_query = extract_path_query(full_url)?;

        // Look up request_id by path+query
        let request_id = self.url_to_request_id.get(&path_query)?;

        // Return the logs for that request_id
        self.request_logs
            .get(request_id)
            .map(|logs| (request_id.as_str(), logs))
    }

    /// Try to find matching logs by partial URL match (fallback)
    pub fn find_matching_logs_by_url(
        &self,
        partial_url: &str,
    ) -> Option<(&str, &Vec<(String, ApiLogEntry)>)> {
        // Try exact match first
        if let Some(result) = self.get_failed_request_logs(partial_url) {
            return Some(result);
        }

        // Try partial match on path
        let search_path = extract_path_query(partial_url)?;

        for (uri, request_id) in &self.seen_urls {
            if uri.contains(&search_path) || search_path.contains(uri) {
                if let Some(logs) = self.request_logs.get(request_id) {
                    return Some((request_id.as_str(), logs));
                }
            }
        }

        None
    }

    /// Get the last N logs from the most recent request (fallback for failures without URL)
    pub fn get_last_request_logs(&self, max_logs: usize) -> Option<(&str, Vec<&(String, ApiLogEntry)>)> {
        let request_id = self.last_request_id.as_ref()?;
        let logs = self.request_logs.get(request_id)?;
        
        // Return the last N logs
        let start = logs.len().saturating_sub(max_logs);
        let last_logs: Vec<_> = logs.iter().skip(start).collect();
        
        Some((request_id.as_str(), last_logs))
    }

    /// Clear all buffered logs (call after test completes)
    pub fn clear(&mut self) {
        self.request_logs.clear();
        self.url_to_request_id.clear();
        self.seen_urls.clear();
        self.last_request_id = None;
    }

    /// Get the number of buffered requests
    pub fn buffered_count(&self) -> usize {
        self.request_logs.len()
    }

    /// Get total number of buffered log entries
    pub fn total_logs(&self) -> usize {
        self.request_logs.values().map(|v| v.len()).sum()
    }
}

impl Default for RequestCorrelator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_log(request_id: &str, msg: &str, uri: Option<&str>, status: Option<u16>) -> ApiLogEntry {
        ApiLogEntry {
            request_id: Some(request_id.to_string()),
            msg: msg.to_string(),
            uri: uri.map(|s| s.to_string()),
            status,
            ..Default::default()
        }
    }

    #[test]
    fn test_buffer_and_retrieve() {
        let mut correlator = RequestCorrelator::new();

        // Simulate a request flow
        let entry1 = make_log("abc123", "token claims", Some("/api/v1/test?id=1"), None);
        let entry2 = make_log("abc123", "processing", Some("/api/v1/test?id=1"), None);
        let entry3 = make_log("abc123", "REQUEST", Some("/api/v1/test?id=1"), Some(200));

        correlator.buffer_api_log("{\"test\":1}".to_string(), entry1);
        correlator.buffer_api_log("{\"test\":2}".to_string(), entry2);
        correlator.buffer_api_log("{\"test\":3}".to_string(), entry3);

        // Retrieve by full URL
        let result = correlator.get_failed_request_logs("http://localhost:1323/api/v1/test?id=1");
        assert!(result.is_some());

        let (request_id, logs) = result.unwrap();
        assert_eq!(request_id, "abc123");
        assert_eq!(logs.len(), 3);
    }

    #[test]
    fn test_partial_url_match() {
        let mut correlator = RequestCorrelator::new();

        let entry = make_log("xyz789", "REQUEST", Some("/api/v1/karte/outcome?patientID=1"), Some(200));
        correlator.buffer_api_log("{}".to_string(), entry);

        // Should find by partial match
        let result = correlator.find_matching_logs_by_url(
            "http://localhost:1323/api/v1/karte/outcome?patientID=1"
        );
        assert!(result.is_some());
    }
}
