#!/bin/bash
# S3 cache operations

source "$(dirname "${BASH_SOURCE[0]}")/config.sh"
source "$(dirname "${BASH_SOURCE[0]}")/utils.sh"

check_s3_cache() {
    local tool="$1"
    local version="$2"
    
    if ! validate_s3_config; then
        return 1
    fi
    
    local cache_key=$(get_cache_key "$tool" "$version")
    
    aws s3 ls "s3://$S3_CACHE_BUCKET/$cache_key/" \
        --region "$S3_CACHE_REGION" >/dev/null 2>&1
}

restore_from_s3_cache() {
    local tool="$1"
    local version="$2"
    local install_path="$3"
    local cache_key=$(get_cache_key "$tool" "$version")
    local temp_dir=$(mktemp -d)
    local start_time=$(date +%s)
    
    log_info "📦 Downloading ${tool}@${version} from S3 cache"
    
    # Download archive from S3
    local archive_name="${tool}-${version}.tar.gz"
    if ! aws s3 cp "s3://$S3_CACHE_BUCKET/$cache_key/$archive_name" "$temp_dir/" \
         --region "$S3_CACHE_REGION" --quiet 2>/dev/null; then
        rm -rf "$temp_dir"
        return 1
    fi
    
    # Download and verify checksum if available
    local expected_checksum=$(aws s3 cp "s3://$S3_CACHE_BUCKET/$cache_key/checksum.sha256" - \
                              --region "$S3_CACHE_REGION" --quiet 2>/dev/null || echo "")
    
    if [[ -n "$expected_checksum" ]]; then
        local actual_checksum=$(sha256sum "$temp_dir/$archive_name" | cut -d' ' -f1)
        if [[ "$expected_checksum" != "$actual_checksum" ]]; then
            log_warn "⚠️  Checksum mismatch for ${tool}@${version}, skipping cache"
            rm -rf "$temp_dir"
            update_cache_stats "$tool" "$version" "false" "0" "checksum_mismatch"
            return 1
        fi
    fi
    
    # Extract to install path
    mkdir -p "$install_path"
    if tar -xzf "$temp_dir/$archive_name" -C "$install_path" --strip-components=1 2>/dev/null; then
        local end_time=$(date +%s)
        local download_time=$((end_time - start_time))
        
        rm -rf "$temp_dir"
        log_success "✅ Restored ${tool}@${version} from S3 cache (${download_time}s)"
        
        # Update statistics
        local file_size=$(stat -c%s "$temp_dir/$archive_name" 2>/dev/null || echo "0")
        update_cache_stats "$tool" "$version" "true" "$download_time" "success" "$file_size"
        
        return 0
    else
        rm -rf "$temp_dir"
        log_warn "⚠️  Failed to extract ${tool}@${version} from cache"
        update_cache_stats "$tool" "$version" "false" "0" "extraction_failed"
        return 1
    fi
}

store_in_s3_cache() {
    local tool="$1"
    local version="$2"
    local install_path="$3"
    local cache_key=$(get_cache_key "$tool" "$version")
    local temp_dir=$(mktemp -d)
    
    # Only cache if S3 is configured and tool is in project config
    if ! validate_s3_config; then
        return 0
    fi
    
    if ! is_tool_in_project "$tool" "$version"; then
        log_debug "Tool ${tool}@${version} not in project config, skipping cache"
        return 0
    fi
    
    log_debug "📤 Storing ${tool}@${version} in S3 cache (background)"
    
    # Create archive
    local archive_name="${tool}-${version}.tar.gz"
    if ! tar -czf "$temp_dir/$archive_name" -C "$install_path" . 2>/dev/null; then
        rm -rf "$temp_dir"
        return 1
    fi
    
    # Generate checksum
    local checksum=$(sha256sum "$temp_dir/$archive_name" | cut -d' ' -f1)
    echo "$checksum" > "$temp_dir/checksum.sha256"
    
    # Create metadata
    cat > "$temp_dir/metadata.json" << EOF
{
    "tool": "$tool",
    "version": "$version",
    "platform": "$(uname -s)",
    "arch": "$(uname -m)",
    "created": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "size": $(stat -c%s "$temp_dir/$archive_name"),
    "checksum": "$checksum",
    "mise_version": "$(mise version 2>/dev/null || echo "unknown")"
}
EOF
    
    # Upload to S3 (suppress output to not interfere with mise)
    if aws s3 sync "$temp_dir/" "s3://$S3_CACHE_BUCKET/$cache_key/" \
        --region "$S3_CACHE_REGION" --quiet >/dev/null 2>&1; then
        log_debug "✅ Cached ${tool}@${version} to S3"
    else
        log_debug "❌ Failed to cache ${tool}@${version} to S3"
    fi
    
    rm -rf "$temp_dir"
}

update_cache_stats() {
    local tool="$1"
    local version="$2"
    local cache_hit="$3"  # true/false
    local download_time="$4"
    local status="$5"  # success, checksum_mismatch, extraction_failed, etc.
    local file_size="${6:-0}"
    
    local stats_file=$(get_cache_stats_file)
    init_stats_file
    
    # Update statistics using jq (if available) or basic shell
    if command -v jq >/dev/null 2>&1; then
        update_stats_with_jq "$stats_file" "$tool" "$version" "$cache_hit" "$download_time" "$status" "$file_size"
    else
        update_stats_basic "$stats_file" "$tool" "$version" "$cache_hit"
    fi
}

update_stats_with_jq() {
    local stats_file="$1"
    local tool="$2"
    local version="$3"
    local cache_hit="$4"
    local download_time="$5"
    local status="$6"
    local file_size="$7"
    local timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    
    if [[ "$cache_hit" == "true" ]]; then
        jq --arg tool "$tool" --arg version "$version" --arg time "$download_time" \
           --arg ts "$timestamp" --arg status "$status" --argjson size "$file_size" '
            .cache_hits += 1 |
            .total_downloads += 1 |
            .total_savings_bytes += $size |
            .tools[$tool] = (.tools[$tool] // {}) |
            .tools[$tool][$version] = {
                "last_used": $ts,
                "cache_hits": (.tools[$tool][$version].cache_hits // 0) + 1,
                "avg_download_time": $time,
                "status": $status,
                "size_bytes": $size
            }
        ' "$stats_file" > "${stats_file}.tmp" && mv "${stats_file}.tmp" "$stats_file"
    else
        jq --arg tool "$tool" --arg version "$version" --arg ts "$timestamp" --arg status "$status" '
            .cache_misses += 1 |
            .total_downloads += 1 |
            .tools[$tool] = (.tools[$tool] // {}) |
            .tools[$tool][$version] = {
                "last_missed": $ts,
                "cache_misses": (.tools[$tool][$version].cache_misses // 0) + 1,
                "status": $status
            }
        ' "$stats_file" > "${stats_file}.tmp" && mv "${stats_file}.tmp" "$stats_file"
    fi
}

update_stats_basic() {
    local stats_file="$1"
    local tool="$2"
    local version="$3"
    local cache_hit="$4"
    
    # Simple increment without detailed tracking
    if [[ "$cache_hit" == "true" ]]; then
        echo "Cache hit: ${tool}@${version} at $(date)" >> "${stats_file}.log"
    else
        echo "Cache miss: ${tool}@${version} at $(date)" >> "${stats_file}.log"
    fi
}

cleanup_old_cache() {
    local days_old="${1:-7}"
    
    log_info "🧹 Cleaning up S3 cache entries older than $days_old days"
    
    if ! validate_s3_config; then
        log_error "S3 not configured"
        return 1
    fi
    
    # Find and remove old cache entries
    local cutoff_date=$(date -d "$days_old days ago" -u +%Y-%m-%dT%H:%M:%SZ)
    
    aws s3api list-objects-v2 \
        --bucket "$S3_CACHE_BUCKET" \
        --prefix "$S3_CACHE_PREFIX/tools/" \
        --query "Contents[?LastModified<='$cutoff_date'].Key" \
        --output text 2>/dev/null | while read -r key; do
        
        if [[ -n "$key" && "$key" != "None" ]]; then
            log_info "Removing old cache entry: $key"
            aws s3 rm "s3://$S3_CACHE_BUCKET/$key" --region "$S3_CACHE_REGION" --quiet
        fi
    done
    
    log_success "✅ Cache cleanup complete"
}