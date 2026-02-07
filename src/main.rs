use anyhow::{Context, Result};
use clap::Parser;
use rusted_tools::{api, config};
use std::path::PathBuf;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "rusted-tools")]
#[command(about = "High-performance MCP proxy server", long_about = None)]
#[command(version)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,

    /// Override log level (trace, debug, info, warn, error)
    #[arg(long)]
    log_level: Option<String>,

    /// Override log format (pretty, json)
    #[arg(long)]
    log_format: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize rustls crypto provider for axum-reverse-proxy
    // This must be done before any TLS operations
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    // Load .env file if it exists
    let _ = dotenvy::dotenv();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Load configuration
    let mut config = config::load_config(&cli.config).with_context(|| {
        format!(
            "Failed to load configuration from: {}",
            cli.config.display()
        )
    })?;

    // Apply CLI overrides
    if let Some(log_level) = cli.log_level {
        config.logging.level = log_level;
    }
    if let Some(log_format) = cli.log_format {
        config.logging.format = log_format;
    }

    // Initialize logging
    init_logging(&config.logging)?;

    // Print banner
    print_banner(&config);

    // Start the proxy server
    info!("Starting rusted-tools MCP proxy server...");
    api::start_server(config).await?;

    Ok(())
}

fn init_logging(config: &config::LoggingConfig) -> Result<()> {
    use tracing_subscriber::{EnvFilter, fmt, prelude::*};

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.level));

    match config.format.as_str() {
        "json" => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().json())
                .init();
        }
        _ => {
            // Default to pretty format
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().pretty())
                .init();
        }
    }

    Ok(())
}

fn print_banner(config: &config::AppConfig) {
    let version = env!("CARGO_PKG_VERSION");
    let authors = env!("CARGO_PKG_AUTHORS");
    let width = 59usize;
    let border = "═".repeat(width + 2);
    let line = |content: &str| {
        info!("║ {:width$} ║", content, width = width);
    };

    info!("╔{}╗", border);
    line("RUSTED-TOOLS");
    line(&format!("MCP Proxy Server v{}", version));
    line("");
    line(&format!("Author: {}", authors));
    info!("╚{}╝", border);
    info!("");
    info!("Server Configuration:");
    info!("  → Address: {}:{}", config.http.host, config.http.port);
    info!("  → Log Level: {}", config.logging.level);
    info!("  → Log Format: {}", config.logging.format);
    info!("  → MCP Endpoints: {}", config.endpoints.len());
    info!("");
}
