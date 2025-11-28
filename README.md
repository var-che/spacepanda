# SpacePanda 🐼

A Rust project with a modular architecture, inspired by Veilid.

## Project Structure

```
spacepanda/
├── spacepanda-core/     # Core library with logging subsystem
│   └── src/
│       ├── lib.rs
│       └── logging/     # Logging module
├── spacepanda-cli/      # Command-line interface
│   └── src/
│       └── main.rs
├── Cargo.toml          # Workspace configuration
└── flake.nix           # Nix development environment
```

## Prerequisites

- [Nix](https://nixos.org/download.html) with flakes enabled
- Or: Rust stable toolchain (if not using Nix)

## Development with Nix

### Enable Nix Flakes

If you haven't enabled flakes yet, add this to `~/.config/nix/nix.conf`:

```
experimental-features = nix-command flakes
```

### Enter Development Shell

```bash
nix develop
```

This will set up a development environment with:

- Rust stable toolchain
- rust-analyzer for IDE support
- cargo and related tools
- All necessary build dependencies

## Building and Testing

### Build the entire workspace

```bash
cargo build
```

### Run tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run tests for spacepanda-core only
cargo test -p spacepanda-core

# Run tests for the logging module
cargo test -p spacepanda-core logging
```

### Run the CLI

```bash
# Run with default settings
cargo run --bin spacepanda

# Run the test command
cargo run --bin spacepanda test "Hello World"

# Set log level to debug
cargo run --bin spacepanda -- --log-level debug test

# Enable JSON logging
cargo run --bin spacepanda -- --json-logs test
```

### Development with auto-reload

```bash
# Watch for changes and run tests
cargo watch -x test

# Watch and run the CLI
cargo watch -x "run --bin spacepanda"
```

## Features

### Logging Subsystem

The `spacepanda-core` library includes a comprehensive logging subsystem with:

- Multiple log levels (Trace, Debug, Info, Warn, Error)
- Configurable output format (plain text or JSON)
- Timestamp and target module configuration
- Full test coverage

Example usage:

```rust
use spacepanda_core::logging::{init_logging_with_config, LogConfig, LogLevel};
use tracing::info;

fn main() {
    let config = LogConfig::new(LogLevel::Debug)
        .with_timestamp(true)
        .with_target(false);

    init_logging_with_config(config).expect("Failed to initialize logging");

    info!("Application started");
}
```

## License

MIT OR Apache-2.0
