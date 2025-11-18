#!/bin/bash
# Configuration management for S3 cache

# Default configuration
S3_CACHE_BUCKET="${MISE_S3_CACHE_BUCKET:-}"
S3_CACHE_REGION="${MISE_S3_CACHE_REGION:-us-east-1}"
S3_CACHE_PREFIX="${MISE_S3_CACHE_PREFIX:-mise-cache}"
S3_CACHE_TTL="${MISE_S3_CACHE_TTL:-604800}"  # 7 days
S3_CACHE_ENABLED="${MISE_S3_CACHE_ENABLED:-true}"

# Load user configuration if exists
USER_CONFIG="$HOME/.config/mise/s3-cache.conf"
if [[ -f "$USER_CONFIG" ]]; then
    source "$USER_CONFIG"
fi

# Load project-specific configuration  
PROJECT_CONFIG=".mise-s3-cache.conf"
if [[ -f "$PROJECT_CONFIG" ]]; then
    source "$PROJECT_CONFIG"
fi

validate_s3_config() {
    if [[ "$S3_CACHE_ENABLED" != "true" ]]; then
        return 1
    fi
    
    if [[ -z "$S3_CACHE_BUCKET" ]]; then
        echo "❌ S3_CACHE_BUCKET not configured"
        echo "Set MISE_S3_CACHE_BUCKET environment variable"
        return 1
    fi
    
    if ! command -v aws >/dev/null 2>&1; then
        echo "❌ AWS CLI not found"
        echo "Install AWS CLI to use S3 cache"
        return 1
    fi
    
    return 0
}

test_s3_access() {
    if ! validate_s3_config; then
        return 1
    fi
    
    # Test read access
    if ! aws s3 ls "s3://$S3_CACHE_BUCKET/" --region "$S3_CACHE_REGION" >/dev/null 2>&1; then
        echo "❌ Cannot read from S3 bucket: $S3_CACHE_BUCKET"
        return 1
    fi
    
    # Test write access with a test file
    local test_key="$S3_CACHE_PREFIX/test-$(date +%s)"
    if echo "test" | aws s3 cp - "s3://$S3_CACHE_BUCKET/$test_key" --region "$S3_CACHE_REGION" >/dev/null 2>&1; then
        aws s3 rm "s3://$S3_CACHE_BUCKET/$test_key" --region "$S3_CACHE_REGION" >/dev/null 2>&1
        return 0
    else
        echo "❌ Cannot write to S3 bucket: $S3_CACHE_BUCKET"
        return 1
    fi
}

get_cache_key() {
    local tool="$1"
    local version="$2"
    local platform=$(uname -s | tr '[:upper:]' '[:lower:]')
    local arch=$(uname -m)
    
    echo "$S3_CACHE_PREFIX/tools/${tool}/${version}/${platform}-${arch}"
}

get_cache_stats_file() {
    echo "$HOME/.cache/mise-s3/stats.json"
}

init_stats_file() {
    local stats_file=$(get_cache_stats_file)
    mkdir -p "$(dirname "$stats_file")"
    
    if [[ ! -f "$stats_file" ]]; then
        cat > "$stats_file" << 'EOF'
{
    "cache_hits": 0,
    "cache_misses": 0, 
    "total_downloads": 0,
    "total_savings_bytes": 0,
    "tools": {}
}
EOF
    fi
}

show_config() {
    echo "📋 S3 Cache Configuration"
    echo "========================"
    echo "Enabled: $S3_CACHE_ENABLED"
    echo "Bucket: $S3_CACHE_BUCKET"
    echo "Region: $S3_CACHE_REGION"
    echo "Prefix: $S3_CACHE_PREFIX"
    echo "TTL: ${S3_CACHE_TTL}s"
    echo ""
    
    if validate_s3_config; then
        echo "✅ Configuration valid"
        
        if test_s3_access; then
            echo "✅ S3 access working"
            
            # Show cache size
            local cache_size=$(aws s3 ls --recursive "s3://$S3_CACHE_BUCKET/$S3_CACHE_PREFIX/" --human-readable --summarize 2>/dev/null | grep "Total Size" | awk '{print $3, $4}' || echo "unknown")
            echo "Cache size: $cache_size"
        else
            echo "❌ S3 access failed"
        fi
    else
        echo "❌ Configuration invalid"
    fi
}