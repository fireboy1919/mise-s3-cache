#![allow(dead_code)]

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
// use std::io::{Read, Write};
use std::path::{Path, PathBuf};
// use std::time::{SystemTime, UNIX_EPOCH};
use tar::{Archive, Builder};
use tempfile::TempDir;
use tokio::fs;
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::s3_operations::S3Client;
use crate::tool_detection::ToolDetector;
use crate::utils;

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheMetadata {
    pub tool: String,
    pub version: String,
    pub platform: String,
    pub arch: String,
    pub created_at: u64,
    pub size_bytes: u64,
    pub checksum: String,
    pub mise_version: String,
    pub compressed: bool,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CacheStats {
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub total_downloads: u64,
    pub total_savings_bytes: u64,
    pub tools: HashMap<String, ToolStats>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolStats {
    pub last_used: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub total_download_time_ms: u64,
    pub average_download_time_ms: u64,
    pub size_bytes: u64,
}

#[derive(Clone)]
pub struct CacheManager {
    config: Config,
    s3_client: S3Client,
    tool_detector: ToolDetector,
}

impl CacheManager {
    pub fn new(config: Config, s3_client: S3Client) -> Self {
        let tool_detector = ToolDetector::new();

        Self {
            config,
            s3_client,
            tool_detector,
        }
    }

    pub async fn check_cache(&self, tool: &str, version: &str) -> Result<bool> {
        self.validate_tool_version(tool, version).await?;

        let cache_key = self.config.get_cache_key(tool, version);
        let metadata_key = format!("{}/metadata.json", cache_key);

        self.s3_client.object_exists(&metadata_key).await
    }

    pub async fn restore_from_cache(
        &self,
        tool: &str,
        version: &str,
        install_path: &str,
    ) -> Result<bool> {
        let start_time = std::time::Instant::now();
        self.validate_tool_version(tool, version).await?;

        let cache_key = self.config.get_cache_key(tool, version);
        let archive_key = format!("{}/archive.tar.gz", cache_key);
        let metadata_key = format!("{}/metadata.json", cache_key);
        let checksum_key = format!("{}/checksum.sha256", cache_key);

        // Check if cache entry exists
        if !self.s3_client.object_exists(&metadata_key).await? {
            debug!("Cache miss: {tool}@{version} - metadata not found");
            self.update_stats(tool, version, false, 0, "not_found")
                .await?;
            return Ok(false);
        }

        info!("📦 Restoring {tool}@{version} from S3 cache");

        // Create temp directory for downloads
        let temp_dir = TempDir::new()?;
        let temp_archive = temp_dir.path().join("archive.tar.gz");

        // Download archive and checksum
        match self
            .s3_client
            .download_file(&archive_key, &temp_archive)
            .await
        {
            Ok(_) => {}
            Err(e) => {
                warn!("Failed to download archive for {tool}@{version}: {e}");
                self.update_stats(tool, version, false, 0, "download_failed")
                    .await?;
                return Ok(false);
            }
        }

        // Verify checksum if available
        if let Ok(expected_checksum) = self.s3_client.download_string(&checksum_key).await {
            let actual_checksum = utils::calculate_file_hash(&temp_archive)?;
            if expected_checksum.trim() != actual_checksum {
                warn!("Checksum mismatch for {tool}@{version}");
                self.update_stats(tool, version, false, 0, "checksum_mismatch")
                    .await?;
                return Ok(false);
            }
            debug!("✅ Checksum verified for {tool}@{version}");
        }

        // Extract archive to install path
        let install_path = PathBuf::from(install_path);
        fs::create_dir_all(&install_path).await?;

        match self.extract_archive(&temp_archive, &install_path).await {
            Ok(_) => {
                let duration = start_time.elapsed();
                info!(
                    "✅ Restored {tool}@{version} from cache in {}ms",
                    duration.as_millis()
                );

                // Get file size for stats
                let _file_size = fs::metadata(&temp_archive).await?.len();
                self.update_stats(tool, version, true, duration.as_millis() as u64, "success")
                    .await?;

                Ok(true)
            }
            Err(e) => {
                error!("Failed to extract {tool}@{version}: {e}");
                self.update_stats(tool, version, false, 0, "extraction_failed")
                    .await?;
                Ok(false)
            }
        }
    }

    pub async fn store_in_cache(
        &self,
        tool: &str,
        version: &str,
        install_path: &str,
    ) -> Result<()> {
        self.validate_tool_version(tool, version).await?;

        let install_path = PathBuf::from(install_path);
        if !install_path.exists() {
            return Err(anyhow::anyhow!(
                "Install path does not exist: {}",
                install_path.display()
            ));
        }

        // Only cache if tool is in project configuration
        if !self.tool_detector.is_tool_in_project(tool, version).await? {
            debug!("Tool {tool}@{version} not in project config, skipping cache");
            return Ok(());
        }

        info!("📤 Storing {tool}@{version} in S3 cache");

        let cache_key = self.config.get_cache_key(tool, version);

        // Create temporary archive
        let temp_dir = TempDir::new()?;
        let temp_archive = temp_dir.path().join("archive.tar.gz");

        // Create compressed archive
        let archive_size = self.create_archive(&install_path, &temp_archive).await?;
        debug!("Created archive: {} bytes", archive_size);

        // Calculate checksum
        let checksum = utils::calculate_file_hash(&temp_archive)?;

        // Create metadata
        let metadata = CacheMetadata {
            tool: tool.to_string(),
            version: version.to_string(),
            platform: utils::get_platform().to_string(),
            arch: utils::get_architecture().to_string(),
            created_at: utils::current_timestamp(),
            size_bytes: archive_size,
            checksum: checksum.clone(),
            mise_version: get_mise_version(),
            compressed: true,
        };

        let metadata_json = serde_json::to_string_pretty(&metadata)?;

        // Upload to S3
        let archive_key = format!("{}/archive.tar.gz", cache_key);
        let metadata_key = format!("{}/metadata.json", cache_key);
        let checksum_key = format!("{}/checksum.sha256", cache_key);

        // Upload in parallel using tokio::try_join!
        tokio::try_join!(
            self.s3_client.upload_file(&temp_archive, &archive_key),
            self.s3_client.upload_string(&metadata_json, &metadata_key),
            self.s3_client.upload_string(&checksum, &checksum_key)
        )?;

        info!(
            "✅ Cached {tool}@{version} ({} bytes)",
            utils::human_readable_size(archive_size)
        );
        Ok(())
    }

    async fn create_archive(&self, source_dir: &Path, archive_path: &Path) -> Result<u64> {
        debug!(
            "Creating archive from {} to {}",
            source_dir.display(),
            archive_path.display()
        );

        // Use tokio::task::spawn_blocking for CPU-intensive work
        let source_dir = source_dir.to_path_buf();
        let archive_path = archive_path.to_path_buf();

        tokio::task::spawn_blocking(move || -> Result<u64> {
            let file = std::fs::File::create(&archive_path)?;
            let encoder = GzEncoder::new(file, Compression::default());
            let mut builder = Builder::new(encoder);

            // Add all files from source directory
            builder.append_dir_all(".", &source_dir).with_context(|| {
                format!("Failed to create archive from {}", source_dir.display())
            })?;

            builder.finish()?;

            // Get final archive size
            let metadata = std::fs::metadata(&archive_path)?;
            Ok(metadata.len())
        })
        .await?
    }

    async fn extract_archive(&self, archive_path: &Path, target_dir: &Path) -> Result<()> {
        debug!(
            "Extracting {} to {}",
            archive_path.display(),
            target_dir.display()
        );

        let archive_path = archive_path.to_path_buf();
        let target_dir = target_dir.to_path_buf();

        tokio::task::spawn_blocking(move || -> Result<()> {
            let file = std::fs::File::open(&archive_path)?;
            let decoder = GzDecoder::new(file);
            let mut archive = Archive::new(decoder);

            archive.unpack(&target_dir).with_context(|| {
                format!("Failed to extract archive to {}", target_dir.display())
            })?;

            Ok(())
        })
        .await?
    }

    async fn validate_tool_version(&self, tool: &str, version: &str) -> Result<()> {
        if !utils::is_valid_tool_name(tool) {
            return Err(anyhow::anyhow!("Invalid tool name: {}", tool));
        }

        if !utils::is_valid_version(version) {
            return Err(anyhow::anyhow!("Invalid version: {}", version));
        }

        Ok(())
    }

    pub async fn show_stats(&self) -> Result<()> {
        let stats = self.load_stats().await?;

        println!("📊 Cache Statistics");
        println!("==================");
        println!("Cache hits: {}", stats.cache_hits);
        println!("Cache misses: {}", stats.cache_misses);
        println!("Total downloads: {}", stats.total_downloads);

        if stats.total_downloads > 0 {
            let hit_rate = (stats.cache_hits as f64 / stats.total_downloads as f64) * 100.0;
            println!("Hit rate: {:.1}%", hit_rate);
        }

        println!(
            "Total savings: {}",
            utils::human_readable_size(stats.total_savings_bytes)
        );

        if !stats.tools.is_empty() {
            println!("\n📋 Tool Statistics:");
            for (tool, tool_stats) in &stats.tools {
                println!(
                    "  {}: {} hits, {} misses, {} avg download time",
                    tool,
                    tool_stats.cache_hits,
                    tool_stats.cache_misses,
                    tool_stats.average_download_time_ms
                );
            }
        }

        Ok(())
    }

    pub async fn analyze_project(&self) -> Result<()> {
        let tools = self.tool_detector.get_project_tools().await?;

        if tools.is_empty() {
            warn!("No tools found in .mise.toml or .tool-versions");
            return Ok(());
        }

        info!(
            "📊 Analyzing cache status for {} project tools...",
            tools.len()
        );

        let mut cached_tools = Vec::new();
        let mut missing_tools = Vec::new();

        for (tool, version) in &tools {
            if self.check_cache(tool, version).await? {
                cached_tools.push(format!("{tool}@{version}"));
            } else {
                missing_tools.push(format!("{tool}@{version}"));
            }
        }

        println!("📋 Cache Analysis Results:");
        println!("   Total tools: {}", tools.len());
        println!("   Already cached: {}", cached_tools.len());
        println!("   Missing from cache: {}", missing_tools.len());

        if !cached_tools.is_empty() {
            println!("\n✅ Cached tools:");
            for tool in &cached_tools {
                println!("   - {}", tool);
            }
        }

        if !missing_tools.is_empty() {
            println!("\n❌ Tools needing cache:");
            for tool in &missing_tools {
                println!("   - {}", tool);
            }
            println!("\n💡 Run 's3-cache warm' to pre-cache missing tools");
        }

        if !tools.is_empty() {
            let hit_rate = (cached_tools.len() * 100) / tools.len();
            println!("\n📈 Cache hit rate: {}%", hit_rate);
        }

        Ok(())
    }

    pub async fn warm_project_cache(&self, _max_parallel: usize) -> Result<()> {
        let tools = self.tool_detector.get_project_tools().await?;

        if tools.is_empty() {
            warn!("No tools found to warm cache");
            return Ok(());
        }

        info!("🔥 Warming S3 cache for {} project tools...", tools.len());

        // Find missing tools
        let mut missing_tools = Vec::new();
        for (tool, version) in &tools {
            if !self.check_cache(tool, version).await? {
                missing_tools.push((tool.clone(), version.clone()));
            } else {
                info!("✅ {tool}@{version} already cached");
            }
        }

        if missing_tools.is_empty() {
            info!("🎉 All project tools already cached!");
            return Ok(());
        }

        info!(
            "Installing {} missing tools to warm cache...",
            missing_tools.len()
        );

        // Install missing tools using mise
        for (tool, version) in missing_tools {
            info!("🔧 Installing {tool}@{version}...");

            if let Err(e) = self.install_tool(&tool, &version).await {
                warn!("Failed to install {tool}@{version}: {e}");
            }
        }

        info!("🎉 Cache warming complete!");
        Ok(())
    }

    async fn install_tool(&self, tool: &str, version: &str) -> Result<()> {
        // Try to use mise to install the tool
        let output = tokio::process::Command::new("mise")
            .args(["install", &format!("{tool}@{version}")])
            .output()
            .await
            .context("Failed to execute mise install command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("mise install failed: {}", stderr));
        }

        // After successful installation, cache the tool
        if let Ok(install_path) = self.get_tool_install_path(tool, version).await {
            if let Err(e) = self
                .store_in_cache(tool, version, &install_path.to_string_lossy())
                .await
            {
                warn!("Failed to cache {tool}@{version} after installation: {e}");
            }
        }

        Ok(())
    }

    async fn get_tool_install_path(&self, tool: &str, version: &str) -> Result<PathBuf> {
        let output = tokio::process::Command::new("mise")
            .args(["where", tool, version])
            .output()
            .await
            .context("Failed to execute mise where command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("mise where failed: {}", stderr));
        }

        let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(PathBuf::from(path_str))
    }

    pub async fn cleanup_old_cache(&self, days_old: u32) -> Result<()> {
        info!(
            "🧹 Cleaning up S3 cache entries older than {} days",
            days_old
        );

        let max_age_seconds = days_old as u64 * 24 * 60 * 60;
        let deleted_keys = self
            .s3_client
            .cleanup_old_objects(&format!("{}/tools", self.config.prefix), max_age_seconds)
            .await?;

        info!("✅ Removed {} old cache entries", deleted_keys.len());

        for key in &deleted_keys {
            debug!("Removed: {}", key);
        }

        Ok(())
    }

    async fn update_stats(
        &self,
        tool: &str,
        version: &str,
        cache_hit: bool,
        download_time_ms: u64,
        _status: &str,
    ) -> Result<()> {
        let mut stats = self.load_stats().await?;

        if cache_hit {
            stats.cache_hits += 1;
        } else {
            stats.cache_misses += 1;
        }

        stats.total_downloads += 1;

        let tool_key = format!("{tool}@{version}");
        let tool_stats = stats.tools.entry(tool_key).or_insert_with(|| ToolStats {
            last_used: utils::current_timestamp(),
            cache_hits: 0,
            cache_misses: 0,
            total_download_time_ms: 0,
            average_download_time_ms: 0,
            size_bytes: 0,
        });

        tool_stats.last_used = utils::current_timestamp();

        if cache_hit {
            tool_stats.cache_hits += 1;
            tool_stats.total_download_time_ms += download_time_ms;
            tool_stats.average_download_time_ms =
                tool_stats.total_download_time_ms / tool_stats.cache_hits;
        } else {
            tool_stats.cache_misses += 1;
        }

        self.save_stats(&stats).await?;
        Ok(())
    }

    async fn load_stats(&self) -> Result<CacheStats> {
        let stats_path = self.config.get_stats_file_path();

        if !stats_path.exists() {
            // Create parent directory
            if let Some(parent) = stats_path.parent() {
                fs::create_dir_all(parent).await?;
            }
            return Ok(CacheStats::default());
        }

        let content = fs::read_to_string(&stats_path).await?;
        let stats: CacheStats =
            serde_json::from_str(&content).unwrap_or_else(|_| CacheStats::default());

        Ok(stats)
    }

    async fn save_stats(&self, stats: &CacheStats) -> Result<()> {
        let stats_path = self.config.get_stats_file_path();

        // Create parent directory
        if let Some(parent) = stats_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let json = serde_json::to_string_pretty(stats)?;
        fs::write(&stats_path, json).await?;

        Ok(())
    }

    pub async fn get_project_tools(&self) -> Result<Vec<(String, String)>> {
        self.tool_detector.get_project_tools().await
    }

    pub async fn get_installed_tools(&self) -> Result<Vec<(String, String, String)>> {
        // Get tools from the project configuration
        let project_tools = self.tool_detector.get_project_tools().await?;
        let mut installed_tools = Vec::new();

        for (tool, version) in project_tools {
            // Check if the tool is actually installed by trying to get its path
            if let Ok(install_path) = self.get_tool_install_path(&tool, &version).await {
                if install_path.exists() {
                    installed_tools.push((
                        tool,
                        version,
                        install_path.to_string_lossy().to_string(),
                    ));
                }
            }
        }

        Ok(installed_tools)
    }

    pub async fn cleanup_temp_files(&self) -> Result<()> {
        info!("🧹 Cleaning up temporary cache files");

        // Get cache directory
        let cache_dir = self.config.get_cache_dir();
        let temp_dir = cache_dir.join("tmp");

        if !temp_dir.exists() {
            info!("No temporary files to clean");
            return Ok(());
        }

        let mut count = 0;
        let mut entries = fs::read_dir(&temp_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                if let Err(e) = fs::remove_file(&path).await {
                    warn!("Failed to remove temp file {}: {}", path.display(), e);
                } else {
                    count += 1;
                    debug!("Removed temp file: {}", path.display());
                }
            }
        }

        info!("✅ Cleaned up {} temporary files", count);
        Ok(())
    }
}

fn get_mise_version() -> String {
    std::process::Command::new("mise")
        .arg("version")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
