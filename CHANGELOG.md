# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

-   **Transfer resume functionality** (yoop-core)

    -   Persistent state management for interrupted transfers
    -   Resume from last successful chunk
    -   State file cleanup on completion
    -   New protocol messages: `ResumeRequest`, `ResumeAck`
    -   Cross-platform file permissions and symlink handling
    -   Comprehensive resume tests

-   **mDNS/DNS-SD discovery** (yoop-core)

    -   Service discovery via mDNS on `_yoop._tcp.local`
    -   Hybrid discovery combining UDP broadcast and mDNS
    -   Automatic fallback between discovery methods
    -   Proper mDNS daemon lifecycle management
    -   Cross-platform service advertising

-   **Trusted device transfers** (yoop-core)

    -   Device identity with Ed25519 keypair generation and storage
    -   Beacon discovery system for trusted device detection
    -   New protocol messages: `TrustedHello`, `TrustedHelloAck` with signature verification
    -   `TrustedSendSession` and `TrustedReceiveSession` for authenticated transfers
    -   Post-transfer trust prompts in share and receive workflows
    -   Device identity exchange during regular transfers
    -   Trust levels: Full (auto-accept) and AskEachTime (confirm each transfer)

-   **History module** (yoop-core)

    -   SQLite-based transfer history storage
    -   Track sent and received files with timestamps, sizes, and peer info
    -   Automatic cleanup of old entries (configurable retention)
    -   Query API for recent transfers and statistics

-   **Web interface** (yoop-core)

    -   Full-featured web UI for file sharing and receiving
    -   Server-sent events (SSE) for real-time progress updates
    -   Embedded assets (HTML, CSS, JavaScript)
    -   Authentication support with generated passwords
    -   File upload and download endpoints
    -   Active share listing and management
    -   Localhost-only mode for security

-   **Clipboard sharing module** (yoop-core)

    -   `ClipboardContent` type supporting text and PNG images
    -   `ClipboardMetadata` for content type, size, and checksum
    -   `NativeClipboard` wrapper using arboard for cross-platform access
    -   `ClipboardWatcher` for polling-based change detection (500ms interval)
    -   `ClipboardShareSession` for one-shot clipboard sharing
    -   `ClipboardReceiveSession` for one-shot clipboard receiving with keep-alive
    -   `ClipboardSyncSession` for live bidirectional clipboard synchronization
    -   New protocol message types: `ClipboardMeta`, `ClipboardData`, `ClipboardAck`, `ClipboardChanged`, `ClipboardRequest`
    -   xxHash64 content hashing for efficient change detection
    -   Linux clipboard holder subprocess for ownership persistence

-   **CLI commands** (yoop-cli)

    -   **clipboard**: Full clipboard sharing implementation
        -   `share` - Share current clipboard content with a code
        -   `receive <code>` - Receive clipboard content using a code
        -   `sync [code]` - Start or join bidirectional clipboard sync
        -   JSON output mode (`--json`) for scripting
        -   Quiet mode (`--quiet`) for minimal output
        -   Batch mode (`--batch`) for non-interactive receiving
    -   **send**: Send files directly to trusted devices without codes
        -   Direct transfer to trusted devices by name
        -   Integration with trust system
        -   Compression and transfer settings support
    -   **scan**: Network scanning for active shares
        -   Discover active shares on local network
        -   Interactive connection mode
        -   Configurable scan duration
        -   JSON output support
    -   **diagnose**: Network diagnostics
        -   Network interface detection
        -   Port availability checks
        -   Firewall status detection
        -   mDNS service verification
        -   Comprehensive network troubleshooting
    -   **history**: Transfer history management
        -   View recent sent and received transfers
        -   Filter by type (sent/received)
        -   Clear history option
        -   JSON output for scripting
    -   **config**: Complete configuration management
        -   Get/set all 34 configuration keys
        -   Show all configuration sections
        -   List available configuration keys
        -   Display configuration file path
        -   Reset to defaults
        -   Config integration across all commands
    -   **trust**: Full trust management implementation
        -   List trusted devices with transfer counts
        -   Remove devices from trust store
        -   Set trust level (full or ask_each_time)
        -   Integration with device identity system

-   **UI improvements** (yoop-cli)
    -   Centered Unicode box for share code display
    -   Live countdown timer for code expiration
    -   Progress indicators for clipboard operations
    -   Improved error messages and user prompts

### Changed

-   Config fallbacks integrated into all commands (share, receive, send, clipboard, web, scan, diagnose, history)
-   Share and receive commands now prompt for device trust after successful transfers
-   Discovery now uses hybrid approach (UDP + mDNS) for better reliability

### Fixed

-   mDNS daemon lifecycle and cleanup
-   Channel closure errors in mDNS operations
-   Connection keep-alive during transfer prompts
-   Clipboard sync argument handling
-   Various clippy warnings and formatting issues

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
