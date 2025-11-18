#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};

use crate::s3_operations::S3Client;
use crate::utils;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub enabled: bool,
    pub bucket: String,
    pub region: String,
    pub prefix: String,
    pub ttl_seconds: u64,
    pub parallel_uploads: usize,
    pub compression: String,
    pub debug: bool,
    pub log_file: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            bucket: String::new(),
            region: "us-east-1".to_string(),
            prefix: "mise-cache".to_string(),
            ttl_seconds: 604800, // 7 days
            parallel_uploads: 3,
            compression: "gzip".to_string(),
            debug: false,
            log_file: None,
        }
    }
}

impl Config {
    pub fn load(config_path: Option<&str>) -> Result<Self> {
        let mut config = Self::default();

        // Load from config files first
        config.load_from_files(config_path)?;

        // Load from environment variables (overrides files)
        config.load_from_env();

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    fn load_from_env(&mut self) {
        if let Ok(val) = env::var("MISE_S3_CACHE_ENABLED") {
            self.enabled = val.to_lowercase() == "true";
        }

        if let Ok(val) = env::var("MISE_S3_CACHE_BUCKET") {
            self.bucket = val;
        }

        if let Ok(val) = env::var("MISE_S3_CACHE_REGION") {
            self.region = val;
        }

        if let Ok(val) = env::var("MISE_S3_CACHE_PREFIX") {
            self.prefix = val;
        }

        if let Ok(val) = env::var("MISE_S3_CACHE_TTL") {
            if let Ok(ttl) = val.parse::<u64>() {
                self.ttl_seconds = ttl;
            }
        }

        if let Ok(val) = env::var("MISE_S3_CACHE_PARALLEL_UPLOADS") {
            if let Ok(parallel) = val.parse::<usize>() {
                self.parallel_uploads = parallel;
            }
        }

        if let Ok(val) = env::var("MISE_S3_CACHE_DEBUG") {
            self.debug = val.to_lowercase() == "true";
        }

        if let Ok(val) = env::var("MISE_S3_CACHE_LOG_FILE") {
            self.log_file = Some(PathBuf::from(val));
        }
    }

    fn load_from_files(&mut self, config_path: Option<&str>) -> Result<()> {
        let config_paths = if let Some(path) = config_path {
            vec![PathBuf::from(path)]
        } else {
            self.get_default_config_paths()
        };

        for path in config_paths {
            if path.exists() {
                match self.load_from_file(&path) {
                    Ok(_) => info!("Loaded config from {}", path.display()),
                    Err(e) => warn!("Failed to load config from {}: {}", path.display(), e),
                }
            }
        }

        Ok(())
    }

    fn get_default_config_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // Project-specific config
        paths.push(PathBuf::from(".mise-s3-cache.toml"));
        paths.push(PathBuf::from(".mise-s3-cache.conf"));

        // User config
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join(".config/mise/s3-cache.toml"));
            paths.push(home.join(".config/mise/s3-cache.conf"));
        }

        paths
    }

    fn load_from_file(&mut self, path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        if path.extension().and_then(|s| s.to_str()) == Some("toml") {
            let file_config: Config = toml::from_str(&content)
                .with_context(|| format!("Failed to parse TOML config: {}", path.display()))?;
            self.merge_with(file_config);
        } else {
            // Parse shell-style config (for backwards compatibility)
            self.load_shell_config(&content)?;
        }

        Ok(())
    }

    fn load_shell_config(&mut self, content: &str) -> Result<()> {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');

                match key {
                    "S3_CACHE_ENABLED" => self.enabled = value.to_lowercase() == "true",
                    "S3_CACHE_BUCKET" => self.bucket = value.to_string(),
                    "S3_CACHE_REGION" => self.region = value.to_string(),
                    "S3_CACHE_PREFIX" => self.prefix = value.to_string(),
                    "S3_CACHE_TTL" => {
                        if let Ok(ttl) = value.parse::<u64>() {
                            self.ttl_seconds = ttl;
                        }
                    }
                    "S3_CACHE_PARALLEL_UPLOADS" => {
                        if let Ok(parallel) = value.parse::<usize>() {
                            self.parallel_uploads = parallel;
                        }
                    }
                    "S3_CACHE_DEBUG" => self.debug = value.to_lowercase() == "true",
                    _ => {} // Ignore unknown keys
                }
            }
        }
        Ok(())
    }

    fn merge_with(&mut self, other: Config) {
        if !other.bucket.is_empty() {
            self.bucket = other.bucket;
        }
        self.region = other.region;
        self.prefix = other.prefix;
        self.ttl_seconds = other.ttl_seconds;
        self.parallel_uploads = other.parallel_uploads;
        self.compression = other.compression;
        self.debug = other.debug;
        if other.log_file.is_some() {
            self.log_file = other.log_file;
        }
    }

    fn validate(&self) -> Result<()> {
        if self.bucket.is_empty() {
            return Err(anyhow::anyhow!(
                "S3 bucket not configured. Set MISE_S3_CACHE_BUCKET environment variable"
            ));
        }

        // Validate bucket name format (basic validation)
        if !utils::is_valid_s3_bucket_name(&self.bucket) {
            return Err(anyhow::anyhow!("Invalid S3 bucket name: {}", self.bucket));
        }

        // Validate region
        if self.region.is_empty() {
            return Err(anyhow::anyhow!("S3 region cannot be empty"));
        }

        // Validate prefix
        if self.prefix.contains("//") || self.prefix.starts_with('/') {
            return Err(anyhow::anyhow!("Invalid S3 prefix: {}", self.prefix));
        }

        Ok(())
    }

    pub fn get_cache_key(&self, tool: &str, version: &str) -> String {
        let platform = utils::get_platform();
        let arch = utils::get_architecture();
        format!(
            "{}/tools/{}/{}/{}-{}",
            self.prefix, tool, version, platform, arch
        )
    }

    pub async fn show_status(&self, s3_client: &S3Client) {
        println!("📋 S3 Cache Configuration");
        println!("========================");
        println!("Enabled: {}", self.enabled);
        println!("Bucket: {}", self.bucket);
        println!("Region: {}", self.region);
        println!("Prefix: {}", self.prefix);
        println!("TTL: {}s", self.ttl_seconds);
        println!("Parallel uploads: {}", self.parallel_uploads);
        println!("Compression: {}", self.compression);
        println!("Debug: {}", self.debug);

        if let Some(log_file) = &self.log_file {
            println!("Log file: {}", log_file.display());
        }

        println!();

        // Test S3 connectivity
        match s3_client.test_connectivity().await {
            Ok(_) => {
                println!("✅ S3 connectivity: OK");

                // Get cache size
                match s3_client.get_cache_size(&self.prefix).await {
                    Ok(size) => println!("📊 Cache size: {}", utils::human_readable_size(size)),
                    Err(e) => warn!("Failed to get cache size: {}", e),
                }
            }
            Err(e) => {
                error!("❌ S3 connectivity: FAILED - {}", e);
            }
        }
    }

    pub fn get_stats_file_path(&self) -> PathBuf {
        if let Some(home) = dirs::home_dir() {
            home.join(".cache/mise-s3/stats.json")
        } else {
            PathBuf::from(".mise-s3-stats.json")
        }
    }

    pub fn get_cache_dir(&self) -> PathBuf {
        if let Some(home) = dirs::home_dir() {
            home.join(".cache/mise-s3")
        } else {
            PathBuf::from(".mise-s3-cache")
        }
    }
}

// Add toml dependency to Cargo.toml for this to work
