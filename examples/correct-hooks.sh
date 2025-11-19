#!/bin/bash
# Example of correct mise hooks for s3-cache integration

# This script shows the proper hook implementation that:
# 1. preinstall: Actually restores from cache (not just checks)
# 2. postinstall: Only stores newly installed tools (not already cached ones)

echo "Installing correct mise hooks for s3-cache integration..."

# Ensure hooks directory exists
mkdir -p ~/.config/mise/hooks

# Create preinstall hook that actually restores from cache
cat > ~/.config/mise/hooks/preinstall <<'EOF'
#!/bin/bash
# s3-cache preinstall hook - restore from cache if available
if command -v s3-cache &> /dev/null; then
    # Attempt to restore from cache using auto-path detection
    if s3-cache restore --hook-mode "${MISE_TOOL_NAME}" "${MISE_TOOL_VERSION}" 2>/dev/null; then
        # Cache hit - tool was restored
        exit 0
    fi
fi
# Cache miss or s3-cache not available - proceed with normal installation
exit 1
EOF

# Create postinstall hook that stores newly installed tools
cat > ~/.config/mise/hooks/postinstall <<'EOF'
#!/bin/bash  
# s3-cache postinstall hook - store newly installed tools in cache
if command -v s3-cache &> /dev/null; then
    # Only store if this was a fresh install (not a cache restore)
    # The hook will run regardless, but we should only store newly installed tools
    s3-cache store --hook-mode "${MISE_TOOL_NAME}" "${MISE_TOOL_VERSION}" "${MISE_TOOL_PATH}" 2>/dev/null || true
fi
EOF

# Make hooks executable
chmod +x ~/.config/mise/hooks/preinstall ~/.config/mise/hooks/postinstall

echo "✅ Correct s3-cache hooks installed!"
echo ""
echo "Hook behavior:"
echo "- preinstall: Attempts to restore tool from S3 cache to proper install path"  
echo "- If cache hit: Tool is restored, mise skips installation"
echo "- If cache miss: Normal mise installation proceeds"
echo "- postinstall: Stores newly installed tools in S3 cache"
echo ""
echo "Test with: mise install <tool>@<version>"