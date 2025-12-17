# Test log Monitor

> [!CAUTION]
> The code is customized for personal use only.

A Rust-based CLI tool for monitoring and analyzing Karate E2E tests. This tool replaces the shell-based log processing, providing configurable log filtering, test result summaries, SQL analysis, and log persistence.

## Features

- 🔍 **Real-time log filtering** by level and regex patterns
- 📊 **Test result summaries** with pass/fail counts
- 🗃️ **SQL query analysis** with timing statistics
- 💾 **Log export** to JSON or text files
- 🎯 **Failed-only mode** - shows only logs related to failed tests
- 🎨 **Colored output** with customizable prefixes
- ⚙️ **Configurable** via TOML/JSON or CLI arguments

## Usage

### Basic Usage

```bash
# Run all tests
karate-monitor /tests

# Run specific feature file
karate-monitor /tests/my_feature.feature
```

### CLI Options

```bash
# Show only logs for failed scenarios
karate-monitor --failed-only /tests

# Filter by log level (DEBUG, INFO, WARN, ERROR)
karate-monitor --level ERROR /tests

# Include only logs matching patterns
karate-monitor --include "karte" --include "patient" /tests

# Exclude logs matching patterns
karate-monitor --exclude "health" --exclude "ping" /tests

# Disable colors (for CI environments)
karate-monitor --no-color /tests

# Export logs to file
karate-monitor --export /tmp/test-logs /tests

# Show SQL statistics
karate-monitor --sql-stats /tests

# Use custom config file
karate-monitor -c /path/to/config.toml /tests
```

### Configuration File

Create a `karate-monitor.toml` file:

```toml
[api]
command = "/go/bin/api"
health_url = "http://localhost:1323/"
health_timeout_secs = 30

[karate]
jar_path = "/app/karate.jar"
threads = 1
default_test_path = "/tests"

[logging]
level = "ALL"
include_patterns = []
exclude_patterns = ["health"]
colors = true

[analysis]
show_test_summary = true
show_sql_stats = true
failed_only = false
```

## Building

### Local Build

```bash
cd karate-monitor
cargo build --release
```

### Docker Build

The Rust binary is built as part of the Docker multi-stage build:

```dockerfile
FROM rust:1.83-alpine AS rust-builder
WORKDIR /app
RUN apk add --no-cache musl-dev
COPY karate-monitor/ .
RUN cargo build --release && strip target/release/karate-monitor
```

## Architecture

```
karate-monitor/
├── src/
│   ├── main.rs           # Entry point, CLI parsing
│   ├── config.rs         # Configuration loading (TOML/JSON)
│   ├── process.rs        # Process spawning and management
│   ├── log_parser.rs     # Log parsing for API and Karate
│   ├── filter.rs         # Log filtering logic
│   ├── formatter.rs      # Colored output formatting
│   ├── correlation.rs    # Request correlation for failed-only mode
│   ├── analysis.rs       # Test summary and SQL analysis
│   └── export.rs         # Log export functionality
├── Cargo.toml
└── README.md
```

## Failed-Only Mode

When `--failed-only` is enabled, the tool:

1. Buffers all API logs grouped by `request_id`
2. Maps URLs to request IDs from the `REQUEST` summary logs
3. When Karate outputs a failure with a URL, shows all buffered logs for that request
4. Discards logs for successful requests

This dramatically reduces log noise when debugging test failures.

## License

MIT
