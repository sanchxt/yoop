# LocalDrop

**Cross-Platform Local Network File Sharing**

[![CI](https://github.com/arceus/localdrop/workflows/CI/badge.svg)](https://github.com/arceus/localdrop/actions)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)
[![Rust Version](https://img.shields.io/badge/rust-1.86.0%2B-blue.svg)](https://www.rust-lang.org)

LocalDrop enables seamless peer-to-peer file transfers over local networks using simple, time-limited codes. Unlike cloud-based solutions, all data stays on your local network, ensuring privacy, speed, and zero bandwidth costs.

## Features

### Core Features

-   **Cross-platform**: Works on Windows, Linux, and macOS
-   **No account required**: Zero configuration, no cloud dependency
-   **Simple 4-character codes**: Easy discovery without IP addresses
-   **Dual discovery**: UDP broadcast + mDNS/DNS-SD for reliable device discovery
-   **Private & secure**: TLS 1.3 encryption, data never leaves local network
-   **Fast transfers**: Chunked transfers with xxHash64 verification
-   **Resume capability**: Interrupted transfers can be resumed automatically
-   **CLI interface**: Full-featured command-line tool
-   **Web interface**: Browser-based UI for devices without CLI access
-   **Clipboard sharing**: One-shot transfer and live bidirectional sync

## Quick Start

### Share Files

```bash
# Share a single file
localdrop share document.pdf

# Share multiple files and folders
localdrop share photos/ videos/ notes.md

# Share with custom expiration
localdrop share project.zip --expire 10m
```

### Receive Files

```bash
# Receive using the 4-character code
localdrop receive A7K9

# Receive to specific directory
localdrop receive A7K9 --output ~/Downloads/

# Batch mode (auto-accept)
localdrop receive A7K9 --batch
```

### Clipboard Sharing

```bash
# Share current clipboard content (generates a code)
localdrop clipboard share

# Receive clipboard content using a code
localdrop clipboard receive A7K9

# Start bidirectional clipboard sync (host)
localdrop clipboard sync

# Join existing sync session
localdrop clipboard sync A7K9
```

## Installation

### From Source

Requires **Rust 1.86.0** or later.

```bash
git clone https://github.com/arceus/localdrop
cd localdrop
cargo install --path crates/localdrop-cli
```

### Pre-built Binaries

Pre-built binaries and package manager support are planned for future releases.

## How It Works

1. **Sender** shares files and gets a 4-character code (e.g., `A 7 K 9`)
2. **Receiver** enters the code on their device
3. **Discovery** happens via UDP broadcast + mDNS on local network
4. **Transfer** occurs directly over TLS 1.3 encrypted TCP connection
5. **Verification** using xxHash64 per chunk, SHA-256 for complete file
6. **Resume** automatic resumption of interrupted transfers from last checkpoint

```
┌─────────────┐           UDP Broadcast            ┌──────────────┐
│   Sender    │ ◄────────  Code: A7K9  ──────────► │  Receiver    │
│             │                                    │              │
│ Share A7K9  │           TCP + TLS 1.3            │ Receive A7K9 │
│             │ ────────►  File Data  ───────────► │              │
└─────────────┘                                    └──────────────┘
```

## CLI Commands

```bash
# Sharing & Receiving
localdrop share <files...>           # Share files/folders
localdrop receive <code>             # Receive with code

# Clipboard Sharing
localdrop clipboard share            # Share clipboard content
localdrop clipboard receive <code>   # Receive clipboard content
localdrop clipboard sync [code]      # Bidirectional clipboard sync

# Utilities
localdrop scan                       # Scan for active shares on network
localdrop web                        # Start web interface
localdrop config                     # Manage configuration
localdrop diagnose                   # Network diagnostics
localdrop history                    # View transfer history

# Planned Features
localdrop send <device> <files>      # Send to trusted device (in development)
localdrop trust list                 # Manage trusted devices (in development)
```

## Configuration

LocalDrop can be configured via TOML files:

-   **Linux**: `~/.config/localdrop/config.toml`
-   **macOS**: `~/Library/Application Support/LocalDrop/config.toml`
-   **Windows**: `%APPDATA%\LocalDrop\config.toml`

Example configuration:

```toml
[general]
device_name = "My-Laptop"
default_expire = "5m"

[network]
port = 52525
ipv6 = true

[transfer]
chunk_size = 1048576
parallel_chunks = 4
verify_checksum = true

[security]
tls_verify = true
rate_limit_attempts = 3
```

## Development

### Prerequisites

-   **Rust**: 1.86.0 or later
-   **Git**: For cloning the repository

### Building

```bash
# Clone repository
git clone https://github.com/arceus/localdrop
cd localdrop

# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Run with logging
RUST_LOG=debug cargo run --bin localdrop -- share test.txt
```

### Running Tests

```bash
# All tests
cargo test --workspace

# Unit tests only
cargo test --lib --workspace

# Integration tests only
cargo test --test integration_transfer

# With output
cargo test -- --nocapture
```

### Code Quality

```bash
# Format code
cargo fmt --all

# Lint with clippy
cargo clippy --workspace -- -D warnings

# Check without building
cargo check --workspace
```

## Architecture

LocalDrop uses a custom binary protocol (LDRP) over TLS 1.3:

-   **Discovery**: UDP broadcast + mDNS/DNS-SD on port 52525
-   **Transfer**: TCP on ports 52530-52540
-   **Encryption**: TLS 1.3 with self-signed ephemeral certificates
-   **Integrity**: xxHash64 per chunk, SHA-256 per file
-   **Resume**: State persistence for interrupted transfer recovery
-   **Code Format**: 4 characters from `[2-9A-HJ-KMN-Z]` (avoiding ambiguous chars)

## Security

LocalDrop prioritizes security and privacy:

-   **Encryption**: All transfers use TLS 1.3 with perfect forward secrecy
-   **No persistence**: Ephemeral certificates, no long-term keys (except trusted devices)
-   **Rate limiting**: 3 failed attempts → 30 second lockout
-   **Local only**: No internet connectivity required or used
-   **Code verification**: HMAC-based verification prevents timing attacks

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

Before submitting a PR:

1. Ensure all tests pass: `cargo test --workspace`
2. Format code: `cargo fmt --all`
3. Check lints: `cargo clippy --workspace -- -D warnings`
4. Update documentation if needed

## License

Licensed under either of:

-   **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
-   **MIT license** ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## Acknowledgments

Built with Rust and powered by:

-   [tokio](https://tokio.rs/) - Async runtime
-   [rustls](https://github.com/rustls/rustls) - TLS implementation
-   [mdns-sd](https://github.com/keepsimple1/mdns-sd) - mDNS/DNS-SD discovery
-   [arboard](https://github.com/1Password/arboard) - Cross-platform clipboard access
-   [clap](https://github.com/clap-rs/clap) - CLI parsing
-   [serde](https://serde.rs/) - Serialization framework
