//! Example demonstrating the logging subsystem
//!
//! Run with:
//! ```bash
//! cargo run --example logging_demo
//! ```

use spacepanda_core::logging::{init_logging_with_config, LogConfig, LogLevel};
use tracing::{debug, error, info, trace, warn};

fn main() {
    // Initialize logging with debug level
    let config = LogConfig::new(LogLevel::Debug).with_timestamp(true).with_target(true);

    init_logging_with_config(config).expect("Failed to initialize logging");

    // Log at different levels
    trace!("This is a trace message (won't show with Debug level)");
    debug!("This is a debug message");
    info!("This is an info message");
    warn!("This is a warning");
    error!("This is an error");

    // Structured logging
    let user_id = 42;
    let action = "login";
    info!(user_id = user_id, action = action, "User action logged");

    // Logging with context
    info!("Application initialized successfully");
}
