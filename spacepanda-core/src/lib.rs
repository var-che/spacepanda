pub mod logging;
pub mod core_identity;

pub use logging::{init_logging, LogLevel};
pub use core_identity::{ChannelIdentity, Ed25519Keypair, GlobalIdentity, IdentityError};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_exports() {
        // Ensure the main exports are accessible
        let _ = LogLevel::Info;
    }
}
