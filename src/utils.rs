#![allow(dead_code)]

use anyhow::Result;
use regex::Regex;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Get the current platform name (linux, darwin, windows)
pub fn get_platform() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    }
}

/// Get the current architecture (x86_64, aarch64, etc.)
pub fn get_architecture() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "arm") {
        "arm"
    } else {
        "unknown"
    }
}

/// Validate S3 bucket name according to AWS rules
pub fn is_valid_s3_bucket_name(name: &str) -> bool {
    if name.len() < 3 || name.len() > 63 {
        return false;
    }

    // Must start and end with lowercase letter or number
    let first_char = name.chars().next().unwrap_or(' ');
    let last_char = name.chars().last().unwrap_or(' ');

    if !first_char.is_ascii_lowercase() && !first_char.is_ascii_digit() {
        return false;
    }

    if !last_char.is_ascii_lowercase() && !last_char.is_ascii_digit() {
        return false;
    }

    // Can contain only lowercase letters, numbers, hyphens, and periods
    let valid_chars_regex = Regex::new(r"^[a-z0-9.-]+$").unwrap();
    if !valid_chars_regex.is_match(name) {
        return false;
    }

    // Cannot have consecutive periods or period-dash combinations
    if name.contains("..") || name.contains(".-") || name.contains("-.") {
        return false;
    }

    // Cannot be formatted as IP address
    let ip_regex = Regex::new(r"^\d+\.\d+\.\d+\.\d+$").unwrap();
    if ip_regex.is_match(name) {
        return false;
    }

    true
}

/// Validate tool name and version for safety
pub fn is_valid_tool_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 100 {
        return false;
    }

    // Allow alphanumeric, hyphens, underscores, and dots
    let regex = Regex::new(r"^[a-zA-Z0-9._-]+$").unwrap();
    regex.is_match(name)
}

/// Validate version string
pub fn is_valid_version(version: &str) -> bool {
    if version.is_empty() || version.len() > 50 {
        return false;
    }

    // Allow alphanumeric, dots, hyphens, and plus signs (for semver)
    let regex = Regex::new(r"^[a-zA-Z0-9.+-]+$").unwrap();
    regex.is_match(version)
}

/// Sanitize a string for use in file paths or S3 keys
pub fn sanitize_path_component(input: &str) -> String {
    // Replace any character that's not alphanumeric, dash, underscore, or dot with dash
    let regex = Regex::new(r"[^a-zA-Z0-9._-]").unwrap();
    regex.replace_all(input, "-").to_string()
}

/// Calculate SHA256 hash of a file
pub fn calculate_file_hash(path: &Path) -> Result<String> {
    let content = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&content);
    Ok(format!("{:x}", hasher.finalize()))
}

/// Calculate SHA256 hash of a byte slice
pub fn calculate_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Convert bytes to human-readable size
pub fn human_readable_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: u64 = 1024;

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= THRESHOLD as f64 && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD as f64;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", size as u64, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Check if running in a CI environment
pub fn is_ci_environment() -> bool {
    std::env::var("CI").is_ok()
        || std::env::var("GITHUB_ACTIONS").is_ok()
        || std::env::var("GITLAB_CI").is_ok()
        || std::env::var("BITBUCKET_BUILD_NUMBER").is_ok()
        || std::env::var("JENKINS_URL").is_ok()
        || std::env::var("BUILDKITE").is_ok()
}

/// Get current timestamp in seconds since Unix epoch
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Check if a path exists and is a directory
pub fn is_directory(path: &Path) -> bool {
    path.exists() && path.is_dir()
}

/// Find project root by looking for .git directory
pub fn find_project_root() -> Option<std::path::PathBuf> {
    let mut current = std::env::current_dir().ok()?;

    loop {
        if current.join(".git").exists() {
            return Some(current);
        }

        if !current.pop() {
            break;
        }
    }

    None
}

/// Get the size of a directory recursively
pub fn get_directory_size(path: &Path) -> Result<u64> {
    let mut total_size = 0;

    if path.is_file() {
        return Ok(std::fs::metadata(path)?.len());
    }

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;

        if metadata.is_file() {
            total_size += metadata.len();
        } else if metadata.is_dir() {
            total_size += get_directory_size(&entry.path())?;
        }
    }

    Ok(total_size)
}

/// Create a temporary file with a specific extension
pub fn create_temp_file_with_extension(extension: &str) -> Result<tempfile::NamedTempFile> {
    tempfile::Builder::new()
        .suffix(&format!(".{}", extension))
        .tempfile()
        .map_err(anyhow::Error::from)
}

/// Retry a function with exponential backoff
pub async fn retry_with_backoff<F, Fut, T, E>(
    mut operation: F,
    max_attempts: usize,
    initial_delay_ms: u64,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut delay = initial_delay_ms;

    for attempt in 1..=max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt == max_attempts {
                    return Err(e);
                }

                tracing::warn!("Attempt {} failed: {}. Retrying in {}ms", attempt, e, delay);
                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                delay *= 2; // Exponential backoff
            }
        }
    }

    unreachable!("Should have returned from the loop")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_s3_bucket_name() {
        // Valid names
        assert!(is_valid_s3_bucket_name("my-bucket"));
        assert!(is_valid_s3_bucket_name("bucket123"));
        assert!(is_valid_s3_bucket_name("my.bucket.name"));

        // Invalid names
        assert!(!is_valid_s3_bucket_name("My-Bucket")); // uppercase
        assert!(!is_valid_s3_bucket_name("bucket_name")); // underscore
        assert!(!is_valid_s3_bucket_name("ab")); // too short
        assert!(!is_valid_s3_bucket_name("-bucket")); // starts with dash
        assert!(!is_valid_s3_bucket_name("bucket-")); // ends with dash
        assert!(!is_valid_s3_bucket_name("192.168.1.1")); // IP address format
        assert!(!is_valid_s3_bucket_name("bucket..name")); // consecutive dots
    }

    #[test]
    fn test_is_valid_tool_name() {
        // Valid names
        assert!(is_valid_tool_name("node"));
        assert!(is_valid_tool_name("terraform-1.5"));
        assert!(is_valid_tool_name("some_tool"));
        assert!(is_valid_tool_name("tool.name"));

        // Invalid names
        assert!(!is_valid_tool_name("")); // empty
        assert!(!is_valid_tool_name("tool name")); // space
        assert!(!is_valid_tool_name("tool/name")); // slash
        assert!(!is_valid_tool_name("tool@name")); // at sign
    }

    #[test]
    fn test_human_readable_size() {
        assert_eq!(human_readable_size(0), "0 B");
        assert_eq!(human_readable_size(512), "512 B");
        assert_eq!(human_readable_size(1024), "1.0 KB");
        assert_eq!(human_readable_size(1536), "1.5 KB");
        assert_eq!(human_readable_size(1048576), "1.0 MB");
        assert_eq!(human_readable_size(1073741824), "1.0 GB");
    }

    #[test]
    fn test_sanitize_path_component() {
        assert_eq!(sanitize_path_component("node@18.17.0"), "node-18.17.0");
        assert_eq!(sanitize_path_component("tool/name"), "tool-name");
        assert_eq!(sanitize_path_component("valid-name.123"), "valid-name.123");
    }
}
