//! Integration tests for sync session lifecycle.

use tempfile::TempDir;
use tokio::fs;
use yoop_core::sync::{SyncConfig, SyncSession};
use yoop_core::transfer::TransferConfig;

#[tokio::test]
#[ignore = "Requires UDP broadcast which doesn't work in CI"]
async fn test_sync_session_index_exchange() {
    let host_dir = TempDir::new().unwrap();
    let client_dir = TempDir::new().unwrap();

    fs::write(host_dir.path().join("file1.txt"), b"hello")
        .await
        .unwrap();
    fs::write(host_dir.path().join("file2.txt"), b"world")
        .await
        .unwrap();

    let host_config = SyncConfig {
        sync_root: host_dir.path().to_path_buf(),
        ..Default::default()
    };

    let _client_config = SyncConfig {
        sync_root: client_dir.path().to_path_buf(),
        ..Default::default()
    };

    let transfer_config = TransferConfig::default();

    let host_task =
        tokio::spawn(async move { SyncSession::host(host_config, transfer_config).await });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let Ok(Ok(Ok((code, _session)))) =
        tokio::time::timeout(tokio::time::Duration::from_secs(1), host_task).await
    else {
        panic!("Host setup failed")
    };

    assert_eq!(code.as_str().len(), 4);
}

#[tokio::test]
async fn test_sync_config_default() {
    let config = SyncConfig::default();
    assert_eq!(config.debounce_ms, 100);
    assert!(config.sync_deletions);
    assert!(!config.follow_symlinks);
}

#[tokio::test]
#[ignore = "Requires network connection which blocks indefinitely in CI"]
async fn test_sync_session_debug() {
    let temp_dir = TempDir::new().unwrap();
    let config = SyncConfig {
        sync_root: temp_dir.path().to_path_buf(),
        ..Default::default()
    };
    let transfer_config = TransferConfig::default();

    let result = SyncSession::host(config, transfer_config).await;
    assert!(result.is_ok());
}
