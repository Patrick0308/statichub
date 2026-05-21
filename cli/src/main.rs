mod config;
mod auth;
mod upload;
mod client;

use anyhow::Context;
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

            let dir = if let Some(d) = directory.as_ref() {
                std::path::PathBuf::from(d)
            } else {
                std::env::current_dir()
                    .context("Failed to get current directory")?
            };

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
            let server_url = std::env::var("STATICHUB_SERVER")
                .unwrap_or_else(|_| "http://localhost:3000".to_string());

            println!("🔐 Logging in to StaticHub...");

            // Generate session ID
            let session_id = auth::generate_session_id();

            // Initiate login
            let client = client::Client::new(server_url.clone());
            let login_response = client.initiate_login(&session_id).await?;

            // Open browser
            println!("📱 Opening browser for authentication...");
            println!("   If the browser doesn't open, visit: {}", login_response.auth_url);

            if let Err(e) = open::that(&login_response.auth_url) {
                println!("   ⚠️  Could not open browser automatically: {}", e);
                println!("   Please open the URL manually in your browser.");
            }

            // Poll for token
            println!("⏳ Waiting for authentication...");
            let mut attempts = 0;
            let max_attempts = 60; // 5 minutes (60 * 5 seconds)

            let token = loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                attempts += 1;

                let status = client.poll_auth_status(&session_id).await?;

                if let Some(token) = status.token {
                    break token;
                }

                if attempts >= max_attempts {
                    anyhow::bail!("Authentication timed out. Please try again.");
                }

                // Show progress every 30 seconds (every 6 attempts)
                if attempts % 6 == 0 {
                    let elapsed_seconds = attempts * 5;
                    println!("   Still waiting... ({}s elapsed)", elapsed_seconds);
                }
            };

            // Save credentials
            auth::save_credentials(&token)?;

            println!("✅ Login successful!");
            println!("   Credentials saved to ~/.statichub/credentials.json");
        }
        Commands::Logout => {
            match auth::load_credentials()? {
                Some(_) => {
                    auth::clear_credentials()?;
                    println!("✅ Logged out successfully");
                    println!("   Credentials removed from ~/.statichub/credentials.json");
                }
                None => {
                    println!("ℹ️  Not logged in");
                }
            }
        }
    }

    Ok(())
}
