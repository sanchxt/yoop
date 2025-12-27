#!/usr/bin/env bash
#
# Build npm packages for all supported platforms
#
# This script cross-compiles the Yoop CLI for all target platforms
# and copies the binaries to the respective npm package directories.
#
# Prerequisites:
#   - Docker (for cross-compilation via `cross`)
#   - cargo install cross
#
# Usage:
#   ./scripts/build-npm.sh              # Build all targets
#   ./scripts/build-npm.sh --target linux-x64-gnu  # Build specific target
#   ./scripts/build-npm.sh --native     # Build only for current platform (no cross)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
NPM_DIR="$ROOT_DIR/npm"

# Target configuration: npm_suffix:rust_target
declare -A TARGETS=(
    ["linux-x64-gnu"]="x86_64-unknown-linux-gnu"
    ["linux-x64-musl"]="x86_64-unknown-linux-musl"
    ["linux-arm64-gnu"]="aarch64-unknown-linux-gnu"
    ["darwin-x64"]="x86_64-apple-darwin"
    ["darwin-arm64"]="aarch64-apple-darwin"
    ["win32-x64-msvc"]="x86_64-pc-windows-msvc"
)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Detect current platform
detect_native_target() {
    local os arch

    case "$(uname -s)" in
        Linux*)  os="linux" ;;
        Darwin*) os="darwin" ;;
        MINGW*|MSYS*|CYGWIN*) os="win32" ;;
        *)       os="unknown" ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64) arch="x64" ;;
        aarch64|arm64) arch="arm64" ;;
        *)            arch="unknown" ;;
    esac

    if [[ "$os" == "linux" ]]; then
        # Check if musl or glibc
        if ldd --version 2>&1 | grep -q musl; then
            echo "linux-${arch}-musl"
        else
            echo "linux-${arch}-gnu"
        fi
    elif [[ "$os" == "win32" ]]; then
        echo "win32-${arch}-msvc"
    else
        echo "${os}-${arch}"
    fi
}

# Build for a specific target
build_target() {
    local npm_suffix="$1"
    local rust_target="${TARGETS[$npm_suffix]}"
    local use_cross="$2"

    log_info "Building for $npm_suffix ($rust_target)..."

    local build_cmd
    if [[ "$use_cross" == "true" ]]; then
        build_cmd="cross"
    else
        build_cmd="cargo"
    fi

    # Build
    if ! $build_cmd build --release --target "$rust_target" -p yoop; then
        log_error "Failed to build for $rust_target"
        return 1
    fi

    # Determine binary name
    local bin_name="yoop"
    if [[ "$npm_suffix" == *"win32"* ]]; then
        bin_name="yoop.exe"
    fi

    # Source binary path
    local src_bin="$ROOT_DIR/target/$rust_target/release/$bin_name"
    if [[ ! -f "$src_bin" ]]; then
        log_error "Binary not found: $src_bin"
        return 1
    fi

    # Destination directory
    local dest_dir="$NPM_DIR/@sanchxt/yoop-$npm_suffix/bin"
    mkdir -p "$dest_dir"

    # Copy binary
    cp "$src_bin" "$dest_dir/$bin_name"

    # Make executable (except Windows)
    if [[ "$npm_suffix" != *"win32"* ]]; then
        chmod +x "$dest_dir/$bin_name"
    fi

    # Report size
    local size
    size=$(du -h "$dest_dir/$bin_name" | cut -f1)
    log_success "Built $npm_suffix: $dest_dir/$bin_name ($size)"
}

# Main
main() {
    local specific_target=""
    local native_only=false
    local use_cross=true

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --target)
                specific_target="$2"
                shift 2
                ;;
            --native)
                native_only=true
                use_cross=false
                shift
                ;;
            --no-cross)
                use_cross=false
                shift
                ;;
            -h|--help)
                echo "Usage: $0 [OPTIONS]"
                echo ""
                echo "Options:"
                echo "  --target <suffix>   Build only specific target (e.g., linux-x64-gnu)"
                echo "  --native            Build only for current platform (no cross-compilation)"
                echo "  --no-cross          Use cargo instead of cross for all targets"
                echo "  -h, --help          Show this help message"
                echo ""
                echo "Available targets:"
                for key in "${!TARGETS[@]}"; do
                    echo "  $key -> ${TARGETS[$key]}"
                done
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                exit 1
                ;;
        esac
    done

    log_info "Yoop npm package builder"
    echo ""

    # Sync versions first
    log_info "Syncing package versions..."
    node "$SCRIPT_DIR/sync-versions.js"
    echo ""

    # Determine which targets to build
    local targets_to_build=()

    if [[ -n "$specific_target" ]]; then
        if [[ -z "${TARGETS[$specific_target]:-}" ]]; then
            log_error "Unknown target: $specific_target"
            exit 1
        fi
        targets_to_build+=("$specific_target")
    elif [[ "$native_only" == "true" ]]; then
        local native
        native=$(detect_native_target)
        log_info "Detected native platform: $native"
        if [[ -z "${TARGETS[$native]:-}" ]]; then
            log_error "No target configuration for native platform: $native"
            exit 1
        fi
        targets_to_build+=("$native")
    else
        # Build all targets
        for key in "${!TARGETS[@]}"; do
            targets_to_build+=("$key")
        done
    fi

    # Check for cross if needed
    if [[ "$use_cross" == "true" ]]; then
        if ! command -v cross &> /dev/null; then
            log_warn "'cross' not found. Install with: cargo install cross"
            log_warn "Falling back to cargo (may fail for cross-compilation targets)"
            use_cross=false
        elif ! docker info &> /dev/null 2>&1; then
            log_warn "Docker not available. 'cross' requires Docker."
            log_warn "Falling back to cargo (may fail for cross-compilation targets)"
            use_cross=false
        fi
    fi

    # Build each target
    local failed=()
    for target in "${targets_to_build[@]}"; do
        if ! build_target "$target" "$use_cross"; then
            failed+=("$target")
        fi
    done

    echo ""

    # Summary
    local total=${#targets_to_build[@]}
    local failed_count=${#failed[@]}
    local success_count=$((total - failed_count))

    if [[ $failed_count -eq 0 ]]; then
        log_success "All $total target(s) built successfully!"
    else
        log_warn "Built $success_count/$total target(s). Failed: ${failed[*]}"
        exit 1
    fi
}

main "$@"
