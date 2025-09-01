// file: src/logger.rs
// version: 1.1.0
// guid: 5a9fbb43-1e0b-4bea-a858-b74b58176503

use crate::error::Result;
use chrono;
use std::fs;
use std::io;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Setup logging for the application with both stdout and file output
pub fn setup_logging() -> Result<()> {
    // Create logs directory if it doesn't exist
    let logs_dir = "logs";
    if !std::path::Path::new(logs_dir).exists() {
        fs::create_dir_all(logs_dir)?;
    }

    // Generate log filename with timestamp
    let now = chrono::Utc::now();
    let log_filename = format!("{}/copilot-agent-util-{}.log", logs_dir, now.format("%Y%m%d_%H%M%S"));

    // Create filter from environment or default to info
    let filter_stdout = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();
    let filter_file = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    // Create file appender
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_filename)?;

    // Create stdout layer
    let stdout_layer = fmt::layer()
        .with_target(false)
        .with_writer(io::stdout)
        .with_filter(filter_stdout);

    // Create file layer
    let file_layer = fmt::layer()
        .with_target(false)
        .with_ansi(false) // No ANSI colors in log files
        .with_writer(file)
        .with_filter(filter_file);

    // Initialize subscriber with both layers
    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(file_layer)
        .init();

    tracing::info!("Logging initialized - writing to stdout and {}", log_filename);

    Ok(())
}
