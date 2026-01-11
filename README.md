# Yoop

**Cross-Platform Local Network File Sharing**

[![CI](https://github.com/arceus/yoop/workflows/CI/badge.svg)](https://github.com/arceus/yoop/actions)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)
[![Rust Version](https://img.shields.io/badge/rust-1.86.0%2B-blue.svg)](https://www.rust-lang.org)

Yoop enables seamless peer-to-peer file transfers over local networks using simple, time-limited codes. Unlike cloud-based solutions, all data stays on your local network, ensuring privacy, speed, and zero bandwidth costs.

## Features

### Core Features

-   **Cross-platform**: Works on Windows, Linux, and macOS
-   **No account required**: Zero configuration, no cloud dependency
-   **Simple 4-character codes**: Easy discovery without IP addresses
-   **QR code support**: Display scannable codes for upcoming mobile app (experimental)
-   **Dual discovery**: UDP broadcast + mDNS/DNS-SD for reliable device discovery
-   **Private & secure**: TLS 1.3 encryption, data never leaves local network
-   **Fast transfers**: Chunked transfers with xxHash64 verification
-   **Resume capability**: Interrupted transfers can be resumed automatically
-   **CLI + Web interface**: Full-featured command-line tool and browser-based UI
-   **Trusted devices**: Ed25519 signature-based authentication for direct transfers
-   **Clipboard sharing**: One-shot transfer and live bidirectional sync
-   **Shell completions**: Bash, Zsh, Fish, PowerShell, Elvish support

## Quick Start

### Share Files

```bash
# Share a single file
yoop share document.pdf

# Share multiple files and folders
yoop share photos/ videos/ notes.md

# Share with custom expiration
yoop share project.zip --expire 10m
```

### Receive Files

```bash
# Receive using the 4-character code
yoop receive A7K9

# Receive to specific directory
yoop receive A7K9 --output ~/Downloads/

# Batch mode (auto-accept)
yoop receive A7K9 --batch
```

### Clipboard Sharing (Unique Feature!)

```bash
# One-shot clipboard sharing
yoop clipboard share               # Share current clipboard
yoop clipboard receive A7K9        # Receive clipboard content

# Live bidirectional sync (sync clipboard changes in real-time)
yoop clipboard sync                # Host sync session
yoop clipboard sync A7K9           # Join sync session
```

Supports text and images. Changes sync automatically across devices!

## Installation

### via npm (Recommended)

```bash
# npm
npm install -g yoop

# pnpm
pnpm add -g yoop

# yarn
yarn global add yoop

# bun
bun add -g yoop
```

### From Source

Requires **Rust 1.86.0** or later.

```bash
git clone https://github.com/arceus/yoop
cd yoop
cargo install --path crates/yoop-cli
```

### Pre-built Binaries

Pre-built binaries for Windows, Linux, and macOS are coming soon.

## Shell Completions

Install tab completions for your shell:

```bash
yoop completions install           # Auto-detect shell and install
yoop completions install --shell zsh
yoop completions generate bash     # Print to stdout
```

Supported: Bash, Zsh, Fish, PowerShell, Elvish

## How It Works

**Code-based transfers:**

1. **Sender** shares files and gets a 4-character code (e.g., `A 7 K 9`)
2. **Receiver** enters the code on their device
3. **Discovery** happens via UDP broadcast + mDNS on local network
4. **Transfer** occurs directly over TLS 1.3 encrypted TCP connection
5. **Verification** using xxHash64 per chunk, SHA-256 for complete file
6. **Resume** automatic resumption of interrupted transfers from last checkpoint

**For trusted devices:** Direct connection using Ed25519 signatures (no code needed)

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
yoop share <files...>              # Share files/folders
yoop receive <code>                # Receive with code
yoop send <device> <files...>      # Send to trusted device (no code)

# Clipboard Sharing
yoop clipboard share               # Share clipboard content
yoop clipboard receive <code>      # Receive clipboard content
yoop clipboard sync [code]         # Bidirectional clipboard sync

# Device & Network Management
yoop trust list                    # Manage trusted devices
yoop scan                          # Scan for active shares
yoop diagnose                      # Network diagnostics

# Configuration & Utilities
yoop config                        # Manage configuration
yoop history                       # View transfer history
yoop web                           # Start web interface
yoop completions install           # Install shell completions
```

## Web Interface

Start a browser-based UI for devices without CLI access:

```bash
yoop web                    # Start on default port 8080
yoop web --port 9000        # Custom port
yoop web --auth             # Require authentication
yoop web --localhost-only   # Bind to localhost only
```

**Features:**

-   Drag-and-drop file sharing
-   QR codes with deep links (for future mobile app integration)
-   File previews (images, text, archives)
-   Real-time transfer progress
-   No installation required (just open in browser)

Access at `http://[your-ip]:8080` from any device on the network.

## Trusted Devices

Send files directly to trusted devices without share codes:

```bash
# First transfer: Use share code
yoop share file.txt
# After accepting, you'll be prompted to trust the device

# Subsequent transfers: Direct send (no code needed)
yoop send "Device-Name" file.txt

# Manage trusted devices
yoop trust list                    # List all trusted devices
yoop trust set "Name" --level full # Set trust level
yoop trust remove "Name"           # Remove device
```

**Security:** Uses Ed25519 signatures for authentication. No MITM attacks possible.

## Configuration

Yoop can be configured via TOML files:

-   **Linux**: `~/.config/yoop/config.toml`
-   **macOS**: `~/Library/Application Support/yoop/config.toml`
-   **Windows**: `%APPDATA%\yoop\config.toml`

Example configuration:

```toml
[general]
device_name = "My-Laptop"
default_expire = "5m"
default_output = "~/Downloads"

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

[trust]
enabled = true
auto_prompt = true
default_level = "ask_each_time"

[history]
enabled = true
max_entries = 100

[ui]
show_qr = false  # Enable QR codes (for future mobile app)

[web]
port = 8080
auth = false
```

See all options: `yoop config list`

## Development

### Prerequisites

-   **Rust**: 1.86.0 or later
-   **Git**: For cloning the repository

### Building

```bash
# Clone repository
git clone https://github.com/arceus/yoop
cd yoop

# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Run with logging
RUST_LOG=debug cargo run --bin yoop -- share test.txt
```

### Running Tests

```bash
# All tests
cargo test --workspace

# Unit tests only
cargo test --lib --workspace

# Integration tests only
cargo test --test integration_transfer
cargo test --test integration_trust
cargo test --test integration_clipboard

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

# Generate documentation
cargo doc --workspace --open
```

## Architecture

Yoop uses a custom binary protocol (LDRP) over TLS 1.3:

-   **Discovery**: UDP broadcast + mDNS/DNS-SD on port 52525
-   **Transfer**: TCP on ports 52530-52540
-   **Encryption**: TLS 1.3 with self-signed ephemeral certificates
-   **Integrity**: xxHash64 per chunk, SHA-256 per file
-   **Resume**: State persistence for interrupted transfer recovery
-   **Code Format**: 4 characters from `[2-9A-HJ-KMN-Z]` (avoiding ambiguous chars)

## Security

Yoop prioritizes security and privacy:

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
