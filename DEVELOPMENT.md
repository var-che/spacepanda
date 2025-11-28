# Development Guide

## Quick Start

### 1. Enter the Nix Development Environment

```bash
nix develop
```

This will set up everything you need:

- Rust stable toolchain (1.91.1 as of this writing)
- rust-analyzer for IDE support
- cargo and related tools
- All build dependencies

### 2. Run Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run only logging tests
cargo test -p spacepanda-core logging

# Run a specific test
cargo test test_log_level_as_str
```

### 3. Build the Project

```bash
# Build everything
cargo build

# Build in release mode (optimized)
cargo build --release
```

### 4. Run the CLI

```bash
# Basic usage
cargo run --bin spacepanda

# Run test command
cargo run --bin spacepanda test "Hello SpacePanda!"

# With debug logging
cargo run --bin spacepanda -- --log-level debug test

# With JSON logs
cargo run --bin spacepanda -- --json-logs test
```

### 5. Run Examples

```bash
# Run the logging demonstration
cargo run --example logging_demo
```

## Project Structure

```
spacepanda/
├── flake.nix                    # Nix development environment
├── Cargo.toml                   # Workspace configuration
├── spacepanda-core/            # Core library
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs             # Library entry point
│   │   └── logging/           # Logging subsystem
│   │       ├── mod.rs         # Main logging module
│   │       ├── level.rs       # Log level definitions
│   │       └── error.rs       # Error types
│   └── examples/
│       └── logging_demo.rs    # Example usage
└── spacepanda-cli/            # Command-line interface
    ├── Cargo.toml
    └── src/
        └── main.rs            # CLI entry point
```

## The Logging Module

The logging module in `spacepanda-core` provides:

### Features

- **Multiple log levels**: Trace, Debug, Info, Warn, Error
- **Flexible output**: Plain text or JSON format
- **Configurable**: Timestamps and target information can be toggled
- **Environment variable support**: Use `RUST_LOG` to control levels
- **Comprehensive tests**: 14 unit tests covering all functionality

### Usage in Your Code

```rust
use spacepanda_core::logging::{init_logging_with_config, LogConfig, LogLevel};
use tracing::{info, warn, error};

fn main() {
    // Initialize logging
    let config = LogConfig::new(LogLevel::Info)
        .with_timestamp(true)
        .with_target(false);

    init_logging_with_config(config).expect("Failed to init logging");

    // Use logging
    info!("Application started");
    warn!("This is a warning");
    error!("This is an error");
}
```

## Development Workflow

### Watch Mode

For continuous testing during development:

```bash
# Install cargo-watch (if not already available)
cargo install cargo-watch

# Run tests on every file change
cargo watch -x test

# Run specific package tests
cargo watch -x "test -p spacepanda-core"
```

### Adding New Features

1. Add your code to the appropriate module
2. Write tests in a `tests` module
3. Run `cargo test` to verify
4. Update documentation as needed

### Testing Different Log Levels

```bash
# Trace level (most verbose)
cargo run --bin spacepanda -- --log-level trace test

# Debug level
cargo run --bin spacepanda -- --log-level debug test

# Info level (default)
cargo run --bin spacepanda -- --log-level info test

# Warn level
cargo run --bin spacepanda -- --log-level warn test

# Error level (least verbose)
cargo run --bin spacepanda -- --log-level error test
```

## Using direnv (Optional)

If you have `direnv` installed, you can automatically load the Nix environment:

```bash
# Allow direnv for this directory
direnv allow

# Now the environment loads automatically when you cd into the directory
```

## Troubleshooting

### Nix Flake Errors

If you get errors about Git not tracking files:

```bash
git add .
git commit -m "Your changes"
```

### Rust Toolchain Issues

The Nix environment provides a stable Rust toolchain. If you have issues:

```bash
# Exit and re-enter the Nix shell
exit
nix develop
```

### Build Errors

Clean and rebuild:

```bash
cargo clean
cargo build
```

## Next Steps

Now that you have a working logging module, you can:

1. **Add more modules** to `spacepanda-core`:

   - Create new directories under `src/`
   - Add module declarations in `lib.rs`
   - Write tests for each module

2. **Expand the CLI**:

   - Add more subcommands in `spacepanda-cli/src/main.rs`
   - Integrate new core functionality

3. **Add more dependencies**:

   - Edit the workspace `Cargo.toml` to add shared dependencies
   - Update individual package `Cargo.toml` files as needed

4. **Improve the Nix environment**:
   - Add more development tools to `flake.nix`
   - Customize the shell hook message

Enjoy building with SpacePanda! 🐼
