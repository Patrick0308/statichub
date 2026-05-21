mod config;
mod auth;
mod upload;

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
            println!("Deploy command - not yet implemented");
            println!("  Directory: {:?}", directory);
            println!("  Name: {:?}", name);
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
