#!/bin/bash
# install-ci-hooks.sh - Install CI/CD-optimized hooks with environment detection

set -e

MISE_S3_CACHE_BINARY="${MISE_S3_CACHE_BINARY:-mise-s3-cache}"

echo "🏗️ Installing CI/CD-optimized S3 cache hooks..."

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

# Create or update .mise.toml with CI-aware hooks
if [[ -f .mise.toml ]]; then
    echo "📝 Adding CI-optimized hooks to existing .mise.toml"
    
    # Check if hooks section exists
    if grep -q "^\[hooks\]" .mise.toml; then
        echo "⚠️  [hooks] section already exists in .mise.toml"
        echo "   Please manually replace with CI-optimized hooks"
    else
        # Add CI-optimized hooks
        cat >> .mise.toml << EOF

[hooks]
# CI-optimized hooks with environment detection
preinstall = '''
if [ -n "\$CI" ] || [ -n "\$GITHUB_ACTIONS" ] || [ -n "\$GITLAB_CI" ] || [ -n "\$JENKINS_URL" ]; then
  echo "🏗️ CI detected: Using optimized cache restore"
  "$MISE_S3_CACHE_BINARY" restore --ci-mode --all --parallel=8
else
  "$MISE_S3_CACHE_BINARY" restore --hook-mode --all
fi
'''

postinstall = '''  
if [ -n "\$CI" ] || [ -n "\$GITHUB_ACTIONS" ] || [ -n "\$GITLAB_CI" ] || [ -n "\$JENKINS_URL" ]; then
  echo "🏗️ CI detected: Using optimized cache storage"
  "$MISE_S3_CACHE_BINARY" store --all --parallel=8
else
  "$MISE_S3_CACHE_BINARY" store --hook-mode --all  
fi
'''

enter = '''
if [ -n "\$CI" ] || [ -n "\$GITHUB_ACTIONS" ] || [ -n "\$GITLAB_CI" ] || [ -n "\$JENKINS_URL" ]; then
  echo "🏗️ CI detected: Warming cache in foreground"
  "$MISE_S3_CACHE_BINARY" warm --ci-mode --parallel=8
else
  "$MISE_S3_CACHE_BINARY" warm --hook-mode --background
fi
'''
EOF
        echo "✅ CI-optimized hooks added to .mise.toml"
    fi
else
    echo "📝 Creating .mise.toml with CI-optimized hooks"
    cat > .mise.toml << EOF
# Add your tools here
# [tools]
# node = "20.0.0"  
# python = "3.11.0"

[hooks]
# CI-optimized hooks with environment detection
preinstall = '''
if [ -n "\$CI" ] || [ -n "\$GITHUB_ACTIONS" ] || [ -n "\$GITLAB_CI" ] || [ -n "\$JENKINS_URL" ]; then
  echo "🏗️ CI detected: Using optimized cache restore"
  "$MISE_S3_CACHE_BINARY" restore --ci-mode --all --parallel=8
else
  "$MISE_S3_CACHE_BINARY" restore --hook-mode --all
fi
'''

postinstall = '''
if [ -n "\$CI" ] || [ -n "\$GITHUB_ACTIONS" ] || [ -n "\$GITLAB_CI" ] || [ -n "\$JENKINS_URL" ]; then
  echo "🏗️ CI detected: Using optimized cache storage"  
  "$MISE_S3_CACHE_BINARY" store --all --parallel=8
else
  "$MISE_S3_CACHE_BINARY" store --hook-mode --all
fi
'''

enter = '''
if [ -n "\$CI" ] || [ -n "\$GITHUB_ACTIONS" ] || [ -n "\$GITLAB_CI" ] || [ -n "\$JENKINS_URL" ]; then
  echo "🏗️ CI detected: Warming cache in foreground"
  "$MISE_S3_CACHE_BINARY" warm --ci-mode --parallel=8  
else
  "$MISE_S3_CACHE_BINARY" warm --hook-mode --background
fi
'''
EOF
    echo "✅ Created .mise.toml with CI-optimized S3 cache hooks"
fi

echo ""
echo "🎉 CI/CD-optimized S3 cache hooks installed successfully!"
echo ""
echo "Behavior differences:"
echo "📱 Development (local):"
echo "   • Background cache warming (non-blocking)"
echo "   • Silent error handling (won't break workflows)"
echo "   • Conservative parallelism (--parallel=3)"
echo ""
echo "🏗️ CI/CD (detected):"
echo "   • Foreground cache operations (block until complete)"
echo "   • Fail-fast on cache errors (expose config issues)"  
echo "   • Aggressive parallelism (--parallel=8)"
echo "   • Verbose logging for debugging"
echo ""
echo "Environment variables for CI detection:"
echo "   CI, GITHUB_ACTIONS, GITLAB_CI, JENKINS_URL"
echo ""
echo "Manual override:"
echo "   export MISE_S3_CACHE_CI_MODE=true  # Force CI behavior"
echo "   export MISE_S3_CACHE_CI_MODE=false # Force dev behavior"
EOF