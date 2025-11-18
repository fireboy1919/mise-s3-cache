# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2025-11-18

### Added
- Initial release of mise-s3-cache
- Automatic S3 caching for mise tool installations
- Zero runtime dependencies with single Rust binary
- Comprehensive hook integration with mise (preinstall, postinstall, enter, leave)
- CLI commands: check, restore, store, analyze, warm, stats, status, cleanup, test
- Hook-mode support for silent operation during mise integration
- CI/CD optimization with --ci-mode flag
- Cross-platform support (Linux, macOS, Windows)
- SHA256 checksum verification for cached downloads
- Gzip compression for efficient storage
- Comprehensive error handling and graceful degradation
- Configuration via environment variables and TOML files
- Support for both .mise.toml and .tool-versions files
- Cache analytics and usage statistics
- Recovery tools for corrupted hook scenarios
- GitHub Actions CI/CD pipeline with automated builds and releases
- Comprehensive test coverage with integration tests
- LocalStack integration for S3 testing

### Performance
- Up to 90% reduction in tool installation time with warm cache
- Typical improvements: Node.js (12x faster), Terraform (10x faster), Python (12x faster)
- Background caching to minimize impact on initial installations
- Regional S3 optimization for fastest downloads

### Security
- Input validation and sanitization for tool names and versions
- Path traversal protection
- Secure handling of AWS credentials
- No secrets logged or exposed in error messages

### Documentation
- Comprehensive README with quick start guide
- Hook integration examples and best practices
- CI/CD integration guides for GitHub Actions, Bitbucket Pipelines, Docker
- Recovery and troubleshooting documentation
- Performance benchmarks and usage examples

[Unreleased]: https://github.com/fireboy1919/mise-s3-cache/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/fireboy1919/mise-s3-cache/releases/tag/v0.1.0