#![allow(dead_code)]

use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, warn};

use crate::utils;

#[derive(Clone, Default)]
pub struct ToolDetector;

impl ToolDetector {
    pub fn new() -> Self {
        Self
    }

    pub async fn is_tool_in_project(&self, tool: &str, version: &str) -> Result<bool> {
        // Check current directory and parent directories up to git root
        let mut current_dir = std::env::current_dir()?;

        loop {
            // Check .mise.toml
            let mise_toml = current_dir.join(".mise.toml");
            if mise_toml.exists() && self.check_mise_toml(&mise_toml, tool, version).await? {
                return Ok(true);
            }

            // Check .tool-versions
            let tool_versions = current_dir.join(".tool-versions");
            if tool_versions.exists()
                && self
                    .check_tool_versions(&tool_versions, tool, version)
                    .await?
            {
                return Ok(true);
            }

            // Stop at git root
            if current_dir.join(".git").exists() {
                break;
            }

            // Move to parent directory
            if !current_dir.pop() {
                break;
            }
        }

        Ok(false)
    }

    pub async fn get_project_tools(&self) -> Result<Vec<(String, String)>> {
        let mut tools = Vec::new();

        // Check for .mise.toml in current directory
        if Path::new(".mise.toml").exists() {
            let toml_tools = self.parse_mise_toml(Path::new(".mise.toml")).await?;
            tools.extend(toml_tools);
        }

        // Check for .tool-versions in current directory
        if Path::new(".tool-versions").exists() {
            let tv_tools = self
                .parse_tool_versions(Path::new(".tool-versions"))
                .await?;
            tools.extend(tv_tools);
        }

        // Remove duplicates (prefer .mise.toml over .tool-versions)
        let mut unique_tools = HashMap::new();
        for (tool, version) in tools {
            unique_tools.entry(tool).or_insert(version);
        }

        Ok(unique_tools.into_iter().collect())
    }

    async fn check_mise_toml(&self, file_path: &Path, tool: &str, version: &str) -> Result<bool> {
        debug!("Checking {} for {tool}@{version}", file_path.display());

        // Try using mise command first if available
        if self
            .check_with_mise_command(tool, version)
            .await
            .unwrap_or(false)
        {
            return Ok(true);
        }

        // Fallback to manual parsing
        let tools = self.parse_mise_toml(file_path).await?;
        Ok(tools.iter().any(|(t, v)| t == tool && v == version))
    }

    async fn check_tool_versions(
        &self,
        file_path: &Path,
        tool: &str,
        version: &str,
    ) -> Result<bool> {
        debug!("Checking {} for {tool}@{version}", file_path.display());

        let tools = self.parse_tool_versions(file_path).await?;
        Ok(tools.iter().any(|(t, v)| t == tool && v == version))
    }

    async fn check_with_mise_command(&self, tool: &str, version: &str) -> Result<bool> {
        // Use mise to get the configured version for this tool
        let output = tokio::process::Command::new("mise")
            .args(["ls", "--json"])
            .output()
            .await;

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                self.parse_mise_ls_json(&stdout, tool, version)
            }
            _ => {
                debug!("mise command not available or failed");
                Ok(false)
            }
        }
    }

    fn parse_mise_ls_json(
        &self,
        json_output: &str,
        target_tool: &str,
        target_version: &str,
    ) -> Result<bool> {
        let tools: serde_json::Value = serde_json::from_str(json_output)?;

        if let Some(tools_array) = tools.as_array() {
            for tool_info in tools_array {
                if let (Some(tool), Some(version)) = (
                    tool_info.get("name").and_then(|v| v.as_str()),
                    tool_info.get("version").and_then(|v| v.as_str()),
                ) {
                    if tool == target_tool && version == target_version {
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    pub async fn parse_mise_toml(&self, file_path: &Path) -> Result<Vec<(String, String)>> {
        let content = fs::read_to_string(file_path)
            .await
            .with_context(|| format!("Failed to read {}", file_path.display()))?;

        let mut tools = Vec::new();

        // Try parsing as TOML first
        if let Ok(parsed) = toml::from_str::<toml::Value>(&content) {
            if let Some(tools_section) = parsed.get("tools").and_then(|v| v.as_table()) {
                for (tool, version_value) in tools_section {
                    if let Some(version) = version_value.as_str() {
                        tools.push((tool.clone(), version.to_string()));
                    }
                }
            }
        } else {
            // Fallback to regex-based parsing for simple cases
            warn!(
                "Failed to parse {} as TOML, using regex fallback",
                file_path.display()
            );
            tools.extend(self.parse_mise_toml_regex(&content)?);
        }

        Ok(tools)
    }

    fn parse_mise_toml_regex(&self, content: &str) -> Result<Vec<(String, String)>> {
        let mut tools = Vec::new();

        // Match lines like: tool = "version" or tool = 'version'
        let tool_regex = Regex::new(r#"^([a-zA-Z0-9_.-]+)\s*=\s*['""]([^'""]+)['""]"#)?;

        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            if let Some(captures) = tool_regex.captures(line) {
                let tool = captures.get(1).unwrap().as_str().to_string();
                let version = captures.get(2).unwrap().as_str().to_string();
                tools.push((tool, version));
            }
        }

        Ok(tools)
    }

    pub async fn parse_tool_versions(&self, file_path: &Path) -> Result<Vec<(String, String)>> {
        let content = fs::read_to_string(file_path)
            .await
            .with_context(|| format!("Failed to read {}", file_path.display()))?;

        let mut tools = Vec::new();

        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            // Split by whitespace
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let tool = parts[0].to_string();
                let version = parts[1].to_string();

                // Validate tool and version
                if utils::is_valid_tool_name(&tool) && utils::is_valid_version(&version) {
                    tools.push((tool, version));
                } else {
                    warn!(
                        "Invalid tool/version in .tool-versions: {} {}",
                        parts[0], parts[1]
                    );
                }
            }
        }

        Ok(tools)
    }

    pub async fn find_project_root(&self) -> Option<PathBuf> {
        let mut current_dir = std::env::current_dir().ok()?;

        loop {
            // Check for project markers
            if current_dir.join(".git").exists()
                || current_dir.join(".mise.toml").exists()
                || current_dir.join(".tool-versions").exists()
            {
                return Some(current_dir);
            }

            if !current_dir.pop() {
                break;
            }
        }

        None
    }

    pub async fn get_all_project_tools(&self) -> Result<Vec<(String, String)>> {
        let mut all_tools = Vec::new();

        if let Some(project_root) = self.find_project_root().await {
            let mut current_dir = std::env::current_dir()?;

            // Walk up from current directory to project root
            while current_dir.starts_with(&project_root) {
                // Check .mise.toml
                let mise_toml = current_dir.join(".mise.toml");
                if mise_toml.exists() {
                    let tools = self.parse_mise_toml(&mise_toml).await?;
                    all_tools.extend(tools);
                }

                // Check .tool-versions
                let tool_versions = current_dir.join(".tool-versions");
                if tool_versions.exists() {
                    let tools = self.parse_tool_versions(&tool_versions).await?;
                    all_tools.extend(tools);
                }

                if current_dir == project_root {
                    break;
                }

                if !current_dir.pop() {
                    break;
                }
            }
        }

        // Remove duplicates
        let mut unique_tools = HashMap::new();
        for (tool, version) in all_tools {
            unique_tools.entry(tool).or_insert(version);
        }

        Ok(unique_tools.into_iter().collect())
    }

    pub async fn validate_project_config(&self) -> Result<Vec<String>> {
        let mut issues = Vec::new();

        // Check for .mise.toml
        if Path::new(".mise.toml").exists() {
            match self.parse_mise_toml(Path::new(".mise.toml")).await {
                Ok(tools) => {
                    if tools.is_empty() {
                        issues.push(".mise.toml exists but contains no valid tools".to_string());
                    } else {
                        debug!("Found {} tools in .mise.toml", tools.len());
                    }
                }
                Err(e) => {
                    issues.push(format!("Failed to parse .mise.toml: {}", e));
                }
            }
        }

        // Check for .tool-versions
        if Path::new(".tool-versions").exists() {
            match self.parse_tool_versions(Path::new(".tool-versions")).await {
                Ok(tools) => {
                    if tools.is_empty() {
                        issues
                            .push(".tool-versions exists but contains no valid tools".to_string());
                    } else {
                        debug!("Found {} tools in .tool-versions", tools.len());
                    }
                }
                Err(e) => {
                    issues.push(format!("Failed to parse .tool-versions: {}", e));
                }
            }
        }

        // If neither file exists
        if !Path::new(".mise.toml").exists() && !Path::new(".tool-versions").exists() {
            issues.push(
                "No .mise.toml or .tool-versions file found in current directory".to_string(),
            );
        }

        Ok(issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_parse_mise_toml() {
        let temp_dir = TempDir::new().unwrap();
        let toml_path = temp_dir.path().join(".mise.toml");

        let toml_content = r#"
[tools]
node = "18.17.0"
terraform = "1.5.0"
"#;

        fs::write(&toml_path, toml_content).await.unwrap();

        let detector = ToolDetector::new();
        let tools = detector.parse_mise_toml(&toml_path).await.unwrap();

        assert_eq!(tools.len(), 2);
        assert!(tools.contains(&("node".to_string(), "18.17.0".to_string())));
        assert!(tools.contains(&("terraform".to_string(), "1.5.0".to_string())));
    }

    #[tokio::test]
    async fn test_parse_tool_versions() {
        let temp_dir = TempDir::new().unwrap();
        let tv_path = temp_dir.path().join(".tool-versions");

        let tv_content = r#"
# This is a comment
node 18.17.0
terraform 1.5.0

python 3.11.0
"#;

        fs::write(&tv_path, tv_content).await.unwrap();

        let detector = ToolDetector::new();
        let tools = detector.parse_tool_versions(&tv_path).await.unwrap();

        assert_eq!(tools.len(), 3);
        assert!(tools.contains(&("node".to_string(), "18.17.0".to_string())));
        assert!(tools.contains(&("terraform".to_string(), "1.5.0".to_string())));
        assert!(tools.contains(&("python".to_string(), "3.11.0".to_string())));
    }

    #[tokio::test]
    async fn test_parse_mise_toml_regex_fallback() {
        let detector = ToolDetector::new();

        // Test simple format that might not parse as valid TOML
        let content = r#"
node = "18.17.0"
terraform = "1.5.0"
"#;

        let tools = detector.parse_mise_toml_regex(content).unwrap();
        assert_eq!(tools.len(), 2);
        assert!(tools.contains(&("node".to_string(), "18.17.0".to_string())));
        assert!(tools.contains(&("terraform".to_string(), "1.5.0".to_string())));
    }
}
