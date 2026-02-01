# Yoop

**Cross-Platform Local Network File Sharing**

[![CI](https://github.com/sanchxt/yoop/workflows/CI/badge.svg)](https://github.com/sanchxt/yoop/actions)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)
[![Rust Version](https://img.shields.io/badge/rust-1.86.0%2B-blue.svg)](https://www.rust-lang.org)

Yoop enables seamless peer-to-peer file transfers over local networks using simple, time-limited codes. Unlike cloud-based solutions, all data stays on your local network, ensuring privacy, speed, and zero bandwidth costs.

## Features

### Core Features

- **Cross-platform**: Works on Windows, Linux, and macOS
- **No account required**: Zero configuration, no cloud dependency
- **Simple 4-character codes**: Easy discovery without IP addresses
- **QR code support**: Display scannable codes for upcoming mobile app (experimental)
- **Dual discovery**: UDP broadcast + mDNS/DNS-SD for reliable device discovery
- **Private & secure**: TLS 1.3 encryption, data never leaves local network
- **Fast transfers**: Chunked transfers with xxHash64 verification
- **Resume capability**: Interrupted transfers can be resumed automatically
- **CLI + Web interface**: Full-featured command-line tool and browser-based UI
- **Trusted devices**: Ed25519 signature-based authentication for direct transfers
- **Clipboard sharing**: One-shot transfer and live bidirectional sync
- **Directory sync**: Real-time bidirectional directory synchronization
- **Shell completions**: Bash, Zsh, Fish, PowerShell, Elvish support

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

### VPN & Overlay Network Support

Yoop works seamlessly over VPN overlay networks (Tailscale, WireGuard, ZeroTier, Headscale) where UDP broadcast and mDNS discovery don't work:

```bash
# Direct connection using IP address
yoop clipboard sync --host 100.103.164.32 A7K9

# Connect to trusted device (codeless, uses stored IP)
yoop clipboard sync --device "My-Mac"

# First-time pairing over VPN
# Device A:
yoop clipboard sync
# → Code: A7K9

# Device B:
yoop clipboard sync --host 100.103.164.32 A7K9
# After successful connection, Device B is trusted

# Subsequent connections (automatic):
yoop clipboard sync --device "Device-A"
```

**How it works:**

1. **First connection**: Use `--host IP[:PORT]` with a share code
2. **Trusted pairing**: After connection, devices are added to trust store with stored IP
3. **Future connections**: Use `--device <name>` for codeless connections
4. **Auto-fallback**: If discovery fails, automatically tries stored IP addresses

**Supported on all commands:**

- `yoop receive --host IP CODE`
- `yoop clipboard receive --device "Device-Name"`
- `yoop clipboard sync --host IP CODE`
- `yoop sync --device "Device-Name" ./folder`

### Directory Sync

```bash
# Host a sync session
yoop sync ~/Projects/shared-folder

# Join a sync session from another device
yoop sync A7K9 ~/Projects/shared-folder

# With exclusion patterns
yoop sync ./folder --exclude "*.log" --exclude "dist/"

# Use a .gitignore-style file
yoop sync ./folder --ignore-file .syncignore
```

Any file changes (additions, modifications, deletions) sync instantly between devices!

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
git clone https://github.com/sanchxt/yoop
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
3. **Discovery** happens via UDP broadcast + mDNS on local network (or direct IP with `--host`)
4. **Transfer** occurs directly over TLS 1.3 encrypted TCP connection
5. **Verification** using xxHash64 per chunk, SHA-256 for complete file
6. **Resume** automatic resumption of interrupted transfers from last checkpoint

**For trusted devices:** Direct connection using Ed25519 signatures (no code needed)

**Connection methods:**

- **Discovery**: UDP broadcast + mDNS for local networks
- **Direct IP**: `--host IP[:PORT]` for VPN/overlay networks
- **Trusted devices**: `--device <name>` for codeless connections with stored addresses
- **Auto-fallback**: Tries stored IP addresses when discovery fails

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

# Directory Sync
yoop sync <directory>              # Host a sync session
yoop sync <code> <directory>       # Join a sync session

# Device & Network Management
yoop trust list                    # Manage trusted devices
yoop scan                          # Scan for active shares
yoop diagnose                      # Network diagnostics

# Configuration & Utilities
yoop config                        # Manage configuration
yoop history                       # View transfer history
yoop tui                           # Launch interactive TUI dashboard
yoop web                           # Start web interface
yoop completions install           # Install shell completions
```

## TUI Mode

Launch a full-featured terminal dashboard for managing all Yoop features:

```bash
# Launch TUI dashboard
yoop tui

# Launch directly to a specific view
yoop tui --view share
yoop tui --view receive
yoop tui --view clipboard
yoop tui --view devices
```

**Features:**

- **Dashboard view**: All features accessible from a single interface
- **Vim-style navigation**: `j/k` to move, `h/l` to navigate, `Space` to select
- **Multiple views**: Share, Receive, Clipboard, Sync, Devices, History, Config
- **Responsive layout**: Adapts to terminal size (split panels, tabs, or minimal)
- **Real-time monitoring**: See active transfers and clipboard sync status
- **File browser**: Built-in file browser with multi-select support
- **Help overlay**: Press `?` for keybinding reference

**Key navigation:**

| Key | Action |
|-----|--------|
| `S/R/C/Y/D/H/G` | Switch views (Share/Receive/Clipboard/sYnc/Devices/History/confiG) |
| `j/k` or arrows | Navigate up/down |
| `Tab` | Cycle focus |
| `Enter` | Confirm/Start |
| `Space` | Toggle selection |
| `?` | Show help |
| `Q` | Quit |

The TUI provides the same functionality as the CLI commands but with an interactive interface - perfect for users who prefer visual navigation over memorizing commands.

## Web Interface

Start a browser-based UI for devices without CLI access:

```bash
yoop web                    # Start on default port 8080
yoop web --port 9000        # Custom port
yoop web --auth             # Require authentication
yoop web --localhost-only   # Bind to localhost only
```

**Features:**

- Drag-and-drop file sharing
- QR codes with deep links (for future mobile app integration)
- File previews (images, text, archives)
- Real-time transfer progress
- No installation required (just open in browser)

Access at `http://[your-ip]:8080` from any device on the network.

## Trusted Devices

Send files directly to trusted devices without share codes:

```bash
# First transfer: Use share code
yoop share file.txt
# After accepting, you'll be prompted to trust the device

# Subsequent transfers: Direct send (no code needed)
yoop send "Device-Name" file.txt

# Connect to trusted device for clipboard/sync (codeless)
yoop clipboard sync --device "Device-Name"
yoop clipboard receive --device "Device-Name"

# Direct IP connection (saves address for future use)
yoop clipboard sync --host 192.168.1.100 A7K9

# Manage trusted devices
yoop trust list                    # List all trusted devices
yoop trust set "Name" --level full # Set trust level
yoop trust remove "Name"           # Remove device
```

**Security:** Uses Ed25519 signatures for authentication. No MITM attacks possible.

**VPN Support:** Stored IP addresses enable seamless connections over Tailscale, WireGuard, and other overlay networks where discovery doesn't work.

## Directory Sync

Keep directories synchronized across devices in real-time:

```bash
# Host a sync session (generates code)
yoop sync ~/Projects/shared-folder
# → Shows code like A7K9, waits for peer

# Join from another device
yoop sync A7K9 ~/Projects/shared-folder

# Advanced options
yoop sync ./folder --exclude "*.log"         # Exclude patterns
yoop sync ./folder --exclude "node_modules/" # Exclude directories
yoop sync ./folder --ignore-file .syncignore # Use ignore file
yoop sync ./folder --no-delete               # Don't sync deletions
yoop sync ./folder --max-size 100MB          # Limit file size
```

**Features:**

- **Bidirectional**: Changes sync both ways automatically
- **Real-time**: File changes propagate within 1-2 seconds
- **Conflict resolution**: Last-write-wins with notifications
- **Pattern exclusions**: Gitignore-style pattern matching
- **All file types**: Files, directories, and optionally symlinks

**Output modes:**

```bash
yoop sync ./folder --quiet    # Minimal output
yoop sync ./folder --verbose  # Detailed logging
yoop sync ./folder --json     # JSON output for scripting
```

## Configuration

Yoop can be configured via TOML files:

- **Linux**: `~/.config/yoop/config.toml`
- **macOS**: `~/Library/Application Support/yoop/config.toml`
- **Windows**: `%APPDATA%\yoop\config.toml`

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

- **Rust**: 1.86.0 or later
- **Git**: For cloning the repository

### Building

```bash
# Clone repository
git clone https://github.com/sanchxt/yoop
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

- **Discovery**: UDP broadcast + mDNS/DNS-SD on port 52525
- **Transfer**: TCP on ports 52530-52540
- **Encryption**: TLS 1.3 with self-signed ephemeral certificates
- **Integrity**: xxHash64 per chunk, SHA-256 per file
- **Resume**: State persistence for interrupted transfer recovery
- **Code Format**: 4 characters from `[2-9A-HJ-KMN-Z]` (avoiding ambiguous chars)

## Security

Yoop prioritizes security and privacy:

- **Encryption**: All transfers use TLS 1.3 with perfect forward secrecy
- **No persistence**: Ephemeral certificates, no long-term keys (except trusted devices)
- **Rate limiting**: 3 failed attempts → 30 second lockout
- **Local only**: No internet connectivity required or used
- **Code verification**: HMAC-based verification prevents timing attacks

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

Before submitting a PR:

1. Ensure all tests pass: `cargo test --workspace`
2. Format code: `cargo fmt --all`
3. Check lints: `cargo clippy --workspace -- -D warnings`
4. Update documentation if needed

## License

Licensed under either of:

- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- **MIT license** ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## Acknowledgments

Built with Rust and powered by:

- [tokio](https://tokio.rs/) - Async runtime
- [rustls](https://github.com/rustls/rustls) - TLS implementation
- [mdns-sd](https://github.com/keepsimple1/mdns-sd) - mDNS/DNS-SD discovery
- [arboard](https://github.com/1Password/arboard) - Cross-platform clipboard access
- [clap](https://github.com/clap-rs/clap) - CLI parsing
- [serde](https://serde.rs/) - Serialization framework
