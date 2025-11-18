#!/bin/bash
# install-hooks.sh - Add S3 caching hooks to existing mise projects

set -e

MISE_S3_CACHE_BINARY="${MISE_S3_CACHE_BINARY:-mise-s3-cache}"

echo "🔧 Installing mise S3 cache hooks..."

# Check if mise-s3-cache is available
if ! command -v "$MISE_S3_CACHE_BINARY" &> /dev/null; then
    echo "❌ $MISE_S3_CACHE_BINARY not found in PATH"
    echo "   Please install mise-s3-cache first or set MISE_S3_CACHE_BINARY"
    exit 1
fi

# Check if we're in a directory with mise configuration
if [[ ! -f .mise.toml ]] && [[ ! -f .tool-versions ]]; then
    echo "❌ No .mise.toml or .tool-versions found"
    echo "   Run this script in a directory with mise configuration"
    exit 1
fi

# Create or update .mise.toml with hooks
if [[ -f .mise.toml ]]; then
    echo "📝 Adding hooks to existing .mise.toml"
    
    # Check if hooks section exists
    if grep -q "^\[hooks\]" .mise.toml; then
        echo "⚠️  [hooks] section already exists in .mise.toml"
        echo "   Please manually add the following hooks:"
        echo ""
        echo 'preinstall = "'"$MISE_S3_CACHE_BINARY"' restore --hook-mode --all"'
        echo 'postinstall = "'"$MISE_S3_CACHE_BINARY"' store --hook-mode --all"'
        echo 'enter = "'"$MISE_S3_CACHE_BINARY"' warm --hook-mode --background"'
    else
        echo "" >> .mise.toml
        echo "[hooks]" >> .mise.toml
        echo 'preinstall = "'"$MISE_S3_CACHE_BINARY"' restore --hook-mode --all"' >> .mise.toml
        echo 'postinstall = "'"$MISE_S3_CACHE_BINARY"' store --hook-mode --all"' >> .mise.toml
        echo 'enter = "'"$MISE_S3_CACHE_BINARY"' warm --hook-mode --background"' >> .mise.toml
        echo "✅ Hooks added to .mise.toml"
    fi
else
    echo "📝 Creating .mise.toml with hooks"
    cat > .mise.toml << EOF
# Add your tools here
# [tools]
# node = "20.0.0"
# python = "3.11.0"

[hooks]
preinstall = "$MISE_S3_CACHE_BINARY restore --hook-mode --all"
postinstall = "$MISE_S3_CACHE_BINARY store --hook-mode --all"
enter = "$MISE_S3_CACHE_BINARY warm --hook-mode --background"
EOF
    echo "✅ Created .mise.toml with S3 cache hooks"
fi

echo ""
echo "🎉 S3 cache hooks installed successfully!"
echo ""
echo "Next steps:"
echo "1. Configure S3 credentials (AWS CLI or environment variables)"
echo "2. Set MISE_S3_BUCKET environment variable"
echo "3. Test with: mise install"
echo ""
echo "Hook behavior:"
echo "• preinstall: Checks S3 cache before installing tools"
echo "• postinstall: Stores tools in S3 cache after installation" 
echo "• enter: Warms cache in background when entering directory"
echo ""
echo "For more configuration options, see examples/mise-hooks-integration.md"