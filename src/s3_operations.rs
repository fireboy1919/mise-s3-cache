#![allow(dead_code)]

use anyhow::{Context, Result};
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::{primitives::ByteStream, Client};
use std::path::Path;
use tokio::fs;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::config::Config;
// use crate::utils;

#[derive(Clone)]
pub struct S3Client {
    client: Client,
    config: Config,
}

impl S3Client {
    pub async fn new(config: &Config) -> Result<Self> {
        let region = aws_config::Region::new(config.region.clone());
        let region_provider = RegionProviderChain::default_provider().or_else(region);

        let mut aws_config_builder =
            aws_config::defaults(aws_config::BehaviorVersion::latest()).region(region_provider);

        // Check for custom endpoint (for MinIO or other S3-compatible services)
        if let Ok(endpoint_url) = std::env::var("AWS_ENDPOINT_URL") {
            debug!("Using custom S3 endpoint: {}", endpoint_url);
            aws_config_builder = aws_config_builder.endpoint_url(endpoint_url);
        }

        let aws_config = aws_config_builder.load().await;
        let client = Client::new(&aws_config);

        Ok(Self {
            client,
            config: config.clone(),
        })
    }

    pub async fn test_connectivity(&self) -> Result<()> {
        // Test bucket access by attempting to list objects
        self.client
            .list_objects_v2()
            .bucket(&self.config.bucket)
            .prefix(&self.config.prefix)
            .max_keys(1)
            .send()
            .await
            .with_context(|| format!("Failed to access S3 bucket: {}", self.config.bucket))?;

        // Test write permissions with a small test file
        let test_key = format!("{}/test-{}", self.config.prefix, Uuid::new_v4());
        let test_content = "test";

        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&test_key)
            .body(ByteStream::from_static(test_content.as_bytes()))
            .send()
            .await
            .with_context(|| "Failed to write test object to S3")?;

        // Clean up test file
        let _ = self
            .client
            .delete_object()
            .bucket(&self.config.bucket)
            .key(&test_key)
            .send()
            .await;

        info!("✅ S3 connectivity test passed");
        Ok(())
    }

    pub async fn object_exists(&self, key: &str) -> Result<bool> {
        match self
            .client
            .head_object()
            .bucket(&self.config.bucket)
            .key(key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(aws_sdk_s3::error::SdkError::ServiceError(err)) => match err.err() {
                aws_sdk_s3::operation::head_object::HeadObjectError::NotFound(_) => Ok(false),
                _ => Err(anyhow::anyhow!("S3 error: {:?}", err)),
            },
            Err(e) => Err(anyhow::anyhow!("S3 error: {}", e)),
        }
    }

    pub async fn upload_file(&self, local_path: &Path, s3_key: &str) -> Result<()> {
        debug!(
            "Uploading {} to s3://{}/{}",
            local_path.display(),
            self.config.bucket,
            s3_key
        );

        let file_size = fs::metadata(local_path).await?.len();
        let body = ByteStream::from_path(local_path)
            .await
            .with_context(|| format!("Failed to read file: {}", local_path.display()))?;

        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(s3_key)
            .body(body)
            .content_length(file_size as i64)
            .send()
            .await
            .with_context(|| format!("Failed to upload {} to S3", s3_key))?;

        debug!("✅ Uploaded {} ({} bytes)", s3_key, file_size);
        Ok(())
    }

    pub async fn download_file(&self, s3_key: &str, local_path: &Path) -> Result<()> {
        debug!(
            "Downloading s3://{}/{} to {}",
            self.config.bucket,
            s3_key,
            local_path.display()
        );

        let response = self
            .client
            .get_object()
            .bucket(&self.config.bucket)
            .key(s3_key)
            .send()
            .await
            .with_context(|| format!("Failed to download {} from S3", s3_key))?;

        // Ensure parent directory exists
        if let Some(parent) = local_path.parent() {
            fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        // Stream the response body to file
        let mut body = response.body;
        let mut file = fs::File::create(local_path)
            .await
            .with_context(|| format!("Failed to create file: {}", local_path.display()))?;

        use tokio::io::AsyncWriteExt;
        // use futures::TryStreamExt;
        while let Some(chunk) = body
            .try_next()
            .await
            .with_context(|| "Failed to read S3 response body")?
        {
            file.write_all(&chunk)
                .await
                .with_context(|| format!("Failed to write to file: {}", local_path.display()))?;
        }

        file.sync_all()
            .await
            .with_context(|| "Failed to sync file to disk")?;

        debug!("✅ Downloaded {}", local_path.display());
        Ok(())
    }

    pub async fn upload_string(&self, content: &str, s3_key: &str) -> Result<()> {
        debug!(
            "Uploading string content to s3://{}/{}",
            self.config.bucket, s3_key
        );

        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(s3_key)
            .body(ByteStream::from(content.as_bytes().to_vec()))
            .content_length(content.len() as i64)
            .send()
            .await
            .with_context(|| format!("Failed to upload string to S3: {}", s3_key))?;

        Ok(())
    }

    pub async fn download_string(&self, s3_key: &str) -> Result<String> {
        debug!(
            "Downloading string from s3://{}/{}",
            self.config.bucket, s3_key
        );

        let response = self
            .client
            .get_object()
            .bucket(&self.config.bucket)
            .key(s3_key)
            .send()
            .await
            .with_context(|| format!("Failed to download string from S3: {}", s3_key))?;

        let bytes = response
            .body
            .collect()
            .await
            .with_context(|| "Failed to collect response body")?;

        String::from_utf8(bytes.to_vec()).with_context(|| "Invalid UTF-8 in S3 object")
    }

    pub async fn list_objects(&self, prefix: &str) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        let mut continuation_token = None;

        loop {
            let mut request = self
                .client
                .list_objects_v2()
                .bucket(&self.config.bucket)
                .prefix(prefix);

            if let Some(token) = continuation_token {
                request = request.continuation_token(token);
            }

            let response = request
                .send()
                .await
                .with_context(|| format!("Failed to list S3 objects with prefix: {}", prefix))?;

            if let Some(contents) = response.contents {
                for object in contents {
                    if let Some(key) = object.key {
                        keys.push(key);
                    }
                }
            }

            if response.is_truncated == Some(true) {
                continuation_token = response.next_continuation_token;
            } else {
                break;
            }
        }

        Ok(keys)
    }

    pub async fn delete_object(&self, s3_key: &str) -> Result<()> {
        debug!("Deleting s3://{}/{}", self.config.bucket, s3_key);

        self.client
            .delete_object()
            .bucket(&self.config.bucket)
            .key(s3_key)
            .send()
            .await
            .with_context(|| format!("Failed to delete S3 object: {}", s3_key))?;

        Ok(())
    }

    pub async fn get_object_size(&self, s3_key: &str) -> Result<u64> {
        let response = self
            .client
            .head_object()
            .bucket(&self.config.bucket)
            .key(s3_key)
            .send()
            .await
            .with_context(|| format!("Failed to get object metadata: {}", s3_key))?;

        Ok(response.content_length.unwrap_or(0) as u64)
    }

    pub async fn get_cache_size(&self, prefix: &str) -> Result<u64> {
        let mut total_size = 0u64;
        let mut continuation_token = None;

        loop {
            let mut request = self
                .client
                .list_objects_v2()
                .bucket(&self.config.bucket)
                .prefix(prefix);

            if let Some(token) = continuation_token {
                request = request.continuation_token(token);
            }

            let response = request.send().await.with_context(|| {
                format!("Failed to list objects for size calculation: {}", prefix)
            })?;

            if let Some(contents) = response.contents {
                for object in contents {
                    total_size += object.size.unwrap_or(0) as u64;
                }
            }

            if response.is_truncated == Some(true) {
                continuation_token = response.next_continuation_token;
            } else {
                break;
            }
        }

        Ok(total_size)
    }

    pub async fn cleanup_old_objects(
        &self,
        prefix: &str,
        max_age_seconds: u64,
    ) -> Result<Vec<String>> {
        let cutoff_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
            - max_age_seconds;

        let mut deleted_keys = Vec::new();
        let mut continuation_token = None;

        loop {
            let mut request = self
                .client
                .list_objects_v2()
                .bucket(&self.config.bucket)
                .prefix(prefix);

            if let Some(token) = continuation_token {
                request = request.continuation_token(token);
            }

            let response = request
                .send()
                .await
                .with_context(|| format!("Failed to list objects for cleanup: {}", prefix))?;

            if let Some(contents) = response.contents {
                for object in contents {
                    if let (Some(key), Some(last_modified)) = (object.key, object.last_modified) {
                        let modified_time = last_modified.secs() as u64;

                        if modified_time < cutoff_time {
                            info!("Deleting old cache entry: {}", key);
                            if let Err(e) = self.delete_object(&key).await {
                                error!("Failed to delete {}: {}", key, e);
                            } else {
                                deleted_keys.push(key);
                            }
                        }
                    }
                }
            }

            if response.is_truncated == Some(true) {
                continuation_token = response.next_continuation_token;
            } else {
                break;
            }
        }

        Ok(deleted_keys)
    }

    pub async fn show_status(&self) {
        println!("📋 S3 Cache Configuration:");
        println!("   Region: {}", self.config.region);
        println!("   Bucket: {}", self.config.bucket);
        println!("   Prefix: {}", self.config.prefix);

        // Test connectivity
        match self.test_connectivity().await {
            Ok(_) => println!("   Status: ✅ Connected"),
            Err(e) => println!("   Status: ❌ Connection failed: {}", e),
        }

        // Get cache size
        let prefix = format!("{}/tools", self.config.prefix);
        match self.get_cache_size(&prefix).await {
            Ok(size) => {
                println!("   Cache size: {}", crate::utils::human_readable_size(size));
            }
            Err(e) => {
                println!("   Cache size: ❌ Failed to calculate: {}", e);
            }
        }

        // Get object count
        match self.list_objects(&prefix).await {
            Ok(objects) => {
                println!("   Cached tools: {}", objects.len());
            }
            Err(e) => {
                println!("   Cached tools: ❌ Failed to list: {}", e);
            }
        }
    }
}
