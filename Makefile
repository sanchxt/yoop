.PHONY: all build release test lint fmt check clean doc install help

# Default target
all: check

# Build debug
build:
	cargo build --workspace

# Build release
release:
	cargo build --workspace --release

# Run all tests
test:
	cargo test --workspace

# Run tests with output
test-verbose:
	cargo test --workspace -- --nocapture

# Run clippy linter
lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

# Format code
fmt:
	cargo fmt --all

# Check formatting
fmt-check:
	cargo fmt --all -- --check

# Run all checks (fmt, lint, test)
check: fmt-check lint test

# Clean build artifacts
clean:
	cargo clean

# Generate documentation
doc:
	cargo doc --workspace --all-features --no-deps --open

# Install the CLI locally
install:
	cargo install --path crates/yoop-cli

# Run the CLI in development
run:
	cargo run --package yoop -- $(ARGS)

# Run with release optimizations
run-release:
	cargo run --release --package yoop -- $(ARGS)

# Setup development environment
setup:
	@echo "Installing pre-commit hooks..."
	pip install pre-commit
	pre-commit install
	@echo "Done! Run 'make check' to verify everything works."

# Watch for changes and run tests
watch:
	cargo watch -x test

# Show help
help:
	@echo "Yoop Development Commands"
	@echo ""
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@echo "  all          Run all checks (default)"
	@echo "  build        Build debug version"
	@echo "  release      Build release version"
	@echo "  test         Run all tests"
	@echo "  test-verbose Run tests with output"
	@echo "  lint         Run clippy linter"
	@echo "  fmt          Format code"
	@echo "  fmt-check    Check formatting"
	@echo "  check        Run all checks (fmt, lint, test)"
	@echo "  clean        Clean build artifacts"
	@echo "  doc          Generate and open documentation"
	@echo "  install      Install CLI locally"
	@echo "  run          Run CLI (use ARGS='...' for arguments)"
	@echo "  run-release  Run CLI in release mode"
	@echo "  setup        Setup development environment"
	@echo "  watch        Watch for changes and run tests"
	@echo "  help         Show this help"
