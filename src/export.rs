//! Log export functionality

use crate::log_parser::ApiLogEntry;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Export format options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExportFormat {
    Json,
    Text,
    Both,
}

impl ExportFormat {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" => ExportFormat::Json,
            "text" | "txt" => ExportFormat::Text,
            "both" => ExportFormat::Both,
            _ => ExportFormat::Json,
        }
    }
}

/// Log exporter for writing logs to files
pub struct LogExporter {
    format: ExportFormat,
    json_writer: Option<BufWriter<File>>,
    text_writer: Option<BufWriter<File>>,
    json_entries: Vec<serde_json::Value>,
}

impl LogExporter {
    /// Create a new exporter with the given base path
    pub fn new(path: &str, format: ExportFormat) -> std::io::Result<Option<Self>> {
        if path.is_empty() {
            return Ok(None);
        }

        let base_path = Path::new(path);

        let (json_writer, text_writer) = match format {
            ExportFormat::Json => {
                let json_path = if path.ends_with(".json") {
                    base_path.to_path_buf()
                } else {
                    base_path.with_extension("json")
                };
                (Some(BufWriter::new(File::create(json_path)?)), None)
            }
            ExportFormat::Text => {
                let text_path = if path.ends_with(".txt") || path.ends_with(".log") {
                    base_path.to_path_buf()
                } else {
                    base_path.with_extension("log")
                };
                (None, Some(BufWriter::new(File::create(text_path)?)))
            }
            ExportFormat::Both => {
                let json_path = base_path.with_extension("json");
                let text_path = base_path.with_extension("log");
                (
                    Some(BufWriter::new(File::create(json_path)?)),
                    Some(BufWriter::new(File::create(text_path)?)),
                )
            }
        };

        Ok(Some(Self {
            format,
            json_writer,
            text_writer,
            json_entries: Vec::new(),
        }))
    }

    /// Write an API log entry
    pub fn write_api_log(&mut self, raw_json: &str, _entry: &ApiLogEntry) -> std::io::Result<()> {
        // Write to JSON (collect for array output)
        if self.json_writer.is_some() {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw_json) {
                self.json_entries.push(value);
            }
        }

        // Write to text file
        if let Some(writer) = &mut self.text_writer {
            writeln!(writer, "[API] {}", raw_json)?;
        }

        Ok(())
    }

    /// Write a Karate log line
    pub fn write_karate_log(&mut self, line: &str) -> std::io::Result<()> {
        // For JSON, create a simple object
        if self.json_writer.is_some() {
            self.json_entries.push(serde_json::json!({
                "source": "karate",
                "message": line
            }));
        }

        // Write to text file
        if let Some(writer) = &mut self.text_writer {
            writeln!(writer, "[KARATE] {}", line)?;
        }

        Ok(())
    }

    /// Flush and finalize the export
    pub fn finish(mut self) -> std::io::Result<()> {
        // Write JSON array
        if let Some(mut writer) = self.json_writer.take() {
            let json = serde_json::to_string_pretty(&self.json_entries)?;
            writer.write_all(json.as_bytes())?;
            writer.flush()?;
        }

        // Flush text writer
        if let Some(mut writer) = self.text_writer.take() {
            writer.flush()?;
        }

        Ok(())
    }
}

/// Simple line-based exporter for raw output
pub struct RawExporter {
    writer: BufWriter<File>,
}

impl RawExporter {
    pub fn new(path: &str) -> std::io::Result<Option<Self>> {
        if path.is_empty() {
            return Ok(None);
        }

        Ok(Some(Self {
            writer: BufWriter::new(File::create(path)?),
        }))
    }

    pub fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        writeln!(self.writer, "{}", line)
    }

    pub fn finish(mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}
