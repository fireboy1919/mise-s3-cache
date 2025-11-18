#!/bin/bash
# Utility functions

# Logging functions
log_info() {
    echo "ℹ️  $*" >&2
}

log_success() {
    echo "✅ $*" >&2
}

log_warn() {
    echo "⚠️  $*" >&2
}

log_error() {
    echo "❌ $*" >&2
}

log_debug() {
    if [[ "${MISE_S3_CACHE_DEBUG:-false}" == "true" ]]; then
        echo "🐛 $*" >&2
    fi
}

# Check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Get human-readable file size
human_readable_size() {
    local bytes="$1"
    
    if [[ $bytes -lt 1024 ]]; then
        echo "${bytes}B"
    elif [[ $bytes -lt $((1024 * 1024)) ]]; then
        echo "$(( bytes / 1024 ))KB"
    elif [[ $bytes -lt $((1024 * 1024 * 1024)) ]]; then
        echo "$(( bytes / 1024 / 1024 ))MB"
    else
        echo "$(( bytes / 1024 / 1024 / 1024 ))GB"
    fi
}

# Check if running in CI environment
is_ci_environment() {
    [[ -n "${CI:-}" ]] || \
    [[ -n "${BITBUCKET_BUILD_NUMBER:-}" ]] || \
    [[ -n "${GITHUB_ACTIONS:-}" ]] || \
    [[ -n "${GITLAB_CI:-}" ]] || \
    [[ -n "${JENKINS_URL:-}" ]]
}

# Get current project root (git root or current directory)
get_project_root() {
    local current_dir="$PWD"
    
    # Try to find git root
    while [[ "$current_dir" != "/" ]]; do
        if [[ -d "$current_dir/.git" ]]; then
            echo "$current_dir"
            return 0
        fi
        current_dir=$(dirname "$current_dir")
    done
    
    # Fallback to current directory
    echo "$PWD"
}

# Check if we're in a mise/asdf project
is_mise_project() {
    [[ -f ".mise.toml" ]] || [[ -f ".tool-versions" ]]
}

# Get mise/asdf installation directory for a tool
get_tool_install_dir() {
    local tool="$1"
    local version="$2"
    
    if command_exists mise; then
        mise where "$tool" "$version" 2>/dev/null || echo ""
    elif command_exists asdf; then
        echo "$HOME/.asdf/installs/$tool/$version"
    else
        echo ""
    fi
}

# Create a temporary directory with cleanup trap
create_temp_dir() {
    local temp_dir=$(mktemp -d)
    
    # Ensure cleanup on exit
    trap "rm -rf '$temp_dir'" EXIT
    
    echo "$temp_dir"
}

# Validate tool name and version format
validate_tool_version() {
    local tool="$1"
    local version="$2"
    
    # Tool name should be alphanumeric with hyphens/underscores
    if [[ ! "$tool" =~ ^[a-zA-Z0-9_-]+$ ]]; then
        return 1
    fi
    
    # Version should not be empty
    if [[ -z "$version" ]]; then
        return 1
    fi
    
    return 0
}

# Get the size of a directory in bytes
get_directory_size() {
    local dir="$1"
    
    if [[ -d "$dir" ]]; then
        du -sb "$dir" 2>/dev/null | awk '{print $1}' || echo "0"
    else
        echo "0"
    fi
}

# Check if S3 bucket exists and is accessible
verify_s3_bucket() {
    local bucket="$1"
    local region="${2:-us-east-1}"
    
    aws s3api head-bucket --bucket "$bucket" --region "$region" >/dev/null 2>&1
}

# Get AWS CLI version
get_aws_cli_version() {
    aws --version 2>&1 | cut -d' ' -f1 | cut -d'/' -f2 || echo "unknown"
}

# Parse semantic version and compare
version_compare() {
    local version1="$1"
    local version2="$2"
    
    # Simple string comparison for now
    # Could be enhanced for proper semantic version comparison
    if [[ "$version1" == "$version2" ]]; then
        return 0
    else
        return 1
    fi
}

# Create a lockfile to prevent concurrent operations
create_lockfile() {
    local lockfile="$1"
    local timeout="${2:-30}"
    local wait_time=0
    
    while [[ $wait_time -lt $timeout ]]; do
        if (set -C; echo $$ > "$lockfile") 2>/dev/null; then
            # Successfully created lockfile
            trap "rm -f '$lockfile'" EXIT
            return 0
        else
            # Lockfile exists, check if process is still running
            if [[ -f "$lockfile" ]]; then
                local pid=$(cat "$lockfile" 2>/dev/null || echo "")
                if [[ -n "$pid" ]] && ! kill -0 "$pid" 2>/dev/null; then
                    # Process is dead, remove stale lockfile
                    rm -f "$lockfile"
                fi
            fi
            
            sleep 1
            wait_time=$((wait_time + 1))
        fi
    done
    
    return 1
}

# Show a progress spinner for long operations
show_spinner() {
    local pid="$1"
    local message="$2"
    local delay=0.1
    local spinstr='|/-\'
    
    while kill -0 "$pid" 2>/dev/null; do
        local temp=${spinstr#?}
        printf "\r%s %c" "$message" "$spinstr"
        local spinstr=$temp${spinstr%"$temp"}
        sleep $delay
    done
    
    printf "\r%s ✓\n" "$message"
}

# Retry a command with exponential backoff
retry_with_backoff() {
    local max_attempts="$1"
    local delay="$2"
    shift 2
    local command=("$@")
    
    local attempt=1
    while [[ $attempt -le $max_attempts ]]; do
        if "${command[@]}"; then
            return 0
        else
            if [[ $attempt -eq $max_attempts ]]; then
                log_error "Command failed after $max_attempts attempts: ${command[*]}"
                return 1
            else
                log_warn "Command failed, retrying in ${delay}s (attempt $attempt/$max_attempts)"
                sleep "$delay"
                delay=$((delay * 2))  # Exponential backoff
                attempt=$((attempt + 1))
            fi
        fi
    done
}