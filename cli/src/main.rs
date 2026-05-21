mod config;
mod auth;
mod upload;
mod client;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "statichub")]
#[command(about = "Static web publishing for frontend developers", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Deploy static files
    Deploy {
        /// Directory to deploy (default: current directory)
        directory: Option<String>,

        /// Project name (requires login)
        #[arg(long)]
        name: Option<String>,
    },

    /// Login with Google OAuth
    Login,

    /// Logout and clear credentials
    Logout,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Deploy { directory, name } => {
            if name.is_some() {
                anyhow::bail!("Named projects require login. Use 'statichub login' first.");
            }

            let dir = directory
                .as_ref()
                .map(|d| std::path::PathBuf::from(d))
                .unwrap_or_else(|| std::env::current_dir().unwrap());

            println!("📦 Collecting files from {}...", dir.display());
            let files = upload::collect_files(&dir)?;
            println!("   Found {} files", files.len());

            let server_url = std::env::var("STATICHUB_SERVER")
                .unwrap_or_else(|_| "http://localhost:3000".to_string());

            println!("🚀 Deploying to {}...", server_url);
            let client = client::Client::new(server_url);
            let response = client.deploy_anonymous(&files).await?;

            println!("✅ Deploy successful!");
            println!("   URL: {}", response.url);
            println!("   Subdomain: {}", response.subdomain);
        }
        Commands::Login => {
            println!("Login command - not yet implemented");
        }
        Commands::Logout => {
            auth::clear_credentials()?;
            println!("✓ Logged out successfully");
        }
    }

    Ok(())
}
