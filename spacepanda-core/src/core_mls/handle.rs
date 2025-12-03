//! MLS Handle - OpenMLS implementation
//!
//! This module provides the unified MlsHandle interface using OpenMLS.
//! The legacy custom implementation has been removed in favor of OpenMLS.

pub use crate::core_mls::engine::OpenMlsHandleAdapter as MlsHandle;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mls_handle_available() {
        // Verify MlsHandle type is exported
        // (compile-time check)
        let _ = std::marker::PhantomData::<MlsHandle>::default();
    }
}
