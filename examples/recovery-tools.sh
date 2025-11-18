#!/bin/bash
# recovery-tools.sh - Recovery and diagnostics for mise S3 cache issues

set -e

MISE_S3_CACHE_BINARY="${MISE_S3_CACHE_BINARY:-mise-s3-cache}"

echo "🔧 Mise S3 Cache Recovery Tools"
echo "================================"

# Function to disable hooks
disable_hooks() {
    echo "🛑 Disabling S3 cache hooks..."
    
    for config_file in .mise.toml .tool-versions; do
        if [[ -f "$config_file" ]]; then
            # Backup original
            cp "$config_file" "$config_file.backup.$(date +%s)"
            
            if [[ "$config_file" == ".mise.toml" ]]; then
                # Comment out hooks section
                sed -i '/^\[hooks\]/,/^\[/ { /^\[hooks\]/d; /^\[/!d; }' "$config_file"
                sed -i '/^preinstall\s*=\|^postinstall\s*=\|^enter\s*=/ s/^/# DISABLED: /' "$config_file"
                echo "✅ Hooks disabled in $config_file"
            fi
        fi
    done
    
    echo "🔄 Clear mise cache to ensure hooks are reloaded"
    mise cache clear 2>/dev/null || true
}

# Function to enable hooks
enable_hooks() {
    echo "🟢 Enabling S3 cache hooks..."
    
    if [[ ! -f .mise.toml ]]; then
        echo "❌ No .mise.toml found. Run the install-hooks.sh script first."
        return 1
    fi
    
    # Re-enable commented hooks
    sed -i 's/^# DISABLED: //' .mise.toml
    
    # Add hooks section if missing
    if ! grep -q "^\[hooks\]" .mise.toml; then
        echo "" >> .mise.toml
        echo "[hooks]" >> .mise.toml
        echo 'preinstall = "'"$MISE_S3_CACHE_BINARY"' restore --hook-mode --all"' >> .mise.toml
        echo 'postinstall = "'"$MISE_S3_CACHE_BINARY"' store --hook-mode --all"' >> .mise.toml
    fi
    
    mise cache clear 2>/dev/null || true
    echo "✅ Hooks enabled"
}

# Function to test configuration
test_config() {
    echo "🧪 Testing S3 cache configuration..."
    
    echo "Environment variables:"
    env | grep -E "(AWS_|MISE_S3_)" || echo "  No S3 cache environment variables set"
    
    echo ""
    echo "Testing connectivity:"
    if command -v "$MISE_S3_CACHE_BINARY" >/dev/null; then
        "$MISE_S3_CACHE_BINARY" test || echo "❌ Connectivity test failed (this may be expected if not configured)"
    else
        echo "❌ $MISE_S3_CACHE_BINARY not found"
    fi
}

# Function to clean up cache
cleanup_cache() {
    echo "🧹 Cleaning up cache files..."
    
    # Clean local cache
    rm -rf ~/.cache/mise-s3/ 2>/dev/null || true
    
    # Clean mise cache
    mise cache clear 2>/dev/null || true
    
    echo "✅ Local cache cleaned"
}

# Function to show diagnostics
show_diagnostics() {
    echo "🔍 S3 Cache Diagnostics"
    echo "======================="
    
    echo "Mise version:"
    mise --version 2>/dev/null || echo "  mise not found"
    
    echo ""
    echo "S3 cache binary:"
    if command -v "$MISE_S3_CACHE_BINARY" >/dev/null; then
        echo "  Found: $(which "$MISE_S3_CACHE_BINARY")"
        "$MISE_S3_CACHE_BINARY" --version 2>/dev/null || echo "  Version check failed"
    else
        echo "  ❌ Not found in PATH"
    fi
    
    echo ""
    echo "Configuration files:"
    for config in .mise.toml .tool-versions; do
        if [[ -f "$config" ]]; then
            echo "  ✅ $config exists"
            if grep -q "preinstall\|postinstall" "$config" 2>/dev/null; then
                echo "    Contains hooks: $(grep -c "preinstall\|postinstall" "$config" 2>/dev/null || echo 0)"
            fi
        else
            echo "  ❌ $config not found"
        fi
    done
    
    echo ""
    echo "Environment:"
    echo "  MISE_S3_CACHE_ENABLED: ${MISE_S3_CACHE_ENABLED:-not set}"
    echo "  AWS credentials: $(if [[ -n "$AWS_ACCESS_KEY_ID" ]]; then echo "set"; else echo "not set"; fi)"
    echo "  AWS endpoint: ${AWS_ENDPOINT_URL:-default}"
    
    echo ""
    echo "Recent mise installations:"
    ls -la ~/.local/share/mise/installs/ 2>/dev/null | head -10 || echo "  No installations found"
}

# Function to reset everything
reset_all() {
    echo "🔄 Resetting entire S3 cache setup..."
    read -p "This will disable hooks and clear all cache. Continue? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Cancelled"
        return 1
    fi
    
    disable_hooks
    cleanup_cache
    
    echo "✅ Reset complete. S3 caching is now disabled."
    echo "💡 Run ./examples/install-hooks.sh to re-enable"
}

# Main menu
case "${1:-help}" in
    "disable")
        disable_hooks
        ;;
    "enable") 
        enable_hooks
        ;;
    "test")
        test_config
        ;;
    "cleanup")
        cleanup_cache
        ;;
    "diagnostics"|"diag")
        show_diagnostics
        ;;
    "reset")
        reset_all
        ;;
    "help"|*)
        echo "Usage: $0 [command]"
        echo ""
        echo "Commands:"
        echo "  disable      Disable S3 cache hooks (emergency recovery)"
        echo "  enable       Re-enable S3 cache hooks"  
        echo "  test         Test S3 cache configuration"
        echo "  cleanup      Clean local cache files"
        echo "  diagnostics  Show detailed diagnostic information"
        echo "  reset        Reset everything (disable hooks + cleanup)"
        echo "  help         Show this help"
        echo ""
        echo "Environment variables:"
        echo "  MISE_S3_CACHE_BINARY   Path to s3-cache binary (default: mise-s3-cache)"
        ;;
esac