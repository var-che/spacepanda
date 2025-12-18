use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub data_dir: PathBuf,
}

/// Manages user profiles and password-based authentication
pub struct AuthManager {
    profiles: Arc<RwLock<HashMap<String, UserProfile>>>,
    profiles_dir: PathBuf,
}

impl AuthManager {
    pub fn new() -> Self {
        let profiles_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("spacepanda")
            .join("profiles");

        std::fs::create_dir_all(&profiles_dir).ok();

        let mut manager = Self {
            profiles: Arc::new(RwLock::new(HashMap::new())),
            profiles_dir,
        };

        // Load existing profiles
        manager.load_profiles();
        manager
    }

    fn load_profiles(&mut self) {
        // Load profiles from disk
        if let Ok(entries) = std::fs::read_dir(&self.profiles_dir) {
            for entry in entries.flatten() {
                if let Ok(data) = std::fs::read_to_string(entry.path()) {
                    if let Ok(profile) = serde_json::from_str::<UserProfile>(&data) {
                        let profiles = self.profiles.clone();
                        tokio::spawn(async move {
                            profiles.write().await.insert(profile.id.clone(), profile);
                        });
                    }
                }
            }
        }
    }

    pub async fn create_profile(
        &self,
        username: Option<String>,
        password: &str,
    ) -> ApiResult<UserProfile> {
        let username = username.unwrap_or_else(|| "user".to_string());
        let id = Uuid::new_v4().to_string();

        // Hash password
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Password hashing failed: {}", e)))?
            .to_string();

        // Create data directory for this user
        let data_dir = self.profiles_dir.join(&id);
        std::fs::create_dir_all(&data_dir)?;

        let profile = UserProfile {
            id: id.clone(),
            username,
            password_hash,
            data_dir,
        };

        // Save profile
        self.save_profile(&profile).await?;

        // Store in memory
        self.profiles
            .write()
            .await
            .insert(id.clone(), profile.clone());

        Ok(profile)
    }

    pub async fn unlock(
        &self,
        username: Option<String>,
        password: &str,
    ) -> ApiResult<UserProfile> {
        let profiles = self.profiles.read().await;

        // Find profile by username or use default
        let profile = if let Some(username) = username {
            profiles
                .values()
                .find(|p| p.username == username)
                .ok_or_else(|| ApiError::AuthenticationFailed("User not found".to_string()))?
        } else {
            // Use first available profile if no username specified
            profiles
                .values()
                .next()
                .ok_or_else(|| ApiError::AuthenticationFailed("No profiles found".to_string()))?
        };

        // Verify password
        let parsed_hash = PasswordHash::new(&profile.password_hash)
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Invalid password hash: {}", e)))?;

        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| ApiError::AuthenticationFailed("Invalid password".to_string()))?;

        Ok(profile.clone())
    }

    pub async fn list_profiles(&self) -> Vec<String> {
        self.profiles
            .read()
            .await
            .values()
            .map(|p| p.username.clone())
            .collect()
    }

    async fn save_profile(&self, profile: &UserProfile) -> ApiResult<()> {
        let path = self.profiles_dir.join(format!("{}.json", profile.id));
        let data = serde_json::to_string_pretty(profile)?;
        std::fs::write(path, data)?;
        Ok(())
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}
