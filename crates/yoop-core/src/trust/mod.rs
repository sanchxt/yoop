//! Trusted devices management for Yoop.
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

use std::fs;
use std::io::{BufReader, BufWriter};
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::TrustLevel;
use crate::error::{Error, Result};

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
    /// Last known IP address (for direct connection)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_known_ip: Option<IpAddr>,
    /// Last known transfer port
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_known_port: Option<u16>,
    /// When the address was last updated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_updated_at: Option<SystemTime>,
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
            last_known_ip: None,
            last_known_port: None,
            address_updated_at: None,
        }
    }

    /// Set the trust level.
    #[must_use]
    pub fn with_trust_level(mut self, level: TrustLevel) -> Self {
        self.trust_level = level;
        self
    }

    /// Set the last known address.
    #[must_use]
    pub fn with_address(mut self, ip: IpAddr, port: u16) -> Self {
        self.last_known_ip = Some(ip);
        self.last_known_port = Some(port);
        self.address_updated_at = Some(SystemTime::now());
        self
    }

    /// Update the last seen timestamp.
    pub fn update_last_seen(&mut self) {
        self.last_seen = SystemTime::now();
        self.transfer_count += 1;
    }

    /// Get the stored address if available.
    #[must_use]
    pub fn address(&self) -> Option<(IpAddr, u16)> {
        self.last_known_ip.zip(self.last_known_port)
    }
}

/// Serializable wrapper for the trust database.
#[derive(Debug, Serialize, Deserialize)]
struct TrustDatabase {
    /// Version of the trust database format
    version: u32,
    /// List of trusted devices
    devices: Vec<TrustedDevice>,
}

impl Default for TrustDatabase {
    fn default() -> Self {
        Self {
            version: 1,
            devices: Vec::new(),
        }
    }
}

/// Trust database for managing trusted devices.
#[derive(Debug)]
pub struct TrustStore {
    /// Path to the trust database file
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
        Self::load_from(path)
    }

    /// Load from a specific path.
    ///
    /// # Errors
    ///
    /// Returns an error if the store cannot be loaded.
    pub fn load_from(path: PathBuf) -> Result<Self> {
        if !path.exists() {
            return Ok(Self {
                path,
                devices: Vec::new(),
            });
        }

        let file = fs::File::open(&path).map_err(|e| {
            Error::ConfigError(format!(
                "Failed to open trust store at {}: {}",
                path.display(),
                e
            ))
        })?;

        let reader = BufReader::new(file);
        let db: TrustDatabase = serde_json::from_reader(reader).map_err(|e| {
            Error::ConfigError(format!(
                "Failed to parse trust store at {}: {}",
                path.display(),
                e
            ))
        })?;

        Ok(Self {
            path,
            devices: db.devices,
        })
    }

    /// Get the default trust store path.
    #[must_use]
    pub fn default_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "yoop", "Yoop")
            .map(|dirs| dirs.data_dir().join("trust.json"))
    }

    /// Save the trust store.
    ///
    /// # Errors
    ///
    /// Returns an error if the store cannot be saved.
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                Error::ConfigError(format!(
                    "Failed to create trust store directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        let db = TrustDatabase {
            version: 1,
            devices: self.devices.clone(),
        };

        let file = fs::File::create(&self.path).map_err(|e| {
            Error::ConfigError(format!(
                "Failed to create trust store at {}: {}",
                self.path.display(),
                e
            ))
        })?;

        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &db).map_err(|e| {
            Error::ConfigError(format!(
                "Failed to write trust store at {}: {}",
                self.path.display(),
                e
            ))
        })?;

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

    /// Update the last seen timestamp for a device.
    ///
    /// # Errors
    ///
    /// Returns an error if the store cannot be saved.
    pub fn update_last_seen(&mut self, device_id: &Uuid) -> Result<bool> {
        if let Some(device) = self.devices.iter_mut().find(|d| &d.device_id == device_id) {
            device.update_last_seen();
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get the path to the trust store file.
    #[must_use]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Clear all trusted devices.
    ///
    /// # Errors
    ///
    /// Returns an error if the store cannot be saved.
    pub fn clear(&mut self) -> Result<()> {
        self.devices.clear();
        self.save()
    }

    /// Update the stored address for a device.
    ///
    /// Called after successful connection to keep address current.
    ///
    /// # Errors
    ///
    /// Returns an error if the store cannot be saved.
    pub fn update_address(&mut self, device_id: &Uuid, ip: IpAddr, port: u16) -> Result<bool> {
        if let Some(device) = self.devices.iter_mut().find(|d| &d.device_id == device_id) {
            device.last_known_ip = Some(ip);
            device.last_known_port = Some(port);
            device.address_updated_at = Some(SystemTime::now());
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get the stored address for a device.
    #[must_use]
    pub fn get_address(&self, device_id: &Uuid) -> Option<(IpAddr, u16)> {
        self.find_by_id(device_id).and_then(TrustedDevice::address)
    }

    /// Find devices that have stored addresses.
    ///
    /// Returns devices with stored addresses, sorted by last_seen (most recent first).
    #[must_use]
    pub fn get_devices_with_addresses(&self) -> Vec<&TrustedDevice> {
        let mut devices: Vec<_> = self
            .devices
            .iter()
            .filter(|d| d.last_known_ip.is_some() && d.last_known_port.is_some())
            .collect();
        devices.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
        devices
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_device() -> TrustedDevice {
        TrustedDevice::new(
            Uuid::new_v4(),
            "Test Device".to_string(),
            "test-public-key".to_string(),
        )
    }

    #[test]
    fn test_trust_store_save_and_load() {
        let tmp_dir = TempDir::new().unwrap();
        let trust_path = tmp_dir.path().join("trust.json");

        let mut store = TrustStore::load_from(trust_path.clone()).unwrap();
        let device = create_test_device();
        let device_id = device.device_id;
        store.add(device).unwrap();

        let loaded_store = TrustStore::load_from(trust_path).unwrap();
        assert_eq!(loaded_store.devices.len(), 1);
        assert!(loaded_store.find_by_id(&device_id).is_some());
    }

    #[test]
    fn test_trust_store_remove() {
        let tmp_dir = TempDir::new().unwrap();
        let trust_path = tmp_dir.path().join("trust.json");

        let mut store = TrustStore::load_from(trust_path).unwrap();
        let device = create_test_device();
        let device_id = device.device_id;
        store.add(device).unwrap();

        assert!(store.remove(&device_id).unwrap());
        assert!(store.devices.is_empty());
    }

    #[test]
    fn test_trust_store_set_trust_level() {
        let tmp_dir = TempDir::new().unwrap();
        let trust_path = tmp_dir.path().join("trust.json");

        let mut store = TrustStore::load_from(trust_path).unwrap();
        let device = create_test_device();
        let device_id = device.device_id;
        store.add(device).unwrap();

        store.set_trust_level(&device_id, TrustLevel::Full).unwrap();
        assert_eq!(
            store.find_by_id(&device_id).unwrap().trust_level,
            TrustLevel::Full
        );
    }

    #[test]
    fn test_load_nonexistent_file() {
        let tmp_dir = TempDir::new().unwrap();
        let trust_path = tmp_dir.path().join("nonexistent.json");

        let store = TrustStore::load_from(trust_path).unwrap();
        assert!(store.devices.is_empty());
    }

    #[test]
    fn test_verify_key() {
        let tmp_dir = TempDir::new().unwrap();
        let trust_path = tmp_dir.path().join("trust.json");

        let mut store = TrustStore::load_from(trust_path).unwrap();
        let device = create_test_device();
        let device_id = device.device_id;
        let public_key = device.public_key.clone();
        store.add(device).unwrap();

        assert!(store.verify_key(&device_id, &public_key));
        assert!(!store.verify_key(&device_id, "wrong-key"));
        assert!(!store.verify_key(&Uuid::new_v4(), &public_key));
    }

    #[test]
    fn test_trusted_device_address_storage() {
        let device = TrustedDevice::new(
            Uuid::new_v4(),
            "Test Device".to_string(),
            "test-public-key".to_string(),
        )
        .with_address("192.168.1.100".parse().unwrap(), 52530);

        assert!(device.last_known_ip.is_some());
        assert!(device.last_known_port.is_some());
        assert!(device.address_updated_at.is_some());
        assert_eq!(
            device.address(),
            Some(("192.168.1.100".parse().unwrap(), 52530))
        );
    }

    #[test]
    fn test_update_address() {
        let tmp_dir = TempDir::new().unwrap();
        let trust_path = tmp_dir.path().join("trust.json");

        let mut store = TrustStore::load_from(trust_path).unwrap();
        let device = create_test_device();
        let device_id = device.device_id;
        store.add(device).unwrap();

        let ip: IpAddr = "100.103.164.32".parse().unwrap();
        assert!(store.update_address(&device_id, ip, 52530).unwrap());

        let updated = store.find_by_id(&device_id).unwrap();
        assert_eq!(updated.last_known_ip, Some(ip));
        assert_eq!(updated.last_known_port, Some(52530));
        assert!(updated.address_updated_at.is_some());
    }

    #[test]
    fn test_get_devices_with_addresses() {
        let tmp_dir = TempDir::new().unwrap();
        let trust_path = tmp_dir.path().join("trust.json");

        let mut store = TrustStore::load_from(trust_path).unwrap();

        let device1 = TrustedDevice::new(
            Uuid::new_v4(),
            "Device 1".to_string(),
            "key1".to_string(),
        )
        .with_address("192.168.1.1".parse().unwrap(), 52530);

        let device2 = TrustedDevice::new(
            Uuid::new_v4(),
            "Device 2".to_string(),
            "key2".to_string(),
        );

        let device3 = TrustedDevice::new(
            Uuid::new_v4(),
            "Device 3".to_string(),
            "key3".to_string(),
        )
        .with_address("192.168.1.3".parse().unwrap(), 52540);

        store.add(device1).unwrap();
        store.add(device2).unwrap();
        store.add(device3).unwrap();

        let devices_with_addr = store.get_devices_with_addresses();
        assert_eq!(devices_with_addr.len(), 2);
    }

    #[test]
    fn test_backward_compatibility_no_address() {
        let tmp_dir = TempDir::new().unwrap();
        let trust_path = tmp_dir.path().join("trust.json");

        let mut store = TrustStore::load_from(trust_path.clone()).unwrap();
        let device = create_test_device();
        let device_id = device.device_id;
        store.add(device).unwrap();

        let loaded_store = TrustStore::load_from(trust_path).unwrap();
        let loaded_device = loaded_store.find_by_id(&device_id).unwrap();
        assert!(loaded_device.last_known_ip.is_none());
        assert!(loaded_device.last_known_port.is_none());
        assert!(loaded_device.address_updated_at.is_none());
    }
}
