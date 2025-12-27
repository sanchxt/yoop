# Contributing to Yoop

Thank you for your interest in contributing to Yoop <3
This document provides guidelines and instructions for contributing.

## Code of Conduct

Please be respectful and constructive in all interactions. We're building something together.

## Getting Started

1. Fork the repository
2. Clone your fork:

    ```bash
    git clone https://github.com/your-username/yoop
    cd yoop
    ```

3. Create a branch for your changes:

    ```bash
    git checkout -b feature/my-feature
    ```

## Development Setup

### Prerequisites

-   **Rust 1.86.0+**: Install via [rustup](https://rustup.rs/)
-   **Git**: For version control

### Building

```bash
# Build all crates
cargo build --workspace

# Build in release mode
cargo build --workspace --release
```

### Testing

```bash
# Run all tests
cargo test --workspace

# Run tests with output
cargo test --workspace -- --nocapture

# Run specific test
cargo test --workspace test_name
```

### Linting

```bash
# Run clippy
cargo clippy --workspace -- -D warnings

# Format code
cargo fmt --all

# Check formatting
cargo fmt --all -- --check
```

## Code Style

### Rust Guidelines

-   Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
-   Use `rustfmt` for formatting (enforced by CI)
-   Address all `clippy` warnings
-   Write documentation for public APIs
-   Include examples in documentation where helpful

### Commit Messages

Use clear, descriptive commit messages:

```
type(scope): brief description

Longer description if needed, explaining:
- What changed
- Why it changed
- Any breaking changes

Fixes #123
```

Types:

-   `feat`: New feature
-   `fix`: Bug fix
-   `docs`: Documentation only
-   `style`: Formatting, no code change
-   `refactor`: Code change that neither fixes a bug nor adds a feature
-   `test`: Adding missing tests
-   `chore`: Build process or auxiliary tool changes

### Documentation

-   Document all public items with `///` doc comments
-   Include examples in `# Examples` sections
-   Document panics in `# Panics` sections
-   Document errors in `# Errors` sections

## Pull Request Process

1. **Update your branch**: Rebase on the latest `main`:

    ```bash
    git fetch origin
    git rebase origin/main
    ```

2. **Ensure CI passes**: Run locally before pushing:

    ```bash
    cargo fmt --all -- --check
    cargo clippy --workspace -- -D warnings
    cargo test --workspace
    ```

3. **Create the PR**:

    - Use a clear, descriptive title
    - Reference any related issues
    - Describe what the PR does
    - Include any testing notes

4. **Address feedback**: Make requested changes in new commits, then squash when approved

## Adding New Features

1. **Discuss first**: For large changes, open an issue to discuss the approach
2. **Start with tests**: Write tests for the new functionality (TDD)
3. **Implement**: Write the implementation
4. **Document**: Add documentation and update CHANGELOG.md
5. **Submit PR**: Follow the PR process above

## Reporting Bugs

When reporting bugs, please include:

1. **Description**: Clear description of the bug
2. **Steps to reproduce**: Minimal steps to reproduce
3. **Expected behavior**: What you expected to happen
4. **Actual behavior**: What actually happened
5. **Environment**: OS, Rust version, Yoop version
6. **Logs**: Any relevant error messages or logs

## Feature Requests

Feature requests are welcome! Please:

1. Check existing issues first
2. Describe the use case
3. Explain why it would be useful
4. Consider how it might be implemented

## Questions?

-   Open a GitHub issue for questions
-   Check existing issues and documentation first

## License

By contributing to Yoop, you agree that your contributions will be licensed under the MIT OR Apache-2.0 license.
