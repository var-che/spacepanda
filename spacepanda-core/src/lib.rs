pub mod logging;

pub use logging::{init_logging, LogLevel};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_exports() {
        // Ensure the main exports are accessible
        let _ = LogLevel::Info;
    }
}
