// file: src/logging/logger.rs
// version: 1.0.0
// guid: j0k1l2m3-n4o5-6789-0123-456789jklmno

//! Logger initialization and configuration

use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use crate::Result;

/// Initialize the logging system
pub fn init_logger(verbose: bool, quiet: bool) -> Result<()> {
    let filter = if quiet {
        EnvFilter::new("error")
    } else if verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_target(false)
                .with_thread_ids(false)
                .with_file(false)
                .with_line_number(false)
                .compact(),
        )
        .try_init()
        .map_err(|e| crate::error::AutoInstallError::ConfigError(
            format!("Failed to initialize logger: {}", e)
        ))?;

    Ok(())
}

/// Initialize structured JSON logging (for services)
pub fn init_json_logger() -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .try_init()
        .map_err(|e| crate::error::AutoInstallError::ConfigError(
            format!("Failed to initialize JSON logger: {}", e)
        ))?;

    Ok(())
}

/// Create a scoped logger for operations
pub fn with_operation_span<F, R>(operation: &str, f: F) -> R
where
    F: FnOnce() -> R,
{
    let span = tracing::info_span!("operation", name = operation);
    let _enter = span.enter();
    f()
}

/// Create an async scoped logger for operations
pub async fn with_async_operation_span<F, Fut, R>(operation: &str, f: F) -> R
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = R>,
{
    let span = tracing::info_span!("operation", name = operation);
    async move { f().await }.instrument(span).await
}

// Re-export tracing macros for convenience
pub use tracing::{debug, error, info, trace, warn};

use tracing::Instrument;