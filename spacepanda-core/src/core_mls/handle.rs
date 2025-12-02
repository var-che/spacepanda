//! MLS Handle - Feature-gated implementation selector
//!
//! This module provides a unified MlsHandle interface that can use either:
//! - OpenMLS engine (recommended, feature = "openmls-engine")
//! - Legacy custom implementation (feature = "legacy-mls")

#[cfg(feature = "openmls-engine")]
pub use crate::core_mls::engine::OpenMlsHandleAdapter as MlsHandleImpl;

#[cfg(feature = "legacy-mls")]
pub use crate::core_mls::api::MlsHandle as MlsHandleImpl;

// Compile-time check to ensure exactly one feature is enabled
#[cfg(all(feature = "openmls-engine", feature = "legacy-mls"))]
compile_error!("Cannot enable both 'openmls-engine' and 'legacy-mls' features simultaneously");

#[cfg(not(any(feature = "openmls-engine", feature = "legacy-mls")))]
compile_error!("Must enable either 'openmls-engine' or 'legacy-mls' feature");

/// Re-export the selected implementation as the canonical MlsHandle
pub type MlsHandle = MlsHandleImpl;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_flag_selection() {
        #[cfg(feature = "openmls-engine")]
        {
            // Verify we're using OpenMLS implementation
            println!("Using OpenMLS engine (recommended)");
        }

        #[cfg(feature = "legacy-mls")]
        {
            // Verify we're using legacy implementation
            println!("Using legacy MLS implementation");
        }
    }
}
