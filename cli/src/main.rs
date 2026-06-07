mod config;
mod auth;
mod upload;
mod client;

use anyhow::Context;
use clap::{CommandFactory, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "statichub")]
#[command(about = "Static web publishing for frontend developers", long_about = None)]
struct Cli {
    /// Use local server URL (default: http://localhost:3000, optional custom port)
    #[arg(long, num_args = 0..=1, default_missing_value = "3000", global = true)]
    local: Option<u16>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Print version
    Version,

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

    /// Manage API keys (requires login)
    Apikey {
        #[command(subcommand)]
        command: ApiKeyCommands,
    },
}

#[derive(Subcommand)]
enum ApiKeyCommands {
    /// Create a new API key
    Create {
        /// Human-readable name for this key
        name: String,
    },
    /// List your API keys
    List,
    /// Revoke an API key by id
    Revoke {
        /// API key id
        id: i64,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let Some(command) = cli.command else {
        Cli::command().print_help()?;
        println!();
        return Ok(());
    };

    match command {
        Commands::Version => {
            println!("{}", env!("CARGO_PKG_VERSION"));
        }
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

            let server_url = resolve_server_url(cli.local);

            let client = client::Client::new(server_url.clone());

            let response = if let Some(project_name) = name {
                // Authenticated deploy
                let token = resolve_project_auth_token()?;

                println!("🚀 Deploying to project '{}' on {}...", project_name, server_url);
                client.deploy_authenticated(&project_name, &files, &token, config.as_ref()).await?
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
            let server_url = resolve_server_url(cli.local);
            let client = client::Client::new(server_url);

            println!("Logging in to StaticHub...");

            let login = client.create_device_session().await?;

            println!();
            println!("Open this URL:");
            println!("  {}", login.verification_uri);
            println!();
            println!("Enter this code:");
            println!("  {}", login.user_code);
            println!();
            println!("Or open:");
            println!("  {}", login.verification_uri_complete);
            println!();

            if let Err(e) = open::that(&login.verification_uri_complete) {
                println!("Could not open browser automatically: {}", e);
                println!("Please open the URL manually in your browser.");
                println!();
            }

            println!("Waiting for authentication...");
            let started_at = std::time::Instant::now();
            let expires_after = std::time::Duration::from_secs(login.expires_in.max(1) as u64);
            let mut interval = std::time::Duration::from_secs(login.interval.max(1) as u64);

            let token = loop {
                if started_at.elapsed() >= expires_after {
                    anyhow::bail!("Authentication timed out. Please run 'statichub login' again.");
                }

                tokio::time::sleep(interval).await;
                let status = client.poll_device_token(&login.device_code).await?;

                match status.status.as_str() {
                    "approved" => {
                        let token = status
                            .token
                            .ok_or_else(|| anyhow::anyhow!("Login was approved but no token was returned"))?;
                        break token;
                    }
                    "authorization_pending" => {
                        if let Some(next_interval) = status.interval {
                            interval = std::time::Duration::from_secs(next_interval.max(1) as u64);
                        }
                    }
                    "slow_down" => {
                        let next_interval = status
                            .interval
                            .unwrap_or_else(|| interval.as_secs() as i64 + 5);
                        interval = std::time::Duration::from_secs(next_interval.max(1) as u64);
                    }
                    "expired_token" => {
                        anyhow::bail!("Authentication code expired. Please run 'statichub login' again.");
                    }
                    "access_denied" => {
                        anyhow::bail!("Authentication was denied.");
                    }
                    other => {
                        anyhow::bail!("Unexpected login status from server: {}", other);
                    }
                }
            };

            auth::save_credentials(&token)?;

            println!("Login successful!");
            println!("Credentials saved to ~/.statichub/credentials.json");
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
            let token = resolve_project_auth_token()?;

            let server_url = resolve_server_url(cli.local);

            let client = client::Client::new(server_url);
            let projects = client.list_projects(&token).await?;

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
            let token = resolve_project_auth_token()?;

            let server_url = resolve_server_url(cli.local);

            let client = client::Client::new(server_url);
            let info = client.get_project_info(&project, &token).await?;

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
            let token = resolve_project_auth_token()?;

            let server_url = resolve_server_url(cli.local);

            println!("🔄 Rolling back {} to version {}...", project, version);

            let client = client::Client::new(server_url);
            let info = client
                .rollback_project(&project, version, &token)
                .await?;

            println!("✅ Rollback successful!");
            println!("   {} is now at version {}", info.name, info.current_version.unwrap_or(0));
            println!("   URL: {}", info.url);
        }
        Commands::Apikey { command } => {
            let jwt = require_login_jwt()?;
            let server_url = resolve_server_url(cli.local);
            let client = client::Client::new(server_url);

            match command {
                ApiKeyCommands::Create { name } => {
                    let created = client.create_api_key(&jwt, &name).await?;
                    println!("✅ API key created!");
                    println!("   ID: {}", created.id);
                    println!("   Name: {}", created.name);
                    println!("   Prefix: {}", created.prefix);
                    println!("   Key (shown once): {}", created.api_key);
                    println!("   Export it: export STATICHUB_API_KEY='{}'", created.api_key);
                }
                ApiKeyCommands::List => {
                    let keys = client.list_api_keys(&jwt).await?;
                    if keys.is_empty() {
                        println!("📭 No API keys");
                    } else {
                        println!("🔑 API keys:\n");
                        for key in keys {
                            let status = if key.revoked { "revoked" } else { "active" };
                            println!("  {} - {} ({})", key.id, key.name, status);
                            println!("    Prefix: {}", key.prefix);
                            println!("    Created: {}", key.created_at);
                            if let Some(last_used_at) = key.last_used_at {
                                println!("    Last used: {}", last_used_at);
                            }
                            println!();
                        }
                    }
                }
                ApiKeyCommands::Revoke { id } => {
                    client.revoke_api_key(&jwt, id).await?;
                    println!("✅ API key revoked: {}", id);
                }
            }
        }
    }

    Ok(())
}

fn resolve_server_url(local_port: Option<u16>) -> String {
    let env_server = std::env::var("STATICHUB_SERVER").ok();
    resolve_server_url_from(local_port, env_server.as_deref())
}

fn resolve_server_url_from(local_port: Option<u16>, env_server: Option<&str>) -> String {
    if let Some(port) = local_port {
        return format!("http://localhost:{}", port);
    }

    env_server
        .map(str::to_string)
        .unwrap_or_else(|| "https://statichub.dev".to_string())
}

fn resolve_project_auth_token() -> anyhow::Result<String> {
    if let Ok(key) = std::env::var("STATICHUB_API_KEY") {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    let credentials = auth::load_credentials()?
        .ok_or_else(|| anyhow::anyhow!(
            "Not authenticated. Run 'statichub login' or set STATICHUB_API_KEY."
        ))?;
    Ok(credentials.access_token)
}

fn require_login_jwt() -> anyhow::Result<String> {
    let credentials = auth::load_credentials()?
        .ok_or_else(|| anyhow::anyhow!("Not logged in. Run 'statichub login' first."))?;
    Ok(credentials.access_token)
}

#[cfg(test)]
mod tests {
    use super::resolve_server_url_from;

    #[test]
    fn resolve_server_url_prefers_local_flag() {
        let url = resolve_server_url_from(Some(3000), Some("https://example.com"));
        assert_eq!(url, "http://localhost:3000");
    }

    #[test]
    fn resolve_server_url_uses_env_when_no_local_flag() {
        let url = resolve_server_url_from(None, Some("https://example.com"));
        assert_eq!(url, "https://example.com");
    }

    #[test]
    fn resolve_server_url_uses_default_when_no_local_or_env() {
        let url = resolve_server_url_from(None, None);
        assert_eq!(url, "https://statichub.dev");
    }
}
