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

    /// Manage custom domains
    Domain {
        #[command(subcommand)]
        command: DomainCommands,
    },
}

#[derive(Subcommand)]
enum DomainCommands {
    /// Add a custom domain to a project
    Add {
        /// Project name
        project: String,
        /// Domain name (e.g., example.com)
        domain: String,
    },

    /// List domains for a project
    List {
        /// Project name
        project: String,
    },

    /// Verify DNS configuration for a domain
    Verify {
        /// Project name
        project: String,
        /// Domain name
        domain: String,
    },

    /// Remove a custom domain
    Remove {
        /// Project name
        project: String,
        /// Domain name
        domain: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
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
                .unwrap_or_else(|_| "http://localhost:3000".to_string());

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
        Commands::List => {
            let credentials = auth::load_credentials()?
                .ok_or_else(|| anyhow::anyhow!("Not logged in. Run 'statichub login' first."))?;

            let server_url = std::env::var("STATICHUB_SERVER")
                .unwrap_or_else(|_| "http://localhost:3000".to_string());

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
                .unwrap_or_else(|_| "http://localhost:3000".to_string());

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
                .unwrap_or_else(|_| "http://localhost:3000".to_string());

            println!("🔄 Rolling back {} to version {}...", project, version);

            let client = client::Client::new(server_url);
            let info = client
                .rollback_project(&project, version, &credentials.access_token)
                .await?;

            println!("✅ Rollback successful!");
            println!("   {} is now at version {}", info.name, info.current_version.unwrap_or(0));
            println!("   URL: {}", info.url);
        }
        Commands::Domain { command } => {
            let credentials = auth::load_credentials()?
                .ok_or_else(|| anyhow::anyhow!("Not logged in. Run 'statichub login' first."))?;

            let server_url = std::env::var("STATICHUB_SERVER")
                .unwrap_or_else(|_| "http://localhost:3000".to_string());

            let client = client::Client::new(server_url);

            match command {
                DomainCommands::Add { project, domain } => {
                    println!("🌐 Adding domain {} to {}...", domain, project);

                    let response = client
                        .add_domain(&project, &domain, &credentials.access_token)
                        .await?;

                    println!("✅ Domain added successfully!");
                    println!("   Domain: {}", response.domain);
                    println!("   Project: {}", response.project_name);
                    println!("   DNS Target: {}", response.dns_target);
                    println!("\n📋 Next steps:");
                    println!("   1. Add a CNAME record pointing {} to {}", response.domain, response.dns_target);
                    println!("   2. Run: statichub domain verify {} {}", project, domain);
                }
                DomainCommands::List { project } => {
                    let domains = client
                        .list_domains(&project, &credentials.access_token)
                        .await?;

                    if domains.is_empty() {
                        println!("📭 No custom domains configured");
                        println!("   Add one with: statichub domain add {} yourdomain.com", project);
                    } else {
                        println!("🌐 Custom domains for {}:\n", project);
                        for domain in domains {
                            let status = if domain.verified { "✅ Verified" } else { "⏳ Pending" };
                            println!("  {} - {}", domain.domain, status);
                            println!("    DNS Target: {}", domain.dns_target);
                            println!("    Added: {}", domain.created_at);
                            if let Some(verified_at) = domain.verified_at {
                                println!("    Verified: {}", verified_at);
                            }
                            println!();
                        }
                    }
                }
                DomainCommands::Verify { project, domain } => {
                    println!("🔍 Verifying DNS for {}...", domain);

                    let response = client
                        .verify_domain(&project, &domain, &credentials.access_token)
                        .await?;

                    if response.verified {
                        println!("✅ Domain verified successfully!");
                        println!("   Your site is now accessible at https://{}", response.domain);
                    } else {
                        println!("❌ Domain verification failed");
                        println!("   Make sure you have a CNAME record pointing {} to {}", response.domain, response.dns_target);
                        println!("   DNS changes can take up to 48 hours to propagate");
                    }
                }
                DomainCommands::Remove { project, domain } => {
                    println!("🗑️  Removing domain {} from {}...", domain, project);

                    client
                        .remove_domain(&project, &domain, &credentials.access_token)
                        .await?;

                    println!("✅ Domain removed successfully!");
                }
            }
        }
    }

    Ok(())
}
