# Mise Hooks Integration Guide

This guide shows how to integrate `mise-s3-cache` with mise hooks for automatic caching without manual intervention.

## Automatic Tool Caching with Preinstall/Postinstall Hooks

Add these hooks to your `.mise.toml` to automatically cache tools during installation:

```toml
[hooks]
# Check S3 cache before installing tools
preinstall = "mise-s3-cache restore --hook-mode"

# Store tools in S3 cache after successful installation
postinstall = "mise-s3-cache store --hook-mode"
```

## Project-Level Cache Management

### Auto-warm cache when entering projects:
```toml
[hooks]
enter = "mise-s3-cache warm --background"
```

### Cleanup temporary files when leaving projects:
```toml
[hooks]
leave = "mise-s3-cache cleanup --temp-only"
```

## Advanced Hook Configuration

### Multiple hook commands:
```toml
[hooks]
enter = [
  "mise-s3-cache status --quiet",
  "mise-s3-cache warm --background"
]

preinstall = [
  "mise-s3-cache check --all",
  "mise-s3-cache restore --selective"
]
```

## Hook-Specific Command Options

When running in hook mode, `mise-s3-cache` should:

1. **Exit silently** on errors (don't break mise workflows)
2. **Skip interactive prompts** 
3. **Use faster heuristics** for cache decisions
4. **Log to mise-compatible locations**

### Example hook-mode behavior:
```bash
# Normal mode - interactive, verbose errors
mise-s3-cache restore

# Hook mode - silent on errors, non-interactive  
mise-s3-cache restore --hook-mode
```

## Environment Variables Available in Hooks

Hooks have access to these mise environment variables:
- `MISE_ORIGINAL_CWD`: User's original directory
- `MISE_PROJECT_ROOT`: Project root directory
- `MISE_PREVIOUS_DIR`: Previous directory (for cd hooks)

Our cache system can use these for:
- Determining project scope for caching
- Relative path resolution
- Project-specific cache policies

## Hook Installation Script

```bash
#!/bin/bash
# install-hooks.sh - Add S3 caching hooks to existing mise projects

if [[ ! -f .mise.toml ]]; then
    echo "No .mise.toml found. Creating basic configuration..."
    cat > .mise.toml << 'EOF'
[tools]
# Add your tools here

[hooks]
preinstall = "mise-s3-cache restore --hook-mode"
postinstall = "mise-s3-cache store --hook-mode" 
enter = "mise-s3-cache warm --background"
EOF
else
    echo "Adding S3 cache hooks to existing .mise.toml..."
    # Add hooks section if it doesn't exist
    if ! grep -q "^\[hooks\]" .mise.toml; then
        echo "" >> .mise.toml
        echo "[hooks]" >> .mise.toml
    fi
    
    # Add our hooks
    echo 'preinstall = "mise-s3-cache restore --hook-mode"' >> .mise.toml
    echo 'postinstall = "mise-s3-cache store --hook-mode"' >> .mise.toml
    echo 'enter = "mise-s3-cache warm --background"' >> .mise.toml
fi

echo "S3 cache hooks installed! Test with: mise install"
```

## Benefits of Hook Integration

1. **Zero-friction caching**: No manual cache commands needed
2. **Automatic cache population**: New tools automatically cached
3. **Faster team onboarding**: New devs get cached tools immediately
4. **CI/CD optimization**: Build systems benefit from automatic caching
5. **Transparent operation**: Works with existing mise workflows

## Testing Hook Integration

```bash
# Test preinstall hook
echo 'node = "20.0.0"' >> .mise.toml
mise install node  # Should check S3 cache first

# Test postinstall hook  
# After successful install, tool should be cached in S3

# Test enter hook
cd /some/other/dir && cd back/to/project  # Should warm cache

# Verify hooks are working
mise-s3-cache status --verbose
```