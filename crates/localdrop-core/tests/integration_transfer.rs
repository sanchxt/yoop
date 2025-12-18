//! Integration tests for `LocalDrop` file transfers.
//!
//! These tests verify end-to-end transfer functionality including:
//! - Single file transfers
//! - Multiple file transfers
//! - Large file transfers (multi-chunk)
//! - Error handling (invalid codes, decline, etc.)
//!
//! Note: Most tests are ignored in CI because they rely on UDP broadcast
//! discovery which doesn't work reliably in CI environments (especially macOS).

mod common;

use std::time::Duration;

use localdrop_core::code::ShareCode;
use localdrop_core::transfer::{ReceiveSession, ShareSession, TransferConfig, TransferState};

use common::{
    assert_files_equal, create_temp_dir, create_test_directory, create_test_file, get_test_ports,
    random_bytes,
};

fn test_config() -> TransferConfig {
    let (discovery_port, transfer_port) = get_test_ports();
    TransferConfig {
        discovery_port,
        transfer_port,
        discovery_timeout: Duration::from_secs(10),
        broadcast_interval: Duration::from_millis(100),
        ..Default::default()
    }
}

/// Test transferring a single small file.
#[tokio::test]
#[ignore = "Requires UDP broadcast which doesn't work in CI"]
async fn test_single_file_transfer() {
    let temp_dir = create_temp_dir();
    let test_content = b"Hello, LocalDrop! This is a test file.";
    let test_file = create_test_file(temp_dir.path(), "test.txt", test_content);
    let output_dir = temp_dir.path().join("output");
    std::fs::create_dir_all(&output_dir).unwrap();

    let config = test_config();

    let mut share_session = ShareSession::new(std::slice::from_ref(&test_file), config.clone())
        .await
        .expect("Failed to create share session");
    let code = share_session.code().clone();

    let share_handle = tokio::spawn(async move { share_session.wait().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut receive_session = ReceiveSession::connect(&code, output_dir.clone(), config)
        .await
        .expect("Failed to connect to share");

    let files = receive_session.files();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].size, test_content.len() as u64);

    receive_session
        .accept()
        .await
        .expect("Failed to accept transfer");

    share_handle
        .await
        .expect("Share task panicked")
        .expect("Share failed");

    let received_file = output_dir.join("test.txt");
    assert!(received_file.exists(), "Received file not found");
    assert_files_equal(&test_file, &received_file);
}

/// Test transferring multiple files.
#[tokio::test]
#[ignore = "Requires UDP broadcast which doesn't work in CI"]
async fn test_multiple_files_transfer() {
    let temp_dir = create_temp_dir();

    let file1_content = b"First file content";
    let file2_content = b"Second file content with more data";
    let file3_content = b"Third file!";

    let file1 = create_test_file(temp_dir.path(), "file1.txt", file1_content);
    let file2 = create_test_file(temp_dir.path(), "file2.txt", file2_content);
    let file3 = create_test_file(temp_dir.path(), "file3.txt", file3_content);

    let output_dir = temp_dir.path().join("output");
    std::fs::create_dir_all(&output_dir).unwrap();

    let config = test_config();

    let mut share_session = ShareSession::new(
        &[file1.clone(), file2.clone(), file3.clone()],
        config.clone(),
    )
    .await
    .expect("Failed to create share session");
    let code = share_session.code().clone();

    let share_handle = tokio::spawn(async move { share_session.wait().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut receive_session = ReceiveSession::connect(&code, output_dir.clone(), config)
        .await
        .expect("Failed to connect to share");

    assert_eq!(receive_session.files().len(), 3);

    receive_session
        .accept()
        .await
        .expect("Failed to accept transfer");

    share_handle
        .await
        .expect("Share task panicked")
        .expect("Share failed");

    assert_files_equal(&file1, &output_dir.join("file1.txt"));
    assert_files_equal(&file2, &output_dir.join("file2.txt"));
    assert_files_equal(&file3, &output_dir.join("file3.txt"));
}

/// Test transferring a large file that requires multiple chunks.
#[tokio::test]
#[ignore = "Requires UDP broadcast which doesn't work in CI"]
async fn test_large_file_transfer() {
    let temp_dir = create_temp_dir();
    let large_content = random_bytes(3 * 1024 * 1024); // 3 MB
    let test_file = create_test_file(temp_dir.path(), "large.bin", &large_content);
    let output_dir = temp_dir.path().join("output");
    std::fs::create_dir_all(&output_dir).unwrap();

    let config = test_config();

    let mut share_session = ShareSession::new(std::slice::from_ref(&test_file), config.clone())
        .await
        .expect("Failed to create share session");
    let code = share_session.code().clone();

    let _progress_rx = share_session.progress();

    let share_handle = tokio::spawn(async move { share_session.wait().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut receive_session = ReceiveSession::connect(&code, output_dir.clone(), config)
        .await
        .expect("Failed to connect to share");

    let files = receive_session.files();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].size, large_content.len() as u64);

    receive_session
        .accept()
        .await
        .expect("Failed to accept transfer");

    share_handle
        .await
        .expect("Share task panicked")
        .expect("Share failed");

    let received_file = output_dir.join("large.bin");
    assert!(received_file.exists(), "Received file not found");
    assert_files_equal(&test_file, &received_file);
}

/// Test that an invalid share code is rejected.
#[tokio::test]
async fn test_invalid_code_rejection() {
    let temp_dir = create_temp_dir();
    let output_dir = temp_dir.path().join("output");
    std::fs::create_dir_all(&output_dir).unwrap();

    let config = test_config();

    let fake_code = ShareCode::parse("ZZZZ").expect("Invalid test code");

    let result = ReceiveSession::connect(&fake_code, output_dir, config).await;

    assert!(
        result.is_err(),
        "Expected connection to fail with invalid code"
    );
}

/// Test that declining a transfer works correctly.
#[tokio::test]
#[ignore = "Requires UDP broadcast which doesn't work in CI"]
async fn test_transfer_decline() {
    let temp_dir = create_temp_dir();
    let test_content = b"File that will be declined";
    let test_file = create_test_file(temp_dir.path(), "declined.txt", test_content);
    let output_dir = temp_dir.path().join("output");
    std::fs::create_dir_all(&output_dir).unwrap();

    let config = test_config();

    let mut share_session = ShareSession::new(std::slice::from_ref(&test_file), config.clone())
        .await
        .expect("Failed to create share session");
    let code = share_session.code().clone();

    let share_handle = tokio::spawn(async move { share_session.wait().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut receive_session = ReceiveSession::connect(&code, output_dir.clone(), config)
        .await
        .expect("Failed to connect to share");

    assert_eq!(receive_session.files().len(), 1);

    receive_session.decline().await;

    let _share_result = share_handle.await.expect("Share task panicked");

    let declined_file = output_dir.join("declined.txt");
    assert!(
        !declined_file.exists(),
        "File should not exist after decline"
    );
}

/// Test directory transfer with nested structure.
#[tokio::test]
#[ignore = "Requires UDP broadcast which doesn't work in CI"]
async fn test_directory_transfer() {
    let temp_dir = create_temp_dir();
    let test_dir = create_test_directory(temp_dir.path(), "mydir");
    let output_dir = temp_dir.path().join("output");
    std::fs::create_dir_all(&output_dir).unwrap();

    let config = test_config();

    let mut share_session = ShareSession::new(std::slice::from_ref(&test_dir), config.clone())
        .await
        .expect("Failed to create share session");
    let code = share_session.code().clone();

    let share_handle = tokio::spawn(async move { share_session.wait().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut receive_session = ReceiveSession::connect(&code, output_dir.clone(), config)
        .await
        .expect("Failed to connect to share");

    assert_eq!(receive_session.files().len(), 3);

    receive_session
        .accept()
        .await
        .expect("Failed to accept transfer");

    share_handle
        .await
        .expect("Share task panicked")
        .expect("Share failed");

    let received_file_count = walkdir::WalkDir::new(&output_dir)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_file())
        .count();

    assert_eq!(
        received_file_count, 3,
        "Expected 3 files in output directory"
    );
}

/// Test progress tracking during transfer.
#[tokio::test]
#[ignore = "Requires UDP broadcast which doesn't work in CI"]
async fn test_progress_tracking() {
    let temp_dir = create_temp_dir();
    let content = random_bytes(2 * 1024 * 1024);
    let test_file = create_test_file(temp_dir.path(), "progress_test.bin", &content);
    let output_dir = temp_dir.path().join("output");
    std::fs::create_dir_all(&output_dir).unwrap();

    let config = test_config();

    let mut share_session = ShareSession::new(std::slice::from_ref(&test_file), config.clone())
        .await
        .expect("Failed to create share session");
    let code = share_session.code().clone();
    let _sender_progress = share_session.progress();

    let share_handle = tokio::spawn(async move { share_session.wait().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut receive_session = ReceiveSession::connect(&code, output_dir.clone(), config)
        .await
        .expect("Failed to connect to share");
    let receiver_progress = receive_session.progress();

    receive_session
        .accept()
        .await
        .expect("Failed to accept transfer");

    share_handle
        .await
        .expect("Share task panicked")
        .expect("Share failed");

    let final_progress = receiver_progress.borrow().clone();
    assert_eq!(
        final_progress.state,
        TransferState::Completed,
        "Transfer should be completed"
    );
    assert_eq!(
        final_progress.total_bytes_transferred, final_progress.total_bytes,
        "All bytes should be transferred"
    );
}
