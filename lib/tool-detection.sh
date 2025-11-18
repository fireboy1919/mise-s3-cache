#!/bin/bash
# Tool detection and project analysis

source "$(dirname "${BASH_SOURCE[0]}")/utils.sh"

is_tool_in_project() {
    local tool="$1"
    local version="$2"
    
    # Check .mise.toml (mise format)
    if [[ -f ".mise.toml" ]]; then
        if check_mise_toml "$tool" "$version"; then
            return 0
        fi
    fi
    
    # Check .tool-versions (asdf format)
    if [[ -f ".tool-versions" ]]; then
        if check_tool_versions "$tool" "$version"; then
            return 0
        fi
    fi
    
    # Check parent directories up to git root or home
    local current_dir="$PWD"
    while [[ "$current_dir" != "/" && "$current_dir" != "$HOME" ]]; do
        current_dir=$(dirname "$current_dir")
        
        if [[ -f "$current_dir/.mise.toml" ]]; then
            if check_mise_toml_at_path "$current_dir/.mise.toml" "$tool" "$version"; then
                return 0
            fi
        fi
        
        if [[ -f "$current_dir/.tool-versions" ]]; then
            if check_tool_versions_at_path "$current_dir/.tool-versions" "$tool" "$version"; then
                return 0
            fi
        fi
        
        # Stop at git root
        if [[ -d "$current_dir/.git" ]]; then
            break
        fi
    done
    
    return 1
}

check_mise_toml() {
    local tool="$1"
    local version="$2"
    
    # Try using mise to parse if available
    if command -v mise >/dev/null 2>&1; then
        local configured_version=$(mise config get "tools.$tool" 2>/dev/null || echo "")
        if [[ "$configured_version" == "$version" ]]; then
            return 0
        fi
    fi
    
    # Fallback to simple grep-based parsing
    if grep -q "^$tool = " .mise.toml 2>/dev/null; then
        local configured_version=$(grep "^$tool = " .mise.toml | sed 's/.*= *"\([^"]*\)".*/\1/' | head -n1)
        if [[ "$configured_version" == "$version" ]]; then
            return 0
        fi
    fi
    
    return 1
}

check_mise_toml_at_path() {
    local file_path="$1"
    local tool="$2"
    local version="$3"
    
    if grep -q "^$tool = " "$file_path" 2>/dev/null; then
        local configured_version=$(grep "^$tool = " "$file_path" | sed 's/.*= *"\([^"]*\)".*/\1/' | head -n1)
        if [[ "$configured_version" == "$version" ]]; then
            return 0
        fi
    fi
    
    return 1
}

check_tool_versions() {
    local tool="$1"
    local version="$2"
    
    if grep -q "^$tool $version$" .tool-versions 2>/dev/null; then
        return 0
    fi
    
    return 1
}

check_tool_versions_at_path() {
    local file_path="$1"
    local tool="$2"
    local version="$3"
    
    if grep -q "^$tool $version$" "$file_path" 2>/dev/null; then
        return 0
    fi
    
    return 1
}

get_project_tools() {
    local tools=()
    
    # Parse .mise.toml
    if [[ -f ".mise.toml" ]]; then
        if command -v mise >/dev/null 2>&1; then
            # Use mise to get configured tools
            while IFS= read -r line; do
                if [[ -n "$line" ]]; then
                    tools+=("$line")
                fi
            done < <(mise config get tools 2>/dev/null | grep -E '^\w+' | sed 's/ = /:/g' | tr -d '"' || echo "")
        else
            # Fallback parsing
            while IFS= read -r line; do
                if [[ "$line" =~ ^([a-zA-Z0-9_-]+)[[:space:]]*=[[:space:]]*\"?([^\"]+)\"? ]]; then
                    local tool="${BASH_REMATCH[1]}"
                    local version="${BASH_REMATCH[2]}"
                    tools+=("${tool}:${version}")
                fi
            done < <(grep -E '^[a-zA-Z0-9_-]+ *= *' .mise.toml 2>/dev/null || echo "")
        fi
    fi
    
    # Parse .tool-versions
    if [[ -f ".tool-versions" ]]; then
        while IFS= read -r line; do
            # Skip comments and empty lines
            [[ "$line" =~ ^#.*$ ]] && continue
            [[ -z "$line" ]] && continue
            
            local tool=$(echo "$line" | awk '{print $1}')
            local version=$(echo "$line" | awk '{print $2}')
            
            if [[ -n "$tool" && -n "$version" ]]; then
                tools+=("${tool}:${version}")
            fi
        done < .tool-versions
    fi
    
    # Remove duplicates and output
    printf '%s\n' "${tools[@]}" | sort -u
}

analyze_project_cache() {
    local tools=($(get_project_tools))
    local cached_count=0
    local missing_count=0
    local cached_tools=()
    local missing_tools=()
    
    if [[ ${#tools[@]} -eq 0 ]]; then
        log_warn "No tools found in .mise.toml or .tool-versions"
        return 1
    fi
    
    log_info "📊 Analyzing cache status for ${#tools[@]} project tools..."
    
    for tool_version in "${tools[@]}"; do
        local tool=$(echo "$tool_version" | cut -d: -f1)
        local version=$(echo "$tool_version" | cut -d: -f2)
        
        if check_s3_cache "$tool" "$version"; then
            cached_count=$((cached_count + 1))
            cached_tools+=("${tool}@${version}")
        else
            missing_count=$((missing_count + 1))
            missing_tools+=("${tool}@${version}")
        fi
    done
    
    echo ""
    echo "📋 Cache Analysis Results:"
    echo "   Total tools: ${#tools[@]}"
    echo "   Already cached: $cached_count"
    echo "   Missing from cache: $missing_count"
    
    if [[ $cached_count -gt 0 ]]; then
        echo ""
        echo "✅ Cached tools:"
        for tool in "${cached_tools[@]}"; do
            echo "   - $tool"
        done
    fi
    
    if [[ $missing_count -gt 0 ]]; then
        echo ""
        echo "❌ Tools needing cache:"
        for tool in "${missing_tools[@]}"; do
            echo "   - $tool"
        done
        
        echo ""
        echo "💡 Run 'mise run s3-cache:warm' to pre-cache missing tools"
    fi
    
    # Calculate cache hit rate if we have data
    if [[ ${#tools[@]} -gt 0 ]]; then
        local hit_rate=$(( (cached_count * 100) / ${#tools[@]} ))
        echo ""
        echo "📈 Cache hit rate: ${hit_rate}%"
    fi
}

warm_project_cache() {
    local tools=($(get_project_tools))
    local max_parallel="${1:-3}"
    
    if [[ ${#tools[@]} -eq 0 ]]; then
        log_warn "No tools found to warm cache"
        return 1
    fi
    
    log_info "🔥 Warming S3 cache for ${#tools[@]} project tools..."
    
    # Check what's already cached
    local missing_tools=()
    for tool_version in "${tools[@]}"; do
        local tool=$(echo "$tool_version" | cut -d: -f1)
        local version=$(echo "$tool_version" | cut -d: -f2)
        
        if ! check_s3_cache "$tool" "$version"; then
            missing_tools+=("${tool_version}")
        else
            log_success "✅ ${tool}@${version} already cached"
        fi
    done
    
    if [[ ${#missing_tools[@]} -eq 0 ]]; then
        log_success "🎉 All project tools already cached!"
        return 0
    fi
    
    log_info "Installing ${#missing_tools[@]} missing tools to warm cache..."
    
    # Install missing tools (mise will cache them automatically via hooks)
    for tool_version in "${missing_tools[@]}"; do
        local tool=$(echo "$tool_version" | cut -d: -f1)
        local version=$(echo "$tool_version" | cut -d: -f2)
        
        log_info "🔧 Installing ${tool}@${version}..."
        
        if command -v mise >/dev/null 2>&1; then
            mise install "${tool}@${version}" || log_warn "Failed to install ${tool}@${version}"
        elif command -v asdf >/dev/null 2>&1; then
            asdf install "$tool" "$version" || log_warn "Failed to install ${tool}@${version}"
        else
            log_error "Neither mise nor asdf found"
            return 1
        fi
    done
    
    log_success "🎉 Cache warming complete!"
}