# Integration Tests

This directory contains integration tests for Yoop.

## Running Tests

```bash
# Run all tests (unit + integration)
cargo test --workspace

# Run only integration tests
cargo test --test '*'

# Run specific integration test
cargo test --test test_name

# Run with output
cargo test --workspace -- --nocapture
```

## Test Categories

### Unit Tests

Unit tests are located alongside the source code in `crates/*/src/` files under `#[cfg(test)]` modules.

### Integration Tests

Integration tests in this directory test the public API and cross-module interactions:

-   **Transfer tests**: End-to-end file transfer scenarios
-   **Discovery tests**: Network discovery on loopback
-   **Protocol tests**: LDRP protocol compliance
-   **CLI tests**: Command-line interface behavior

## Writing Tests

### Test Utilities

The `common` module provides shared utilities:

```rust
use crate::common::*;

#[tokio::test]
async fn test_something() {
    let temp_dir = create_temp_dir();
    // ... test code
}
```

### Async Tests

Use `#[tokio::test]` for async tests:

```rust
#[tokio::test]
async fn test_async_operation() {
    let result = some_async_function().await;
    assert!(result.is_ok());
}
```

### Network Tests

For tests that require network access, use loopback addresses:

```rust
let addr = "127.0.0.1:0".parse().unwrap();
```

## Test Data

Place test fixtures in `tests/fixtures/` (create as needed).
