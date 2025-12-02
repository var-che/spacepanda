//! Commit Validator Trait
//!
//! Validates commits before applying them to group state.

use async_trait::async_trait;
use crate::core_mls::errors::MlsResult;
use super::transport::WireMessage;

/// Commit validation trait
///
/// Validates commits before applying them to ensure:
/// - Epoch correctness
/// - Confirmation tag matches
/// - Parent hash matches
/// - Sender is authorized
/// - No replay attacks
#[async_trait]
pub trait CommitValidator: Send + Sync {
    /// Validate a commit before application
    ///
    /// Should check:
    /// - Epoch is correct (current_epoch + 1)
    /// - Confirmation tag is valid
    /// - Parent hash matches current tree hash
    /// - Sender is a valid member
    /// - Not a replayed commit
    ///
    /// # Arguments
    /// * `wire` - The wire message containing the commit
    ///
    /// # Returns
    /// `Ok(())` if valid, `Err` with details if invalid
    async fn validate_commit(&self, wire: &WireMessage) -> MlsResult<()>;

    /// Validate epoch transition
    ///
    /// Checks if transitioning from `current_epoch` to `new_epoch` is valid.
    ///
    /// # Arguments
    /// * `current_epoch` - Current group epoch
    /// * `new_epoch` - Proposed new epoch
    ///
    /// # Returns
    /// `Ok(())` if transition is valid
    async fn validate_epoch_transition(&self, current_epoch: u64, new_epoch: u64) -> MlsResult<()> {
        if new_epoch != current_epoch + 1 {
            return Err(crate::core_mls::errors::MlsError::EpochMismatch {
                expected: current_epoch + 1,
                actual: new_epoch,
            });
        }
        Ok(())
    }

    /// Check if sender is authorized to commit
    ///
    /// # Arguments
    /// * `sender_id` - The member attempting to commit
    /// * `group_members` - Current group membership
    ///
    /// # Returns
    /// `Ok(())` if authorized
    async fn validate_sender_authorization(&self, sender_id: &[u8], group_members: &[Vec<u8>]) -> MlsResult<()> {
        if group_members.iter().any(|m| m == sender_id) {
            Ok(())
        } else {
            Err(crate::core_mls::errors::MlsError::PermissionDenied(
                "Sender not a group member".to_string()
            ))
        }
    }
}
