use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser)]
#[command(name = "blogctl")]
#[command(about = "Research blog build and management tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new article
    New {
        /// Article slug
        slug: String,
    },
    /// Build the blog
    Build,
    /// Check tools and configuration
    Check,
    /// Build search index
    Index,
    /// Start local server with watch
    Serve {
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::New { slug } => commands::new_article(&slug),
        Commands::Build => commands::build().await,
        Commands::Check => commands::check(),
        Commands::Index => commands::index(),
        Commands::Serve { port } => commands::serve(port).await,
    }
}
