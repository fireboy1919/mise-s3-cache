use mise_s3_cache::config::Config;
use std::env;
use tempfile::TempDir;
use tokio::fs;

#[tokio::test]
async fn test_config_default() {
    let config = Config::default();

    assert!(config.enabled);
    assert!(config.bucket.is_empty());
    assert_eq!(config.region, "us-east-1");
    assert_eq!(config.prefix, "mise-cache");
    assert_eq!(config.ttl_seconds, 604800);
    assert_eq!(config.parallel_uploads, 3);
    assert_eq!(config.compression, "gzip");
}

#[tokio::test]
async fn test_config_from_env() {
    // Set environment variables
    env::set_var("MISE_S3_CACHE_BUCKET", "test-bucket");
    env::set_var("MISE_S3_CACHE_REGION", "us-west-2");
    env::set_var("MISE_S3_CACHE_PREFIX", "test-prefix");
    env::set_var("MISE_S3_CACHE_TTL", "3600");
    env::set_var("MISE_S3_CACHE_PARALLEL_UPLOADS", "5");
    env::set_var("MISE_S3_CACHE_DEBUG", "true");

    let config = Config::load(None).unwrap();

    assert_eq!(config.bucket, "test-bucket");
    assert_eq!(config.region, "us-west-2");
    assert_eq!(config.prefix, "test-prefix");
    assert_eq!(config.ttl_seconds, 3600);
    assert_eq!(config.parallel_uploads, 5);
    assert!(config.debug);

    // Clean up
    env::remove_var("MISE_S3_CACHE_BUCKET");
    env::remove_var("MISE_S3_CACHE_REGION");
    env::remove_var("MISE_S3_CACHE_PREFIX");
    env::remove_var("MISE_S3_CACHE_TTL");
    env::remove_var("MISE_S3_CACHE_PARALLEL_UPLOADS");
    env::remove_var("MISE_S3_CACHE_DEBUG");
}

#[tokio::test]
async fn test_config_from_toml_file() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let toml_content = r#"
enabled = true
bucket = "file-bucket"
region = "eu-west-1"
prefix = "file-prefix"
ttl_seconds = 7200
parallel_uploads = 4
compression = "gzip"
debug = false
"#;

    fs::write(&config_path, toml_content).await.unwrap();

    let config = Config::load(Some(config_path.to_str().unwrap())).unwrap();

    assert!(config.enabled);
    assert_eq!(config.bucket, "file-bucket");
    assert_eq!(config.region, "eu-west-1");
    assert_eq!(config.prefix, "file-prefix");
    assert_eq!(config.ttl_seconds, 7200);
    assert_eq!(config.parallel_uploads, 4);
    assert!(!config.debug);
}

#[tokio::test]
async fn test_config_from_shell_file() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.conf");

    let shell_content = r#"
# This is a comment
S3_CACHE_BUCKET="shell-bucket"
S3_CACHE_REGION="ap-southeast-1"
S3_CACHE_PREFIX="shell-prefix"
S3_CACHE_TTL="1800"
S3_CACHE_DEBUG="true"
"#;

    fs::write(&config_path, shell_content).await.unwrap();

    let config = Config::load(Some(config_path.to_str().unwrap())).unwrap();

    assert_eq!(config.bucket, "shell-bucket");
    assert_eq!(config.region, "ap-southeast-1");
    assert_eq!(config.prefix, "shell-prefix");
    assert_eq!(config.ttl_seconds, 1800);
    assert!(config.debug);
}

#[tokio::test]
async fn test_config_validation() {
    // Test valid config
    env::set_var("MISE_S3_CACHE_BUCKET", "valid-bucket");
    let config = Config::load(None);
    assert!(config.is_ok());
    env::remove_var("MISE_S3_CACHE_BUCKET");

    // Test invalid bucket name
    env::set_var("MISE_S3_CACHE_BUCKET", "Invalid_Bucket");
    let config = Config::load(None);
    assert!(config.is_err());
    env::remove_var("MISE_S3_CACHE_BUCKET");

    // Test empty bucket
    env::set_var("MISE_S3_CACHE_BUCKET", "");
    let config = Config::load(None);
    assert!(config.is_err());
    env::remove_var("MISE_S3_CACHE_BUCKET");

    // Test no bucket configured
    let config = Config::load(None);
    assert!(config.is_err());
}

#[test]
fn test_get_cache_key() {
    let config = Config {
        prefix: "test-prefix".to_string(),
        ..Default::default()
    };

    let key = config.get_cache_key("node", "18.17.0");

    let platform = mise_s3_cache::utils::get_platform();
    let arch = mise_s3_cache::utils::get_architecture();
    let expected = format!("test-prefix/tools/node/18.17.0/{}-{}", platform, arch);

    assert_eq!(key, expected);
}

#[test]
fn test_get_stats_file_path() {
    let config = Config::default();
    let path = config.get_stats_file_path();

    assert!(path.to_string_lossy().contains("mise-s3"));
    assert!(path.to_string_lossy().ends_with("stats.json"));
}

#[tokio::test]
async fn test_config_env_override_file() {
    // Create a config file
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let toml_content = r#"
bucket = "file-bucket"
region = "eu-west-1"
"#;

    fs::write(&config_path, toml_content).await.unwrap();

    // Set environment variable that should override file
    env::set_var("MISE_S3_CACHE_BUCKET", "env-bucket");
    env::set_var("MISE_S3_CACHE_REGION", "us-east-1");

    let config = Config::load(Some(config_path.to_str().unwrap())).unwrap();

    // Environment should override file
    assert_eq!(config.bucket, "env-bucket");
    assert_eq!(config.region, "us-east-1");

    // Clean up
    env::remove_var("MISE_S3_CACHE_BUCKET");
    env::remove_var("MISE_S3_CACHE_REGION");
}

#[tokio::test]
async fn test_config_invalid_toml() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("bad_config.toml");

    let bad_toml = r#"
bucket = "test-bucket"
invalid toml syntax [[[
"#;

    fs::write(&config_path, bad_toml).await.unwrap();

    // Should fall back to shell-style parsing and not crash
    env::set_var("MISE_S3_CACHE_BUCKET", "fallback-bucket");
    let config = Config::load(Some(config_path.to_str().unwrap())).unwrap();
    assert_eq!(config.bucket, "fallback-bucket");

    env::remove_var("MISE_S3_CACHE_BUCKET");
}
