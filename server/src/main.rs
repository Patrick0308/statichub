use statichub_server::{db, storage, api, create_router, cli, config::ServerConfig};
use clap::Parser;
use std::{net::SocketAddr, sync::Arc, io::Write};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    dotenv::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "statichub_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = cli::Cli::parse();

    match cli.command {
        Some(cli::Commands::Serve { port: _ }) => {
            serve().await?;
        }
        Some(cli::Commands::Db { command }) => {
            handle_db_command(command).await?;
        }
        None => {
            // Default: serve
            serve().await?;
        }
    }

    Ok(())
}

async fn serve() -> anyhow::Result<()> {
    let database_url = std::env::var("STATICHUB_DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:statichub.db".to_string());

    // Try to connect to database
    let pool = match db::create_pool(&database_url).await {
        Ok(pool) => pool,
        Err(e) => {
            eprintln!("\n❌ Failed to connect to database: {}", e);
            eprintln!("\n💡 Did you run migrations?");
            eprintln!("   Try: statichub-server db migrate\n");
            std::process::exit(1);
        }
    };

    // Check if migrations are up to date
    match db::migration_status(&database_url).await {
        Ok(migrations) => {
            let pending: Vec<_> = migrations.iter()
                .filter(|m| !m.applied)
                .collect();

            if !pending.is_empty() {
                eprintln!("\n⚠️  Warning: {} pending migration(s)", pending.len());
                for migration in pending {
                    eprintln!("   - {} ({})", migration.description, migration.version);
                }
                eprintln!("\n💡 Run migrations with: statichub-server db migrate\n");
                std::process::exit(1);
            }
        }
        Err(_) => {
            eprintln!("\n❌ Database exists but migration table not found");
            eprintln!("💡 Run: statichub-server db migrate\n");
            std::process::exit(1);
        }
    }

    tracing::info!("✓ Database connected and migrations up to date");

    // Load configuration
    let config = ServerConfig::from_env()?;
    tracing::info!("✓ Configuration loaded:");
    tracing::info!("  Port: {}", config.port);
    tracing::info!("  Allowed domains: {:?}", config.allowed_domains);

    // Storage setup
    let storage_path = std::env::var("STATICHUB_STORAGE_PATH")
        .unwrap_or_else(|_| "./var/statichub/deploys".to_string());

    let storage = Arc::new(storage::FilesystemStorage::new(storage_path.into())) as Arc<dyn storage::Storage>;

    // Shared state
    let deploy_state = Arc::new(api::DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
    });

    let auth_state = Arc::new(api::AuthState::new(
        pool.clone(),
        std::env::var("STATICHUB_GOOGLE_CLIENT_ID")
            .expect("STATICHUB_GOOGLE_CLIENT_ID must be set"),
        std::env::var("STATICHUB_GOOGLE_CLIENT_SECRET")
            .expect("STATICHUB_GOOGLE_CLIENT_SECRET must be set"),
        std::env::var("STATICHUB_GOOGLE_REDIRECT_URL")
            .unwrap_or_else(|_| "http://localhost:3000/auth/callback/google".to_string()),
        std::env::var("STATICHUB_JWT_SECRET")
            .expect("STATICHUB_JWT_SECRET must be set in production"),
    )?);

    // Build router
    let app = create_router(deploy_state, auth_state)
        .layer(axum::middleware::from_fn_with_state(
            config.clone(),
            statichub_server::middleware::host_validation_middleware,
        ));

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("🚀 Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn handle_db_command(command: cli::DbCommands) -> anyhow::Result<()> {
    let database_url = std::env::var("STATICHUB_DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:statichub.db".to_string());

    match command {
        cli::DbCommands::Init => {
            println!("Initializing new database...");
            println!("Database: {}\n", database_url);

            // Check if database already exists
            if let Ok(_) = db::create_pool(&database_url).await {
                eprintln!("❌ Database already exists!");
                eprintln!("💡 Use 'statichub-server db migrate' to update an existing database");
                eprintln!("   Or 'statichub-server db reset' to recreate it");
                std::process::exit(1);
            }

            // Run migrations (which will create the database)
            match db::migrate(&database_url).await {
                Ok(_) => {
                    println!("✓ Database created successfully");

                    // Show applied migrations
                    if let Ok(migrations) = db::migration_status(&database_url).await {
                        println!("\nInitialized with migrations:");
                        for migration in migrations.iter().filter(|m| m.applied) {
                            println!("  ✓ {} - {}", migration.version, migration.description);
                        }
                    }

                    println!("\n✓ Database initialization complete");
                    println!("💡 You can now start the server with: statichub-server serve");
                }
                Err(e) => {
                    eprintln!("❌ Initialization failed: {}", e);
                    std::process::exit(1);
                }
            }
        }

        cli::DbCommands::Migrate => {
            println!("Running database migrations...");

            match db::migrate(&database_url).await {
                Ok(_) => {
                    println!("✓ Migrations completed successfully");

                    // Show current status
                    if let Ok(migrations) = db::migration_status(&database_url).await {
                        println!("\nApplied migrations:");
                        for migration in migrations.iter().filter(|m| m.applied) {
                            println!("  ✓ {} - {}", migration.version, migration.description);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("❌ Migration failed: {}", e);
                    std::process::exit(1);
                }
            }
        }

        cli::DbCommands::Status => {
            match db::migration_status(&database_url).await {
                Ok(migrations) => {
                    println!("Database migration status:\n");

                    let applied: Vec<_> = migrations.iter().filter(|m| m.applied).collect();
                    let pending: Vec<_> = migrations.iter().filter(|m| !m.applied).collect();

                    if !applied.is_empty() {
                        println!("Applied migrations:");
                        for migration in applied {
                            println!("  ✓ {} - {}", migration.version, migration.description);
                        }
                    }

                    if !pending.is_empty() {
                        println!("\nPending migrations:");
                        for migration in pending {
                            println!("  ⏳ {} - {}", migration.version, migration.description);
                        }
                        println!("\n💡 Run: statichub-server db migrate");
                    } else {
                        println!("\n✓ All migrations up to date");
                    }
                }
                Err(e) => {
                    eprintln!("❌ Failed to check migration status: {}", e);
                    eprintln!("💡 Database might not exist. Run: statichub-server db migrate");
                    std::process::exit(1);
                }
            }
        }

        cli::DbCommands::Reset { force } => {
            if !force {
                print!("\n⚠️  WARNING: This will DELETE ALL DATA in the database!\n");
                print!("   Database: {}\n\n", database_url);
                print!("Type 'yes' to confirm: ");
                std::io::stdout().flush()?;

                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;

                if input.trim() != "yes" {
                    println!("Aborted.");
                    return Ok(());
                }
            }

            println!("Resetting database...");

            match db::reset(&database_url).await {
                Ok(_) => {
                    println!("✓ Database reset and migrations applied");
                }
                Err(e) => {
                    eprintln!("❌ Reset failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}
