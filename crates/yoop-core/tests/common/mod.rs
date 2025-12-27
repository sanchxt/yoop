//! Common test utilities for `Yoop` integration tests.
//!
//! This module provides shared functionality for integration tests.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU16, Ordering};

/// Base port for tests, incremented for each test to avoid conflicts.
static TEST_PORT_COUNTER: AtomicU16 = AtomicU16::new(52600);

/// Create a temporary directory for test files.
///
/// The directory will be automatically cleaned up when the returned
/// `TempDir` is dropped.
pub fn create_temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("Failed to create temp directory")
}

/// Create a test file with the given content.
pub fn create_test_file(dir: &std::path::Path, name: &str, content: &[u8]) -> PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create parent directories");
    }
    std::fs::write(&path, content).expect("Failed to write test file");
    path
}

/// Generate random bytes for testing.
pub fn random_bytes(size: usize) -> Vec<u8> {
    use rand::RngCore;
    let mut bytes = vec![0u8; size];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes
}

/// Assert that two files have identical content.
pub fn assert_files_equal(path1: &std::path::Path, path2: &std::path::Path) {
    let content1 = std::fs::read(path1).expect("Failed to read first file");
    let content2 = std::fs::read(path2).expect("Failed to read second file");
    assert_eq!(content1, content2, "File contents differ");
}

/// Get unique ports for a test to avoid conflicts between parallel tests.
/// Returns (`discovery_port`, `transfer_port`).
pub fn get_test_ports() -> (u16, u16) {
    let base = TEST_PORT_COUNTER.fetch_add(2, Ordering::SeqCst);
    (base, base + 1)
}

/// Create a test directory structure with multiple files.
pub fn create_test_directory(base: &std::path::Path, name: &str) -> PathBuf {
    let dir = base.join(name);
    std::fs::create_dir_all(&dir).expect("Failed to create test directory");

    create_test_file(&dir, "file1.txt", b"Hello, Yoop!");
    create_test_file(&dir, "file2.txt", b"Second test file content");
    create_test_file(&dir, "subdir/nested.txt", b"Nested file in subdirectory");

    dir
}
