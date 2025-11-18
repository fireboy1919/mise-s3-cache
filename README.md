# mise-s3-cache

[![Build Status](https://github.com/fireboy1919/mise-s3-cache/workflows/CI/badge.svg)](https://github.com/fireboy1919/mise-s3-cache/actions)
[![Release](https://img.shields.io/github/v/release/fireboy1919/mise-s3-cache)](https://github.com/fireboy1919/mise-s3-cache/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Intelligent S3 caching for [mise](https://mise.jdx.dev/) tool installations with automatic hook integration**

Speed up your development workflows and CI/CD pipelines by automatically caching mise tool installations in S3. Share cached tools across your team and environments for dramatically faster builds. Now with seamless hook integration for zero-configuration setup!

## Features

- 🚀 **Automatic Hook Integration** - Zero-configuration setup using mise hooks
- 🦀 **Zero Runtime Dependencies** - Single statically-linked Rust binary  
- 🏗️ **CI/CD Optimized** - Intelligent behavior switching for development vs CI environments
- ⚡ **Up to 90% Faster Builds** - Dramatic performance improvements with warm cache
- 🌐 **Team Sharing** - Share cached tools across your entire team
- 🛡️ **Fault Tolerant** - Graceful degradation when cache is unavailable
- 🔒 **Secure** - Checksum verification for all cached downloads
- 📊 **Analytics** - Track cache hit/miss rates and usage patterns
- 🗜️ **Compression** - Efficient gzip compression for cached tools
- 🌍 **Cross-Platform** - Linux, macOS, and Windows support

## 📊 Performance Impact

| Scenario | Before | After | Time Saved |
|----------|--------|-------|------------|
| Fresh installation | 5 minutes | 5 minutes | 0% |
| **Cached installation** | 5 minutes | **30 seconds** | **90%** |
| **Team onboarding** | 5 min/developer | **30 sec/developer** | **90%** |
| **CI builds (cached)** | 4-5 minutes | **30-60 seconds** | **80-87%** |

## 🚀 Quick Start

### 1. Install mise-s3-cache

```bash
# Download latest release
curl -L https://github.com/fireboy1919/mise-s3-cache/releases/latest/download/mise-s3-cache-linux-x86_64.tar.gz | tar xz
sudo mv mise-s3-cache /usr/local/bin/

# Or install from source
git clone https://github.com/fireboy1919/mise-s3-cache.git
cd mise-s3-cache
cargo install --path .
```

### 2. Configure S3 credentials

```bash
# AWS credentials (same as AWS CLI)
export AWS_ACCESS_KEY_ID=your_access_key
export AWS_SECRET_ACCESS_KEY=your_secret_key
export AWS_DEFAULT_REGION=us-east-1

# S3 cache configuration  
export MISE_S3_CACHE_BUCKET=your-mise-cache-bucket
export MISE_S3_CACHE_ENABLED=true
```

### 3. Install hooks in your project

```bash
cd your-project
curl -O https://raw.githubusercontent.com/fireboy1919/mise-s3-cache/main/examples/install-hooks.sh
chmod +x install-hooks.sh
./install-hooks.sh
```

### 4. Use mise normally - caching is automatic!

```bash
mise install  # Tools are now automatically cached and restored
```

## Installation

### Using Cargo

```bash
cargo install mise-s3-cache
```

### Download Binary

Download the latest release for your platform from [GitHub Releases](https://github.com/fireboy1919/mise-s3-cache/releases).

### Using Homebrew (macOS/Linux)

```bash
brew install fireboy1919/tap/mise-s3-cache
```

### Configuration

```bash
# Configure S3 bucket (required)
export MISE_S3_CACHE_BUCKET=your-cache-bucket
export MISE_S3_CACHE_REGION=us-east-1  # optional, defaults to us-east-1

# Test connectivity
s3-cache test
```

## Configuration

### Environment Variables

- `MISE_S3_CACHE_BUCKET` - S3 bucket name (required)
- `MISE_S3_CACHE_REGION` - AWS region (default: us-east-1)
- `MISE_S3_CACHE_PREFIX` - S3 key prefix (default: mise-cache)
- `MISE_S3_CACHE_TTL` - Cache TTL in seconds (default: 604800 = 7 days)

### Project Configuration

```toml
# .mise.toml
[tools]
"s3-cache" = "latest"
node = "18.17.0"
terraform = "1.5.0"

[env]
MISE_S3_CACHE_BUCKET = "my-project-cache"
```

## Usage

Once installed and configured, mise will automatically:

1. Check S3 cache before downloading tools
2. Download from S3 if available (fast, regional)
3. Fall back to upstream downloads if not cached
4. Store successful downloads in S3 cache (background)

### Commands

```bash
# Check if a tool version exists in cache
s3-cache check node 18.17.0

# Restore a tool from cache
s3-cache restore node 18.17.0 --path ~/.mise/installs/node/18.17.0

# Store a tool installation in cache
s3-cache store node 18.17.0 --path ~/.mise/installs/node/18.17.0

# Analyze current project's cache status
s3-cache analyze

# Warm cache for current project
s3-cache warm

# Show cache statistics
s3-cache stats

# Show configuration status
s3-cache status

# Clean old cache entries
s3-cache cleanup --days 7

# Test S3 connectivity
s3-cache test
```

### Integration with mise

To automatically use S3 cache with mise, you can create hooks or wrapper scripts:

```bash
#!/bin/bash
# ~/.local/bin/mise-with-cache

TOOL="$1"
VERSION="$2"
INSTALL_PATH="$3"

# Check cache first
if s3-cache check "$TOOL" "$VERSION"; then
    echo "📦 Restoring $TOOL@$VERSION from S3 cache"
    if s3-cache restore "$TOOL" "$VERSION" --path "$INSTALL_PATH"; then
        exit 0
    fi
fi

# Fallback to normal mise install
echo "⬇️  Installing $TOOL@$VERSION from upstream"
mise install "$TOOL@$VERSION"

# Cache the installation
s3-cache store "$TOOL" "$VERSION" --path "$INSTALL_PATH"
```

## How It Works

1. **Project Analysis**: Parses `.mise.toml` and `.tool-versions` to identify required tools
2. **Cache Check**: Before any tool installation, checks if it exists in S3 cache
3. **Fast Download**: If cached, downloads from S3 (same region = fast)
4. **Fallback**: If not cached, uses normal mise installation
5. **Background Caching**: Successful installs are cached to S3 in background
6. **Selective**: Only tools in your project config are cached, preventing cache bloat

## CI/CD Integration

### Bitbucket Pipelines

```yaml
- step:
    name: Build with S3 Cache
    script:
      - export MISE_S3_CACHE_BUCKET=your-cache-bucket
      - curl -sSL https://github.com/fireboy1919/mise-s3-cache/releases/latest/download/s3-cache-linux-x64.tar.gz | tar xz
      - chmod +x s3-cache
      - ./s3-cache warm  # Pre-cache project tools
      - mise install    # Will be much faster with pre-warmed cache
      - npm run build
```

### GitHub Actions

```yaml
- name: Setup S3 cache for mise
  run: |
    # Download and install s3-cache binary
    curl -sSL https://github.com/fireboy1919/mise-s3-cache/releases/latest/download/s3-cache-linux-x64.tar.gz | tar xz
    chmod +x s3-cache
    export PATH="$PWD:$PATH"
    
    # Configure and warm cache
    export MISE_S3_CACHE_BUCKET=your-cache-bucket
    s3-cache warm
    
    # Install mise and tools (will use cache)
    curl https://mise.run | sh
    mise install
```

### Docker

```dockerfile
FROM ubuntu:22.04

# Install s3-cache binary
RUN curl -sSL https://github.com/fireboy1919/mise-s3-cache/releases/latest/download/s3-cache-linux-x64.tar.gz | tar xz -C /usr/local/bin

# Configure S3 cache
ENV MISE_S3_CACHE_BUCKET=your-cache-bucket

# Install mise
RUN curl https://mise.run | sh

# Warm cache and install tools
COPY .mise.toml .
RUN s3-cache warm && mise install
```

## Requirements

- [mise](https://mise.jdx.dev/) 2024.1.0+ for tool management
- AWS credentials configured (via AWS CLI, environment variables, or IAM roles)
- S3 bucket with read/write permissions

## Performance

Typical performance improvements with S3 cache:

- **Node.js 18.17.0**: ~2 minutes → ~10 seconds (12x faster)
- **Terraform 1.5.0**: ~30 seconds → ~3 seconds (10x faster)
- **Python 3.11.0**: ~3 minutes → ~15 seconds (12x faster)

Results vary based on network speed, S3 region, and tool size.

## Security

- 🔐 **Checksum Verification**: All downloads verified with SHA256
- 🛡️ **Input Validation**: Tool names and versions sanitized
- 🚫 **Path Traversal Protection**: Prevents directory traversal attacks
- 📝 **No Secrets in Logs**: Careful handling of sensitive information

## Contributing

Contributions welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

- 🐛 **Bug Reports**: [Create an issue](https://github.com/fireboy1919/mise-s3-cache/issues)
- 💡 **Feature Requests**: [Start a discussion](https://github.com/fireboy1919/mise-s3-cache/discussions)
- 🔧 **Pull Requests**: See contributing guide

## License

MIT License - see [LICENSE](LICENSE) file for details.