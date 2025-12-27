# Yoop

**Cross-Platform Local Network File Sharing**

Yoop enables seamless peer-to-peer file transfers over local networks using simple, time-limited codes. Unlike cloud-based solutions, all data stays on your local network, ensuring privacy, speed, and zero bandwidth costs.

## Installation

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

### Clipboard Sharing

```bash
# Share current clipboard content
yoop clipboard share

# Receive clipboard content
yoop clipboard receive A7K9

# Bidirectional clipboard sync
yoop clipboard sync
```

## Features

- **Cross-platform**: Windows, Linux, and macOS
- **No account required**: Zero configuration, no cloud dependency
- **Simple 4-character codes**: Easy discovery without IP addresses
- **Private & secure**: TLS 1.3 encryption, data never leaves local network
- **Fast transfers**: Chunked transfers with verification
- **Resume capability**: Interrupted transfers resume automatically
- **Web interface**: Browser-based UI for devices without CLI

## CLI Commands

```bash
yoop share <files...>           # Share files/folders
yoop receive <code>             # Receive with code
yoop clipboard share            # Share clipboard
yoop clipboard receive <code>   # Receive clipboard
yoop clipboard sync [code]      # Bidirectional sync
yoop scan                       # Scan for active shares
yoop web                        # Start web interface
yoop diagnose                   # Network diagnostics
yoop history                    # View transfer history
```

## How It Works

1. **Sender** shares files and gets a 4-character code (e.g., `A7K9`)
2. **Receiver** enters the code on their device
3. **Discovery** via UDP broadcast + mDNS on local network
4. **Transfer** over TLS 1.3 encrypted connection
5. **Resume** automatic resumption of interrupted transfers

## Supported Platforms

| Platform | Architecture |
|----------|--------------|
| Linux | x64, ARM64 |
| macOS | x64 (Intel), ARM64 (Apple Silicon) |
| Windows | x64 |

## Links

- [GitHub Repository](https://github.com/sanchxt/yoop)
- [Documentation](https://github.com/sanchxt/yoop#readme)
- [Issue Tracker](https://github.com/sanchxt/yoop/issues)

## License

MIT OR Apache-2.0
