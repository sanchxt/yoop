//! Trusted devices management for LocalDrop.
//!
//! This module manages the trust relationship between devices:
//!
//! ## Trust Levels
//!
//! | Level | Behavior |
//! |-------|----------|
//! | `Full` | Auto-connect, transfers require only receiver confirmation |
//! | `AskEachTime` | Auto-discover, but sender must confirm each transfer |
//!
//! ## Security Model
//!
//! - Each device has an Ed25519 keypair (generated on first run)
//! - Public key exchanged during first transfer
//! - Subsequent connections verify signature
//! - Prevents impersonation of trusted devices
//! - Trust database stored locally, never synced

use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::TrustLevel;
use crate::error::Result;

/// A trusted device record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedDevice {
    /// Unique device identifier
    pub device_id: Uuid,
    /// Display name
    pub device_name: String,
    /// Ed25519 public key (base64 encoded)
    pub public_key: String,
    /// When first seen
    pub first_seen: SystemTime,
    /// When last seen
    pub last_seen: SystemTime,
    /// Number of transfers with this device
    pub transfer_count: u32,
    /// When trust was established
    pub trusted_at: SystemTime,
    /// Trust level
    pub trust_level: TrustLevel,
}

impl TrustedDevice {
    /// Create a new trusted device record.
    #[must_use]
    pub fn new(device_id: Uuid, device_name: String, public_key: String) -> Self {
        let now = SystemTime::now();
        Self {
            device_id,
            device_name,
            public_key,
            first_seen: now,
            last_seen: now,
            transfer_count: 1,
            trusted_at: now,
            trust_level: TrustLevel::AskEachTime,
        }
    }

    /// Update the last seen timestamp.
    pub fn update_last_seen(&mut self) {
        self.last_seen = SystemTime::now();
        self.transfer_count += 1;
    }
}

/// Trust database for managing trusted devices.
#[derive(Debug)]
pub struct TrustStore {
    /// Path to the trust database file
    #[allow(dead_code)]
    path: PathBuf,
    /// Trusted devices
    devices: Vec<TrustedDevice>,
}

impl TrustStore {
    /// Load the trust store from the default location.
    ///
    /// # Errors
    ///
    /// Returns an error if the store cannot be loaded.
    pub fn load() -> Result<Self> {
        let path = Self::default_path().unwrap_or_else(|| PathBuf::from("trust.json"));

        // TODO: Load from file
        Ok(Self {
            path,
            devices: Vec::new(),
        })
    }

    /// Load from a specific path.
    ///
    /// # Errors
    ///
    /// Returns an error if the store cannot be loaded.
    pub fn load_from(path: PathBuf) -> Result<Self> {
        // TODO: Load from file
        Ok(Self {
            path,
            devices: Vec::new(),
        })
    }

    /// Get the default trust store path.
    #[must_use]
    pub fn default_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "localdrop", "LocalDrop")
            .map(|dirs| dirs.data_dir().join("trust.json"))
    }

    /// Save the trust store.
    ///
    /// # Errors
    ///
    /// Returns an error if the store cannot be saved.
    pub fn save(&self) -> Result<()> {
        // TODO: Save to file
        Ok(())
    }

    /// List all trusted devices.
    #[must_use]
    pub fn list(&self) -> &[TrustedDevice] {
        &self.devices
    }

    /// Find a device by ID.
    #[must_use]
    pub fn find_by_id(&self, device_id: &Uuid) -> Option<&TrustedDevice> {
        self.devices.iter().find(|d| &d.device_id == device_id)
    }

    /// Find a device by name.
    #[must_use]
    pub fn find_by_name(&self, name: &str) -> Option<&TrustedDevice> {
        self.devices
            .iter()
            .find(|d| d.device_name.eq_ignore_ascii_case(name))
    }

    /// Add a trusted device.
    ///
    /// # Errors
    ///
    /// Returns an error if the device cannot be added.
    pub fn add(&mut self, device: TrustedDevice) -> Result<()> {
        self.devices.retain(|d| d.device_id != device.device_id);
        self.devices.push(device);
        self.save()
    }

    /// Remove a trusted device by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the store cannot be saved.
    pub fn remove(&mut self, device_id: &Uuid) -> Result<bool> {
        let len_before = self.devices.len();
        self.devices.retain(|d| &d.device_id != device_id);
        let removed = self.devices.len() < len_before;
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// Update trust level for a device.
    ///
    /// # Errors
    ///
    /// Returns an error if the device is not found or cannot be saved.
    pub fn set_trust_level(&mut self, device_id: &Uuid, level: TrustLevel) -> Result<bool> {
        if let Some(device) = self.devices.iter_mut().find(|d| &d.device_id == device_id) {
            device.trust_level = level;
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Check if a device is trusted.
    #[must_use]
    pub fn is_trusted(&self, device_id: &Uuid) -> bool {
        self.find_by_id(device_id).is_some()
    }

    /// Verify a device's public key.
    ///
    /// # Returns
    ///
    /// `true` if the device is trusted and the public key matches.
    #[must_use]
    pub fn verify_key(&self, device_id: &Uuid, public_key: &str) -> bool {
        self.find_by_id(device_id)
            .is_some_and(|d| d.public_key == public_key)
    }
}
