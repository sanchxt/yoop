#!/usr/bin/env bash
#
# Publish npm packages to the npm registry
#
# This script publishes all platform packages first, then the main package.
# It's intended for local publishing; CI uses the workflow directly.
#
# Prerequisites:
#   - npm login (or NPM_TOKEN environment variable)
#   - All binaries built (run ./scripts/build-npm.sh first)
#
# Usage:
#   ./scripts/publish-npm.sh              # Publish all packages
#   ./scripts/publish-npm.sh --dry-run    # Test without publishing
#   ./scripts/publish-npm.sh --check      # Check if binaries exist

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
NPM_DIR="$ROOT_DIR/npm"

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

# Platform packages in publish order
PLATFORM_PACKAGES=(
    "@sanchxt/yoop-linux-x64-gnu"
    "@sanchxt/yoop-linux-x64-musl"
    "@sanchxt/yoop-linux-arm64-gnu"
    "@sanchxt/yoop-darwin-x64"
    "@sanchxt/yoop-darwin-arm64"
    "@sanchxt/yoop-win32-x64-msvc"
)

# Check if binaries exist
check_binaries() {
    local missing=0

    for pkg in "${PLATFORM_PACKAGES[@]}"; do
        local pkg_dir="$NPM_DIR/$pkg"
        local bin_dir="$pkg_dir/bin"

        if [[ ! -d "$bin_dir" ]]; then
            log_error "Missing bin directory: $bin_dir"
            missing=$((missing + 1))
            continue
        fi

        local bin_count
        bin_count=$(find "$bin_dir" -type f 2>/dev/null | wc -l)
        if [[ "$bin_count" -eq 0 ]]; then
            log_error "No binary in: $bin_dir"
            missing=$((missing + 1))
        else
            log_success "Found binary in: $pkg"
        fi
    done

    return $missing
}

# Publish a single package
publish_package() {
    local pkg_path="$1"
    local dry_run="$2"

    local pkg_name
    pkg_name=$(basename "$pkg_path")

    if [[ "$dry_run" == "true" ]]; then
        log_info "[DRY RUN] Would publish: $pkg_name"
        (cd "$pkg_path" && npm publish --access public --dry-run 2>&1 | head -5)
    else
        log_info "Publishing: $pkg_name"
        (cd "$pkg_path" && npm publish --access public)
    fi
}

# Main
main() {
    local dry_run=false
    local check_only=false

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --dry-run)
                dry_run=true
                shift
                ;;
            --check)
                check_only=true
                shift
                ;;
            -h|--help)
                echo "Usage: $0 [OPTIONS]"
                echo ""
                echo "Options:"
                echo "  --dry-run    Test publish without actually publishing"
                echo "  --check      Only check if binaries exist"
                echo "  -h, --help   Show this help message"
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                exit 1
                ;;
        esac
    done

    log_info "Yoop npm package publisher"
    echo ""

    # Sync versions first
    log_info "Syncing package versions..."
    node "$SCRIPT_DIR/sync-versions.js"
    echo ""

    # Check binaries
    log_info "Checking for binaries..."
    if ! check_binaries; then
        echo ""
        log_error "Some binaries are missing. Run './scripts/build-npm.sh' first."
        exit 1
    fi
    echo ""

    if [[ "$check_only" == "true" ]]; then
        log_success "All binaries present!"
        exit 0
    fi

    # Check npm authentication
    if [[ -z "${NPM_TOKEN:-}" ]]; then
        if ! npm whoami &>/dev/null; then
            log_error "Not logged in to npm. Run 'npm login' or set NPM_TOKEN."
            exit 1
        fi
        log_info "Logged in as: $(npm whoami)"
    else
        log_info "Using NPM_TOKEN for authentication"
    fi
    echo ""

    # Publish platform packages
    log_info "Publishing platform packages..."
    local failed=()
    for pkg in "${PLATFORM_PACKAGES[@]}"; do
        local pkg_path="$NPM_DIR/$pkg"
        if ! publish_package "$pkg_path" "$dry_run"; then
            failed+=("$pkg")
        fi
    done
    echo ""

    if [[ ${#failed[@]} -gt 0 ]]; then
        log_error "Failed to publish: ${failed[*]}"
        exit 1
    fi

    # Wait for registry propagation
    if [[ "$dry_run" != "true" ]]; then
        log_info "Waiting for registry propagation (15s)..."
        sleep 15
    fi

    # Publish main package
    log_info "Publishing main package..."
    if ! publish_package "$NPM_DIR/yoop" "$dry_run"; then
        log_error "Failed to publish main package"
        exit 1
    fi
    echo ""

    if [[ "$dry_run" == "true" ]]; then
        log_success "Dry run complete!"
    else
        log_success "All packages published successfully!"
        echo ""
        echo "Users can now install with:"
        echo "  npm install -g yoop"
        echo "  pnpm add -g yoop"
        echo "  bun add -g yoop"
    fi
}

main "$@"
