use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "statichub-server")]
#[command(about = "StaticHub server - static site hosting platform", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },

    /// Database management commands
    Db {
        #[command(subcommand)]
        command: DbCommands,
    },
}

#[derive(Subcommand)]
pub enum DbCommands {
    /// Initialize a new database (create + migrate)
    Init,

    /// Run pending migrations
    Migrate,

    /// Show migration status
    Status,

    /// Reset database (WARNING: deletes all data)
    Reset {
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
}
