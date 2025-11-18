use mise_s3_cache::utils::*;

#[test]
fn test_get_platform() {
    let platform = get_platform();
    assert!(["linux", "darwin", "windows", "unknown"].contains(&platform));
}

#[test]
fn test_get_architecture() {
    let arch = get_architecture();
    assert!(!arch.is_empty());
}

#[test]
fn test_is_valid_s3_bucket_name() {
    // Valid names
    assert!(is_valid_s3_bucket_name("my-bucket"));
    assert!(is_valid_s3_bucket_name("bucket123"));
    assert!(is_valid_s3_bucket_name("my.bucket.name"));
    assert!(is_valid_s3_bucket_name("a23"));

    // Invalid names
    assert!(!is_valid_s3_bucket_name("My-Bucket")); // uppercase
    assert!(!is_valid_s3_bucket_name("bucket_name")); // underscore
    assert!(!is_valid_s3_bucket_name("ab")); // too short
    assert!(!is_valid_s3_bucket_name("-bucket")); // starts with dash
    assert!(!is_valid_s3_bucket_name("bucket-")); // ends with dash
    assert!(!is_valid_s3_bucket_name("192.168.1.1")); // IP address format
    assert!(!is_valid_s3_bucket_name("bucket..name")); // consecutive dots
    assert!(!is_valid_s3_bucket_name("bucket.-name")); // period-dash
    assert!(!is_valid_s3_bucket_name("bucket-.name")); // dash-period

    // Edge cases
    assert!(!is_valid_s3_bucket_name("")); // empty
    assert!(!is_valid_s3_bucket_name(&"a".repeat(64))); // too long
}

#[test]
fn test_is_valid_tool_name() {
    // Valid names
    assert!(is_valid_tool_name("node"));
    assert!(is_valid_tool_name("terraform-1.5"));
    assert!(is_valid_tool_name("some_tool"));
    assert!(is_valid_tool_name("tool.name"));
    assert!(is_valid_tool_name("Tool123"));

    // Invalid names
    assert!(!is_valid_tool_name("")); // empty
    assert!(!is_valid_tool_name("tool name")); // space
    assert!(!is_valid_tool_name("tool/name")); // slash
    assert!(!is_valid_tool_name("tool@name")); // at sign
    assert!(!is_valid_tool_name("tool|name")); // pipe
    assert!(!is_valid_tool_name(&"a".repeat(101))); // too long
}

#[test]
fn test_is_valid_version() {
    // Valid versions
    assert!(is_valid_version("1.0.0"));
    assert!(is_valid_version("18.17.0"));
    assert!(is_valid_version("1.5.0-beta.1"));
    assert!(is_valid_version("2.0.0+build.123"));
    assert!(is_valid_version("latest"));
    assert!(is_valid_version("stable"));

    // Invalid versions
    assert!(!is_valid_version("")); // empty
    assert!(!is_valid_version("1.0 0")); // space
    assert!(!is_valid_version("v1.0.0@latest")); // at sign
    assert!(!is_valid_version(&"1".repeat(51))); // too long
}

#[test]
fn test_sanitize_path_component() {
    assert_eq!(sanitize_path_component("node@18.17.0"), "node-18.17.0");
    assert_eq!(sanitize_path_component("tool/name"), "tool-name");
    assert_eq!(sanitize_path_component("valid-name.123"), "valid-name.123");
    assert_eq!(sanitize_path_component("tool name"), "tool-name");
    assert_eq!(sanitize_path_component("tool|name"), "tool-name");
    assert_eq!(sanitize_path_component("tool@#$%name"), "tool----name");
}

#[test]
fn test_calculate_hash() {
    let data = b"hello world";
    let hash = calculate_hash(data);

    // SHA256 of "hello world"
    assert_eq!(
        hash,
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );

    // Test empty data
    let empty_hash = calculate_hash(b"");
    assert_eq!(empty_hash.len(), 64); // SHA256 is always 64 hex chars
}

#[test]
fn test_human_readable_size() {
    assert_eq!(human_readable_size(0), "0 B");
    assert_eq!(human_readable_size(512), "512 B");
    assert_eq!(human_readable_size(1024), "1.0 KB");
    assert_eq!(human_readable_size(1536), "1.5 KB");
    assert_eq!(human_readable_size(1048576), "1.0 MB");
    assert_eq!(human_readable_size(1073741824), "1.0 GB");
    assert_eq!(human_readable_size(1099511627776), "1.0 TB");

    // Test intermediate values
    assert_eq!(human_readable_size(2560), "2.5 KB");
    assert_eq!(human_readable_size(5242880), "5.0 MB");
}

#[test]
fn test_current_timestamp() {
    let timestamp1 = current_timestamp();
    std::thread::sleep(std::time::Duration::from_millis(10));
    let timestamp2 = current_timestamp();

    assert!(timestamp2 >= timestamp1);
    assert!(timestamp2 - timestamp1 <= 1); // Should be less than 1 second difference
}

#[test]
fn test_is_ci_environment() {
    // This will depend on the actual environment where tests run
    // We can't easily test this without manipulating environment variables
    let _is_ci = is_ci_environment();

    // Test with mocked environment
    std::env::set_var("CI", "true");
    assert!(is_ci_environment());

    std::env::remove_var("CI");
    std::env::set_var("GITHUB_ACTIONS", "true");
    assert!(is_ci_environment());

    // Clean up
    std::env::remove_var("GITHUB_ACTIONS");
}
