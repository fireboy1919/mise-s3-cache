# Contributing to mise-s3-cache

Thank you for your interest in contributing to mise-s3-cache! This document provides guidelines and information for contributing to this project.

## Table of Contents

- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Making Changes](#making-changes)
- [Testing](#testing)
- [Submitting Changes](#submitting-changes)
- [Code Style](#code-style)
- [Reporting Issues](#reporting-issues)

## Getting Started

1. Fork the repository on GitHub
2. Clone your fork locally
3. Set up the development environment
4. Make your changes
5. Test your changes
6. Submit a pull request

## Development Setup

### Prerequisites

- Rust 1.70+ (latest stable recommended)
- Docker (for integration testing with LocalStack)
- AWS CLI (for testing S3 connectivity)

### Setup

```bash
# Clone your fork
git clone https://github.com/YOUR_USERNAME/mise-s3-cache.git
cd mise-s3-cache

# Install dependencies and build
cargo build

# Run tests
cargo test

# Run integration tests (requires Docker)
cargo test test_integration -- --ignored
```

### Environment Variables for Testing

```bash
export AWS_ACCESS_KEY_ID=test
export AWS_SECRET_ACCESS_KEY=test
export AWS_ENDPOINT_URL=http://localhost:4566
export AWS_DEFAULT_REGION=us-east-1
export MISE_S3_CACHE_BUCKET=mise-cache-test
export MISE_S3_CACHE_REGION=us-east-1
export MISE_S3_CACHE_PREFIX=test-cache
export MISE_S3_CACHE_ENABLED=true
```

## Making Changes

### Code Organization

- `src/main.rs` - CLI interface and command handling
- `src/config.rs` - Configuration management
- `src/s3_operations.rs` - S3 client and operations
- `src/cache.rs` - Cache management and tool operations
- `src/tool_detection.rs` - mise configuration parsing
- `src/utils.rs` - Utility functions and validation
- `tests/` - Unit and integration tests
- `examples/` - Hook integration examples

### Commit Messages

Use conventional commit format:

- `feat:` for new features
- `fix:` for bug fixes
- `docs:` for documentation changes
- `test:` for test-related changes
- `refactor:` for code refactoring
- `chore:` for maintenance tasks

Example:
```
feat: add support for custom S3 endpoints

- Add endpoint_url configuration option
- Support for MinIO and LocalStack testing
- Update documentation with endpoint examples
```

## Testing

### Unit Tests

```bash
# Run all unit tests
cargo test

# Run specific test
cargo test test_config_parsing

# Run tests with output
cargo test -- --nocapture
```

### Integration Tests

Integration tests require Docker for LocalStack:

```bash
# Start LocalStack
docker run --rm -d -p 4566:4566 localstack/localstack

# Run integration tests
cargo test test_integration -- --ignored

# Clean up
docker stop $(docker ps -q --filter ancestor=localstack/localstack)
```

### Testing Hook Integration

```bash
# Build the binary
cargo build --release

# Test hook integration in a test project
cd test-project
../target/release/mise-s3-cache --hook-mode check node 18.17.0
```

## Submitting Changes

### Pull Request Process

1. **Create a branch** for your feature or fix:
   ```bash
   git checkout -b feature/awesome-new-feature
   ```

2. **Make your changes** and commit them:
   ```bash
   git add .
   git commit -m "feat: add awesome new feature"
   ```

3. **Push to your fork**:
   ```bash
   git push origin feature/awesome-new-feature
   ```

4. **Create a Pull Request** on GitHub with:
   - Clear title describing the change
   - Detailed description of what changed and why
   - Link to any related issues
   - Screenshots/examples if applicable

### Pull Request Guidelines

- Ensure all tests pass
- Add tests for new functionality
- Update documentation as needed
- Follow the existing code style
- Keep commits focused and atomic
- Rebase against main before submitting

## Code Style

### Rust Style

- Use `cargo fmt` to format code
- Use `cargo clippy` to catch common issues
- Follow Rust naming conventions
- Add documentation for public APIs

```bash
# Format code
cargo fmt

# Check for issues
cargo clippy

# Check documentation
cargo doc --no-deps
```

### Documentation

- Add doc comments for public functions
- Update README.md for user-facing changes
- Add examples for new CLI commands
- Update CHANGELOG.md with your changes

## Reporting Issues

### Bug Reports

When reporting bugs, please include:

- Operating system and version
- Rust version (`rustc --version`)
- mise version (`mise --version`)
- Full command that caused the issue
- Complete error output
- Steps to reproduce

### Feature Requests

For feature requests, please describe:

- The problem you're trying to solve
- Your proposed solution
- Any alternatives you've considered
- How it fits with the project's goals

## Security

If you discover a security vulnerability, please email rusty.phillips@gmail.com instead of using the issue tracker.

## Questions?

Feel free to open an issue for questions about contributing, or start a discussion in the GitHub Discussions tab.

Thank you for contributing to mise-s3-cache! 🚀