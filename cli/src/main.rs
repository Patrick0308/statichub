mod config;
mod auth;
mod upload;
mod client;

use anyhow::Context;
use clap::{ArgAction, CommandFactory, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "statichub")]
#[command(about = "Static web publishing for frontend developers", long_about = None)]
struct Cli {
    /// Print version
    #[arg(short = 'V', long = "version", action = ArgAction::SetTrue)]
    version: bool,

    #[command(subcommand)]
    command: Option<Commands>,
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

        /// Path to config file (default: auto-detect statichub.yaml)
        #[arg(long)]
        config: Option<String>,
    },

    /// Login with Google OAuth
    Login,

    /// Logout and clear credentials
    Logout,

    /// List your projects
    List,

    /// Show project details and deploy history
    Info {
        /// Project name
        project: String,
    },

    /// Rollback project to a previous version
    Rollback {
        /// Project name
        project: String,
        /// Version to rollback to
        version: i64,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.version {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let Some(command) = cli.command else {
        Cli::command().print_help()?;
        println!();
        return Ok(());
    };

    match command {
        Commands::Deploy { directory, name, config: config_path } => {
            let dir = if let Some(d) = directory.as_ref() {
                std::path::PathBuf::from(d)
            } else {
                std::env::current_dir()
                    .context("Failed to get current directory")?
            };

            println!("📦 Collecting files from {}...", dir.display());
            let files = upload::collect_files(&dir)?;
            println!("   Found {} files", files.len());

            // Load config if specified or auto-detect
            let config = if let Some(path) = config_path {
                let config_file = std::path::PathBuf::from(path);
                Some(config::load_config(&config_file)?)
            } else if let Some(found) = config::find_config_file(&dir) {
                println!("   Using config: {}", found.display());
                Some(config::load_config(&found)?)
            } else {
                None
            };

            let server_url = std::env::var("STATICHUB_SERVER")
                .unwrap_or_else(|_| "https://statichub.dev".to_string());

            let client = client::Client::new(server_url.clone());

            let response = if let Some(project_name) = name {
                // Authenticated deploy
                let credentials = auth::load_credentials()?
                    .ok_or_else(|| anyhow::anyhow!(
                        "Not logged in. Run 'statichub login' first to deploy named projects."
                    ))?;

                println!("🚀 Deploying to project '{}' on {}...", project_name, server_url);
                client.deploy_authenticated(&project_name, &files, &credentials.access_token, config.as_ref()).await?
            } else {
                // Anonymous deploy
                println!("🚀 Deploying to {}...", server_url);
                client.deploy_anonymous(&files, config.as_ref()).await?
            };

            println!("✅ Deploy successful!");
            println!("   URL: {}", response.url);
            println!("   Subdomain: {}", response.subdomain);
            if let Some(version) = response.version {
                println!("   Version: {}", version);
            }
        }
        Commands::Login => {
            let server_url = std::env::var("STATICHUB_SERVER")
                .unwrap_or_else(|_| "https://statichub.dev".to_string());

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
        Commands::List => {
            let credentials = auth::load_credentials()?
                .ok_or_else(|| anyhow::anyhow!("Not logged in. Run 'statichub login' first."))?;

            let server_url = std::env::var("STATICHUB_SERVER")
                .unwrap_or_else(|_| "https://statichub.dev".to_string());

            let client = client::Client::new(server_url);
            let projects = client.list_projects(&credentials.access_token).await?;

            if projects.is_empty() {
                println!("📭 No projects yet");
                println!("   Deploy with a name: statichub deploy --name my-app");
            } else {
                println!("📋 Your projects:\n");
                for project in projects {
                    println!("  {} - {}", project.name, project.url);
                    if let Some(version) = project.current_version {
                        println!("    Version: {}", version);
                    }
                    if let Some(deployed_at) = project.last_deployed_at {
                        println!("    Last deployed: {}", deployed_at);
                    }
                    println!();
                }
            }
        }
        Commands::Info { project } => {
            let credentials = auth::load_credentials()?
                .ok_or_else(|| anyhow::anyhow!("Not logged in. Run 'statichub login' first."))?;

            let server_url = std::env::var("STATICHUB_SERVER")
                .unwrap_or_else(|_| "https://statichub.dev".to_string());

            let client = client::Client::new(server_url);
            let info = client.get_project_info(&project, &credentials.access_token).await?;

            println!("📦 Project: {}", info.name);
            println!("   URL: {}", info.url);
            println!("   Created: {}", info.created_at);
            if let Some(version) = info.current_version {
                println!("   Current version: {}", version);
            }
            println!("\n📜 Deploy history:");

            for deploy in info.deploys {
                let current_marker = if deploy.is_current { " (current)" } else { "" };
                println!(
                    "  v{} - {} files, {} bytes, {}{}",
                    deploy.version,
                    deploy.file_count,
                    deploy.total_size_bytes,
                    deploy.deployed_at,
                    current_marker
                );
            }
        }
        Commands::Rollback { project, version } => {
            let credentials = auth::load_credentials()?
                .ok_or_else(|| anyhow::anyhow!("Not logged in. Run 'statichub login' first."))?;

            let server_url = std::env::var("STATICHUB_SERVER")
                .unwrap_or_else(|_| "https://statichub.dev".to_string());

            println!("🔄 Rolling back {} to version {}...", project, version);

            let client = client::Client::new(server_url);
            let info = client
                .rollback_project(&project, version, &credentials.access_token)
                .await?;

            println!("✅ Rollback successful!");
            println!("   {} is now at version {}", info.name, info.current_version.unwrap_or(0));
            println!("   URL: {}", info.url);
        }
    }

    Ok(())
}
