#![allow(clippy::single_component_path_imports)]
#![allow(clippy::collapsible_else_if)]
#![allow(clippy::redundant_pattern_matching)]

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::{error, info};
use tracing_subscriber;

mod cache;
mod config;
mod s3_operations;
mod tool_detection;
mod utils;

use cache::CacheManager;
use config::Config;
use s3_operations::S3Client;

#[derive(Parser)]
#[command(name = "s3-cache")]
#[command(about = "Intelligent S3 caching for mise tool installations")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Configuration file path
    #[arg(short, long)]
    config: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Check if a tool version exists in S3 cache
    Check {
        /// Tool name (e.g., node, terraform)
        tool: Option<String>,
        /// Tool version (e.g., 18.17.0)
        version: Option<String>,
        /// Check all tools in current project
        #[arg(long)]
        all: bool,
        /// Hook mode - suppress errors and run non-interactively
        #[arg(long)]
        hook_mode: bool,
    },
    /// Download and restore a tool from S3 cache
    Restore {
        /// Tool name
        tool: Option<String>,
        /// Tool version
        version: Option<String>,
        /// Installation path
        #[arg(short, long)]
        path: Option<String>,
        /// Restore all project tools
        #[arg(long)]
        all: bool,
        /// Only restore if exact versions match
        #[arg(long)]
        selective: bool,
        /// Hook mode - suppress errors and run non-interactively
        #[arg(long)]
        hook_mode: bool,
    },
    /// Store a tool installation in S3 cache
    Store {
        /// Tool name
        tool: Option<String>,
        /// Tool version
        version: Option<String>,
        /// Installation path to cache
        #[arg(short, long)]
        path: Option<String>,
        /// Store all installed tools
        #[arg(long)]
        all: bool,
        /// Hook mode - suppress errors and run non-interactively
        #[arg(long)]
        hook_mode: bool,
    },
    /// Show cache statistics
    Stats,
    /// Show configuration
    Status {
        /// Show minimal output
        #[arg(long)]
        quiet: bool,
    },
    /// Analyze current project's cache status
    Analyze,
    /// Warm cache for current project
    Warm {
        /// Maximum parallel operations
        #[arg(short, long, default_value = "3")]
        parallel: usize,
        /// Run in background without blocking
        #[arg(long)]
        background: bool,
        /// Hook mode - suppress errors and run non-interactively
        #[arg(long)]
        hook_mode: bool,
        /// CI mode - prioritize cache hits over speed, fail on errors
        #[arg(long)]
        ci_mode: bool,
    },
    /// Clean old cache entries
    Cleanup {
        /// Age in days for cleanup
        #[arg(short, long, default_value = "7")]
        days: u32,
        /// Only clean temporary files
        #[arg(long)]
        temp_only: bool,
    },
    /// Test S3 connectivity and permissions
    Test,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Extract hook_mode from the command for logging setup
    let hook_mode = match &cli.command {
        Commands::Check { hook_mode, .. } => *hook_mode,
        Commands::Restore { hook_mode, .. } => *hook_mode,
        Commands::Store { hook_mode, .. } => *hook_mode,
        Commands::Warm { hook_mode, .. } => *hook_mode,
        _ => false, // Other commands don't have hook mode
    };

    // In hook mode, suppress logging unless verbose is explicitly set
    let level = match (hook_mode, cli.verbose) {
        (true, false) => "error", // Hook mode: only errors
        (true, true) => "warn",   // Hook mode + verbose: warnings and errors
        (false, true) => "debug", // Normal verbose: everything
        (false, false) => "info", // Normal: info and above
    };

    tracing_subscriber::fmt()
        .with_env_filter(format!("s3_cache={}", level))
        .init();

    // Load configuration - in hook mode, exit silently on config errors
    let config = match Config::load(cli.config.as_deref()) {
        Ok(config) => config,
        Err(e) => {
            if hook_mode {
                std::process::exit(0); // Silent exit in hook mode
            } else {
                return Err(e);
            }
        }
    };

    if !config.enabled {
        if !hook_mode {
            info!("S3 cache is disabled");
        }
        std::process::exit(0);
    }

    // Initialize S3 client and cache manager - handle errors gracefully in hook mode
    let (s3_client, cache_manager) = match (S3Client::new(&config).await, hook_mode) {
        (Ok(s3_client), _) => {
            let cache_manager = CacheManager::new(config.clone(), s3_client.clone());
            (s3_client, cache_manager)
        }
        (Err(_e), true) => {
            // In hook mode, exit silently on S3 connection errors
            std::process::exit(0);
        }
        (Err(e), false) => {
            return Err(e);
        }
    };

    // Execute commands with hook-mode aware error handling
    let result = execute_command(&cli, &cache_manager, &s3_client).await;

    // Extract hook_mode from the command
    let hook_mode = match &cli.command {
        Commands::Check { hook_mode, .. } => *hook_mode,
        Commands::Restore { hook_mode, .. } => *hook_mode,
        Commands::Store { hook_mode, .. } => *hook_mode,
        Commands::Warm { hook_mode, .. } => *hook_mode,
        _ => false, // Other commands don't have hook mode
    };

    match (result, hook_mode) {
        (Ok(_), _) => Ok(()),
        (Err(e), true) => {
            // In hook mode, log error but exit successfully to not break mise
            error!("Hook mode error (non-fatal): {}", e);
            std::process::exit(0);
        }
        (Err(e), false) => Err(e),
    }
}

async fn execute_command(
    cli: &Cli,
    cache_manager: &CacheManager,
    s3_client: &S3Client,
) -> Result<()> {
    match &cli.command {
        Commands::Check {
            tool,
            version,
            all,
            hook_mode,
        } => {
            if *all {
                handle_check_all(cache_manager, *hook_mode).await?;
            } else if let (Some(tool), Some(version)) = (tool, version) {
                handle_check_single(cache_manager, tool, version, *hook_mode).await?;
            } else {
                if !hook_mode {
                    return Err(anyhow::anyhow!(
                        "Must provide --all or both tool and version"
                    ));
                }
            }
        }

        Commands::Restore {
            tool,
            version,
            path,
            all,
            selective,
            hook_mode,
        } => {
            if *all {
                handle_restore_all(cache_manager, *selective, *hook_mode).await?;
            } else if let (Some(tool), Some(version)) = (tool, version) {
                if let Some(install_path) = path {
                    handle_restore_single(cache_manager, tool, version, install_path, *hook_mode)
                        .await?;
                } else {
                    // Auto-determine install path for hooks
                    handle_restore_single_auto_path(cache_manager, tool, version, *hook_mode)
                        .await?;
                }
            } else {
                if !hook_mode {
                    return Err(anyhow::anyhow!(
                        "Must provide --all or both tool and version"
                    ));
                }
            }
        }

        Commands::Store {
            tool,
            version,
            path,
            all,
            hook_mode,
        } => {
            if *all {
                handle_store_all(cache_manager, *hook_mode).await?;
            } else if let (Some(tool), Some(version)) = (tool, version) {
                let default_path = format!("~/.mise/installs/{}/{}", tool, version);
                let install_path = path.as_deref().unwrap_or(&default_path);
                handle_store_single(cache_manager, tool, version, install_path, *hook_mode).await?;
            } else {
                if !hook_mode {
                    return Err(anyhow::anyhow!(
                        "Must provide --all or both tool and version"
                    ));
                }
            }
        }

        Commands::Stats => {
            cache_manager.show_stats().await?;
        }

        Commands::Status { quiet } => {
            if !quiet {
                s3_client.show_status().await;
            }
        }

        Commands::Analyze => {
            cache_manager.analyze_project().await?;
        }

        Commands::Warm {
            parallel,
            background,
            hook_mode,
            ci_mode,
        } => {
            // CI mode overrides background mode - always run in foreground
            if *background && !ci_mode {
                // In background mode, spawn and detach - need to clone for move
                let cache_manager = cache_manager.clone();
                let parallel = *parallel;
                tokio::spawn(async move {
                    if let Err(e) = cache_manager.warm_project_cache(parallel).await {
                        error!("Background warm failed: {}", e);
                    }
                });
                if !hook_mode {
                    println!("🔥 Cache warming started in background");
                }
            } else {
                // Foreground mode (default or CI mode)
                if *ci_mode && !hook_mode {
                    println!("🏗️ CI mode: Prioritizing cache restoration over speed");
                }
                cache_manager.warm_project_cache(*parallel).await?;
                if *ci_mode && !hook_mode {
                    println!("✅ Cache warming completed");
                }
            }
        }

        Commands::Cleanup { days, temp_only } => {
            if *temp_only {
                cache_manager.cleanup_temp_files().await?;
            } else {
                cache_manager.cleanup_old_cache(*days).await?;
            }
        }

        Commands::Test => match s3_client.test_connectivity().await {
            Ok(_) => {
                println!("✅ S3 connectivity test passed");
            }
            Err(e) => {
                error!("❌ S3 connectivity test failed: {}", e);
                return Err(e);
            }
        },
    }

    Ok(())
}

async fn handle_check_single(
    cache_manager: &CacheManager,
    tool: &str,
    version: &str,
    hook_mode: bool,
) -> Result<()> {
    let exists = cache_manager.check_cache(tool, version).await?;
    if exists {
        if !hook_mode {
            println!("✅ {tool}@{version} exists in cache");
        }
        std::process::exit(0);
    } else {
        if !hook_mode {
            println!("❌ {tool}@{version} not found in cache");
        }
        std::process::exit(1);
    }
}

async fn handle_restore_single_auto_path(
    cache_manager: &CacheManager,
    tool: &str,
    version: &str,
    hook_mode: bool,
) -> Result<()> {
    if !hook_mode {
        info!("📦 Restoring {tool}@{version} from S3 cache (auto-path)");
    }

    let success = cache_manager
        .restore_tool_from_cache(tool, version)
        .await?;
    if success {
        if !hook_mode {
            println!("✅ Restored {tool}@{version} from cache");
        }
        // In hook mode, exit with code 0 to indicate success
        if hook_mode {
            std::process::exit(0);
        }
    } else {
        if !hook_mode {
            error!("❌ Failed to restore {tool}@{version} from cache");
        }
        // In hook mode, exit with code 1 to indicate cache miss
        if hook_mode {
            std::process::exit(1);
        }
        return Err(anyhow::anyhow!("Restore failed"));
    }
    Ok(())
}

async fn handle_check_all(cache_manager: &CacheManager, hook_mode: bool) -> Result<()> {
    let tools = cache_manager.get_project_tools().await?;
    let mut all_cached = true;

    for (tool, version) in &tools {
        let exists = cache_manager.check_cache(tool, version).await?;
        if !exists {
            all_cached = false;
            if !hook_mode {
                println!("❌ {}@{} not in cache", tool, version);
            }
        } else if !hook_mode {
            println!("✅ {}@{} cached", tool, version);
        }
    }

    if all_cached {
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}

async fn handle_restore_single(
    cache_manager: &CacheManager,
    tool: &str,
    version: &str,
    path: &str,
    hook_mode: bool,
) -> Result<()> {
    if !hook_mode {
        info!("📦 Restoring {tool}@{version} from S3 cache");
    }

    let success = cache_manager
        .restore_from_cache(tool, version, path)
        .await?;
    if success {
        if !hook_mode {
            println!("✅ Restored {tool}@{version} from cache");
        }
        // In hook mode, exit with code 0 to indicate success
        if hook_mode {
            std::process::exit(0);
        }
    } else {
        if !hook_mode {
            error!("❌ Failed to restore {tool}@{version} from cache");
        }
        // In hook mode, exit with code 1 to indicate cache miss
        if hook_mode {
            std::process::exit(1);
        }
        return Err(anyhow::anyhow!("Restore failed"));
    }
    Ok(())
}

async fn handle_restore_all(
    cache_manager: &CacheManager,
    selective: bool,
    hook_mode: bool,
) -> Result<()> {
    let tools = cache_manager.get_project_tools().await?;
    let mut restored_count = 0;

    for (tool, version) in &tools {
        // In selective mode, only restore exact version matches
        if selective && !cache_manager.check_cache(tool, version).await? {
            continue;
        }

        let path = format!("~/.mise/installs/{}/{}", tool, version);
        if let Ok(_) = cache_manager.restore_from_cache(tool, version, &path).await {
            restored_count += 1;
            if !hook_mode {
                println!("✅ Restored {}@{}", tool, version);
            }
        }
    }

    if !hook_mode {
        println!("📦 Restored {} tools from cache", restored_count);
    }
    Ok(())
}

async fn handle_store_single(
    cache_manager: &CacheManager,
    tool: &str,
    version: &str,
    path: &str,
    hook_mode: bool,
) -> Result<()> {
    if !hook_mode {
        info!("📤 Storing {tool}@{version} in S3 cache");
    }

    cache_manager.store_in_cache(tool, version, path).await?;

    if !hook_mode {
        println!("✅ Stored {tool}@{version} in cache");
    }
    Ok(())
}

async fn handle_store_all(cache_manager: &CacheManager, hook_mode: bool) -> Result<()> {
    let tools = cache_manager.get_installed_tools().await?;
    let mut stored_count = 0;

    for (tool, version, path) in &tools {
        if let Ok(_) = cache_manager.store_in_cache(tool, version, path).await {
            stored_count += 1;
            if !hook_mode {
                println!("✅ Stored {}@{}", tool, version);
            }
        }
    }

    if !hook_mode {
        println!("📤 Stored {} tools in cache", stored_count);
    }
    Ok(())
}
