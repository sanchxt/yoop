# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.6](https://github.com/sanchxt/yoop/compare/v0.1.5...v0.1.6) (2026-01-23)


### Features

* **transfer:** add zstd compression support ([45b62a8](https://github.com/sanchxt/yoop/commit/45b62a8bb4c59d8f8811312e4601ee577027df5c))


### Bug Fixes

* **update:** detect package manager from installed yoop location ([#21](https://github.com/sanchxt/yoop/issues/21)) ([1a6121e](https://github.com/sanchxt/yoop/commit/1a6121e8f04bf16ac7915f47efefd0108ed2d7d8))

## [0.1.5](https://github.com/sanchxt/yoop/compare/v0.1.4...v0.1.5) (2026-01-21)


### Features

* **sync:** implement live bidirectional directory synchronization ([c966b7c](https://github.com/sanchxt/yoop/commit/c966b7cf0078140d9a9a9ea8ecb65509bd69799e))

## [0.1.4](https://github.com/sanchxt/yoop/compare/v0.1.3...v0.1.4) (2026-01-12)


### Features

* add self-update command with automatic migrations ([#16](https://github.com/sanchxt/yoop/issues/16)) ([ff89a41](https://github.com/sanchxt/yoop/commit/ff89a41d4c827f1eb5eabe2d16a80cc9eafcb615))
* **qr:** add QR code support for share codes ([#14](https://github.com/sanchxt/yoop/issues/14)) ([36ab009](https://github.com/sanchxt/yoop/commit/36ab009a71ff503590601c1f951e983fb372b777))

## [0.1.3](https://github.com/sanchxt/yoop/compare/v0.1.2...v0.1.3) (2026-01-07)


### Features

* **completions:** add install and uninstall commands with shell detection ([aacb96c](https://github.com/sanchxt/yoop/commit/aacb96cfc04c971ff857e9fe221a5f52c0c0b9ec))
* **preview:** complete thumbnail and archive listing generation ([96cf9f1](https://github.com/sanchxt/yoop/commit/96cf9f1668a70558cbb57534f4ef12f76d4db505))
* **preview:** integrate file previews into transfer flow ([2f9c516](https://github.com/sanchxt/yoop/commit/2f9c516f44c1a525b52c94259bfc49f06ff9a2e5))
* shell completions and preview system ([38f8354](https://github.com/sanchxt/yoop/commit/38f8354597c27250358f7163532319c1407238a5))


### Bug Fixes

* **discovery:** use dynamic port allocation in hybrid tests ([747ff5e](https://github.com/sanchxt/yoop/commit/747ff5eb3d84276dd5dfb2c115f7c8a56831c878))
* **transfer:** resolve directory transfer hang at 0 bits ([31ab31a](https://github.com/sanchxt/yoop/commit/31ab31a3c582bc315cd55a702b63a84d233a9785))

## [0.1.2](https://github.com/sanchxt/yoop/compare/v0.1.1...v0.1.2) (2025-12-28)


### Bug Fixes

* auto-sync npm package versions in release-please workflow ([482093c](https://github.com/sanchxt/yoop/commit/482093cbc64c55f7f9b4f8822539976b7db015b6))

## [0.1.1](https://github.com/sanchxt/yoop/compare/v0.1.0...v0.1.1) (2025-12-28)


### Features

* Add cross-platform clipboard syncing and sharing functionality ([c2a1aba](https://github.com/sanchxt/yoop/commit/c2a1aba60e4466c10e2910b25f6e0528ed8b7ea7))
* Add diagnose, history, scan commands ([#3](https://github.com/sanchxt/yoop/issues/3)) ([04664dd](https://github.com/sanchxt/yoop/commit/04664ddcc93cfcdca0aef2011f7fa758602372d1))
* Add feature to store trusted devices and quick-accept them, and configs to manage settings ([#5](https://github.com/sanchxt/yoop/issues/5)) ([b047522](https://github.com/sanchxt/yoop/commit/b04752280c88a0cadab1435dfc28ec00c441b91c))
* Add npm package distribution for global CLI installation ([0b30315](https://github.com/sanchxt/yoop/commit/0b30315967b9dde507b835e2b7f8a5d1dc1e1ec3))
* add transfer resume and mDNS/DNS-SD discovery ([826907e](https://github.com/sanchxt/yoop/commit/826907e4ace769adf1f79bd05ef1bd45dbfb14dd))
* improve share code display with centered Unicode box and live countdown ([22d1eb9](https://github.com/sanchxt/yoop/commit/22d1eb9c76df102a1a08de0052b2782b0deb2188))
* initial release v0.1.0 ([3660df7](https://github.com/sanchxt/yoop/commit/3660df73afe3f1625fcb10362fb07f3ff6173f5a))
* preserve directory structure in file transfers ([#9](https://github.com/sanchxt/yoop/issues/9)) ([5b803dd](https://github.com/sanchxt/yoop/commit/5b803ddffba764685b315003b40d2a1bd606fa1d))


### Bug Fixes

* add connection keep-alive during transfer prompt ([5afee02](https://github.com/sanchxt/yoop/commit/5afee02be578cd430c13c7caf1b5d79bc289f802))
* Handle already-published packages gracefully in npm workflow ([22f6cc5](https://github.com/sanchxt/yoop/commit/22f6cc5fe75124812e523144245d356155be3a31))
* handle mdns' asynchronous nature ([8bc740e](https://github.com/sanchxt/yoop/commit/8bc740ec93206e54e03221b1975d3b381eff1d79))
* Implement clipboard's sync argument ([7e8e33b](https://github.com/sanchxt/yoop/commit/7e8e33bf969f46b65b21c350b461c6bafb1d49ea))
* improve keep-alive logic ([b78698b](https://github.com/sanchxt/yoop/commit/b78698b7e5c86b9a0647a363a01403b07ecd6e6d))
* properly shutdown mDNS daemon on drop ([fe4ff26](https://github.com/sanchxt/yoop/commit/fe4ff26463fa7f169b6831d58b6bb5c35db84f34))
* resolve channel closure errors in mDNS cleanup ([aa4db47](https://github.com/sanchxt/yoop/commit/aa4db47fe151ce201b8aeb99c5a9515ea9ae73f6))
* resolve clippy warnings ([a0279a4](https://github.com/sanchxt/yoop/commit/a0279a40522e9d9e1f0c103e728710c2cdd0db27))
* show waiting message before blocking host() call in clipboard sync ([c59e8e4](https://github.com/sanchxt/yoop/commit/c59e8e4beee0a3d5cccab74744efeeca1197fb64))
* Use macos-latest for both macOS targets to avoid runner availability issues ([42bcc15](https://github.com/sanchxt/yoop/commit/42bcc15e903d204d7a5e14a87129935a35641fa6))
* use simple release-type for cargo workspace ([84dc30a](https://github.com/sanchxt/yoop/commit/84dc30ad5a07782c3f9ba2f1fe74225054853918))


### Refactoring

* Change name to Yoop ([4baabbc](https://github.com/sanchxt/yoop/commit/4baabbcd3fe0ae322b2bba27b11eba012f8d042e))
* split ClipboardSyncSession::host() to show code before blocking ([0bf1572](https://github.com/sanchxt/yoop/commit/0bf157207012abf884757b4db0c93cd32e7d8800))

## [0.1.0] - 2025-12-27

### Added

#### Core Library (yoop-core)

-   **Discovery module**: UDP broadcast-based network discovery with collision detection
    -   Broadcasting on port 52525 with configurable intervals
    -   mDNS/DNS-SD support for device discovery
    -   Discovery packet serialization with device metadata
-   **Code module**: 4-character share code generation and validation
    -   Charset of 32 characters (2-9, A-H, J-K, M, N, P-Z) avoiding ambiguous characters
    -   Code collision detection and regeneration
    -   Expiration tracking (default 5 minutes, configurable 1-30 minutes)
-   **Transfer module**: Complete file transfer engine
    -   Share session management with progress tracking
    -   Receive session with preview and acceptance flow
    -   Multi-file transfer support
    -   Transfer state machine (Waiting, Connected, Transferring, Completed, etc.)
-   **Protocol module**: LDRP (Yoop Protocol) wire format implementation
    -   Binary frame format with magic bytes, version, type, and length
    -   Message types: HELLO, FILE_LIST, CHUNK_DATA, CHUNK_ACK, etc.
    -   Efficient chunk data encoding/decoding
-   **Crypto module**: Security primitives
    -   TLS 1.3 configuration with rustls
    -   Self-signed certificate generation using rcgen
    -   HMAC-SHA256 for code verification (timing-attack resistant)
    -   xxHash64 for fast chunk verification
    -   SHA-256 for complete file integrity
    -   Constant-time comparison utilities
-   **File module**: File operations and chunking
    -   File metadata extraction (size, MIME type, timestamps)
    -   Recursive directory enumeration with walkdir
    -   Chunked file reading (default 1MB chunks, configurable)
    -   Path sanitization to prevent directory traversal
    -   FileWriter for receiving and assembling chunks
-   **Config module**: Configuration management system
    -   TOML-based configuration files
    -   Platform-specific config directory detection
    -   Hierarchical config structure (general, network, transfer, security, etc.)
    -   Human-readable duration serialization
-   **Error module**: Comprehensive error handling
    -   Error codes (E001-E010) for common failures
    -   Detailed error context and recovery information
    -   Integration with std::io::Error and thiserror
-   **Preview module** (stub): File preview generation scaffolding
    -   Preview type enumeration (Thumbnail, Text, Archive, Icon)
    -   Text preview implementation
    -   Image thumbnail and archive listing (TODO)
-   **Trust module** (stub): Trusted devices management scaffolding
    -   Trust level enumeration (Full, AskEachTime)
    -   Device record structure with Ed25519 key placeholders
    -   Trust store API (TODO)
-   **Web module** (stub): Web server scaffolding
    -   WebSocket message types defined
    -   Configuration structure
    -   API endpoint planning (TODO)

#### CLI Application (yoop)

-   **Command structure**: Comprehensive CLI using clap
    -   `share` - Share files and directories with progress display
    -   `receive` - Receive files with code, preview support, and acceptance prompt
    -   `send` - Send to trusted devices (stub)
    -   `scan` - Scan network for active shares (stub)
    -   `trust` - Manage trusted devices (stub)
    -   `web` - Start web interface (stub)
    -   `config` - Configuration management (stub)
    -   `diagnose` - Network diagnostics (stub)
    -   `history` - Transfer history (stub)
-   **Progress tracking**: Real-time transfer progress with percentage, speed, and ETA
-   **JSON output mode**: Machine-readable output for scripting
-   **Batch mode**: Non-interactive operation for automation
-   **Logging**: Integrated tracing with configurable log levels

#### Testing & Quality

-   **Unit tests**: 41 unit tests covering core functionality
    -   Code generation and validation
    -   Protocol encoding/decoding
    -   File chunking and assembly
    -   Cryptographic operations
    -   Configuration serialization
    -   Discovery packet handling
-   **Integration tests**: 7 end-to-end tests
    -   Single file transfer
    -   Multiple file transfer
    -   Large file transfer (multi-chunk)
    -   Directory transfer
    -   Transfer decline
    -   Invalid code rejection
    -   Progress tracking
-   **Test utilities**: Common test helpers for temp files, port allocation, and assertions

#### Infrastructure

-   **Cargo workspace**: Multi-crate structure with yoop-core and yoop-cli
-   **GitHub Actions CI**:
    -   Format checking (rustfmt)
    -   Linting (clippy with -D warnings)
    -   Testing across multiple Rust versions
    -   Build verification
    -   Documentation generation
    -   MSRV enforcement (Rust 1.86.0)
-   **Pre-commit hooks**: Automated formatting and linting on commit
    -   `cargo fmt --all --check`
    -   `cargo clippy --workspace -- -D warnings`
-   **Dual licensing**: MIT and Apache-2.0
-   **Documentation**: README, CHANGELOG, CONTRIBUTING, and inline code documentation
-   **Editor configuration**: .editorconfig for consistent code style
-   **Makefile**: Common development tasks (build, test, lint, format)

### Performance

-   Chunked transfers with configurable chunk size (default 1MB)
-   Zero-copy operations where possible
-   Efficient binary protocol (LDRP)
-   xxHash64 for fast chunk verification
-   Parallel chunk streaming support (up to 4 concurrent, configurable)

### Security

-   TLS 1.3 encryption for all transfers
-   Perfect forward secrecy via ephemeral ECDH keys
-   HMAC-based code verification preventing timing attacks
-   Rate limiting (3 failed attempts → 30s lockout)
-   Path sanitization preventing directory traversal
-   No data leaves local network
