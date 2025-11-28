use anyhow::Result;
use clap::Parser;
use spacepanda_core::logging::{init_logging_with_config, LogConfig, LogLevel};
use tracing::{info, warn};

#[derive(Parser, Debug)]
#[command(name = "spacepanda")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Set the log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,

    /// Enable JSON formatted logging
    #[arg(long)]
    json_logs: bool,

    /// Subcommand to execute
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Parser, Debug)]
enum Command {
    /// Run a test command
    Test {
        /// Test message to log
        #[arg(default_value = "Hello from SpacePanda!")]
        message: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Parse log level
    let log_level = LogLevel::from_str(&args.log_level)
        .unwrap_or_else(|| {
            eprintln!("Invalid log level '{}', using 'info'", args.log_level);
            LogLevel::Info
        });

    // Initialize logging
    let config = LogConfig::new(log_level)
        .json_format(args.json_logs);

    init_logging_with_config(config)?;

    info!("SpacePanda CLI started");

    match args.command {
        Some(Command::Test { message }) => {
            info!("Test command executed");
            info!("Message: {}", message);
            warn!("This is a test warning");
        }
        None => {
            info!("No command specified. Use --help for usage information.");
        }
    }

    info!("SpacePanda CLI finished");

    Ok(())
}
