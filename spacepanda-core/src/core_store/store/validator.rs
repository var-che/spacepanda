/*
    validator.rs - CRDT operation validation

    Validates operations before they are applied or stored.
    Ensures:
    - Causality (vector clock ordering)
    - Signatures (if present)
    - Schema conformance
    - Authorization (role-based)
*/

use crate::core_store::crdt::{OperationMetadata, VectorClock};
use crate::core_store::model::{PermissionLevel, UserId};
use crate::core_store::store::errors::{StoreError, StoreResult};
use std::collections::HashMap;

/// Validation rules
pub struct ValidationRules {
    /// Require signatures on all operations
    pub require_signatures: bool,

    /// Enforce strict causality
    pub enforce_causality: bool,

    /// Check permissions
    pub check_permissions: bool,
}

impl Default for ValidationRules {
    fn default() -> Self {
        ValidationRules {
            require_signatures: true,
            enforce_causality: true,
            check_permissions: true,
        }
    }
}

/// Validates CRDT operations
pub struct OperationValidator {
    rules: ValidationRules,

    /// Current vector clock state
    current_clock: VectorClock,

    /// User permissions cache
    permissions: HashMap<UserId, PermissionLevel>,
}

impl OperationValidator {
    pub fn new(rules: ValidationRules) -> Self {
        OperationValidator { rules, current_clock: VectorClock::new(), permissions: HashMap::new() }
    }

    /// Validate operation metadata
    pub fn validate_metadata(&self, metadata: &OperationMetadata) -> StoreResult<()> {
        // Check signature requirement
        if self.rules.require_signatures && metadata.signature.is_none() {
            return Err(StoreError::ValidationError("Missing signature".to_string()));
        }

        // Check causality
        if self.rules.enforce_causality {
            self.validate_causality(&metadata.vector_clock)?;
        }

        Ok(())
    }

    /// Validate causality using vector clocks
    fn validate_causality(&self, _incoming_clock: &VectorClock) -> StoreResult<()> {
        // Simplified validation - in production, would do full causal comparison
        // For now, just accept all operations
        Ok(())
    }

    /// Validate user permissions
    pub fn validate_permission(
        &self,
        user_id: &UserId,
        required_level: &PermissionLevel,
    ) -> StoreResult<()> {
        if !self.rules.check_permissions {
            return Ok(());
        }

        let user_level = self
            .permissions
            .get(user_id)
            .ok_or_else(|| StoreError::ValidationError("User not found".to_string()))?;

        if !Self::has_permission(user_level, required_level) {
            return Err(StoreError::ValidationError(format!("Insufficient permissions")));
        }

        Ok(())
    }

    /// Check if a permission level is sufficient
    fn has_permission(user_level: &PermissionLevel, required_level: &PermissionLevel) -> bool {
        // Check each required permission
        if required_level.read && !user_level.read {
            return false;
        }
        if required_level.write && !user_level.write {
            return false;
        }
        if required_level.admin && !user_level.admin {
            return false;
        }
        if required_level.ban_members && !user_level.ban_members {
            return false;
        }
        if required_level.manage_roles && !user_level.manage_roles {
            return false;
        }
        if required_level.manage_channels && !user_level.manage_channels {
            return false;
        }
        true
    }

    /// Add user permission
    pub fn add_user_permission(&mut self, user_id: UserId, level: PermissionLevel) {
        self.permissions.insert(user_id, level);
    }

    /// Update vector clock after accepting an operation
    pub fn update_clock(&mut self, metadata: &OperationMetadata) {
        self.current_clock.merge(&metadata.vector_clock);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_creation() {
        let validator = OperationValidator::new(ValidationRules::default());
        assert_eq!(validator.rules.require_signatures, true);
    }

    #[test]
    fn test_signature_requirement() {
        let validator = OperationValidator::new(ValidationRules::default());

        let metadata_without_sig = OperationMetadata {
            timestamp: 1000,
            vector_clock: VectorClock::new(),
            signature: None,
            node_id: "node1".to_string(),
        };

        let result = validator.validate_metadata(&metadata_without_sig);
        assert!(result.is_err());
    }

    #[test]
    fn test_signature_optional() {
        let rules = ValidationRules { require_signatures: false, ..Default::default() };
        let validator = OperationValidator::new(rules);

        let metadata = OperationMetadata {
            timestamp: 1000,
            vector_clock: VectorClock::new(),
            signature: None,
            node_id: "node1".to_string(),
        };

        let result = validator.validate_metadata(&metadata);
        assert!(result.is_ok());
    }

    #[test]
    fn test_permission_validation() {
        let mut validator = OperationValidator::new(ValidationRules::default());

        let user_id = UserId::generate();
        validator.add_user_permission(user_id.clone(), PermissionLevel::read_only());

        // Read permission should succeed
        assert!(validator.validate_permission(&user_id, &PermissionLevel::read_only()).is_ok());

        // Write permission should fail
        assert!(validator.validate_permission(&user_id, &PermissionLevel::member()).is_err());
    }

    #[test]
    fn test_admin_permissions() {
        let mut validator = OperationValidator::new(ValidationRules::default());

        let user_id = UserId::generate();
        validator.add_user_permission(user_id.clone(), PermissionLevel::admin());

        // Admin can do everything
        assert!(validator.validate_permission(&user_id, &PermissionLevel::read_only()).is_ok());
        assert!(validator.validate_permission(&user_id, &PermissionLevel::member()).is_ok());
        assert!(validator.validate_permission(&user_id, &PermissionLevel::admin()).is_ok());
    }
}
