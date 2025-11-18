use mise_s3_cache::tool_detection::ToolDetector;
use std::env;
use tempfile::TempDir;
use tokio::fs;

#[tokio::test]
async fn test_simple_mise_toml() {
    let temp_dir = TempDir::new().unwrap();
    let toml_path = temp_dir.path().join(".mise.toml");

    let toml_content = r#"[tools]
node = "18.17.0"
terraform = "1.5.0"
python = "3.11.0"
"#;

    fs::write(&toml_path, toml_content).await.unwrap();

    let detector = ToolDetector::new();

    // Test the parsing directly
    let tools = detector.parse_mise_toml(&toml_path).await.unwrap();

    assert_eq!(tools.len(), 3);
    assert!(tools.contains(&("node".to_string(), "18.17.0".to_string())));
    assert!(tools.contains(&("terraform".to_string(), "1.5.0".to_string())));
    assert!(tools.contains(&("python".to_string(), "3.11.0".to_string())));
}

#[tokio::test]
async fn test_simple_tool_versions() {
    let temp_dir = TempDir::new().unwrap();
    let tv_path = temp_dir.path().join(".tool-versions");

    let tv_content = r#"# This is a comment
node 18.17.0
terraform 1.5.0
python 3.11.0
"#;

    fs::write(&tv_path, tv_content).await.unwrap();

    let detector = ToolDetector::new();

    // Test the parsing directly
    let tools = detector.parse_tool_versions(&tv_path).await.unwrap();

    assert_eq!(tools.len(), 3);
    assert!(tools.contains(&("node".to_string(), "18.17.0".to_string())));
    assert!(tools.contains(&("terraform".to_string(), "1.5.0".to_string())));
    assert!(tools.contains(&("python".to_string(), "3.11.0".to_string())));
}

#[tokio::test]
async fn test_no_config_files() {
    // Save current directory
    let current_dir = env::current_dir().unwrap();

    let temp_dir = TempDir::new().unwrap();
    env::set_current_dir(temp_dir.path()).unwrap();

    let detector = ToolDetector::new();
    let tools = detector.get_project_tools().await.unwrap();

    assert!(tools.is_empty());

    // Restore directory
    env::set_current_dir(current_dir).unwrap();
}
