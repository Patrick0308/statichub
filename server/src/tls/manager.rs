use anyhow::{Result, Context};
use std::sync::Arc;
#[allow(unused_imports)] // Will be used in Task 9 for ACME integration
use rustls_acme::{AcmeConfig, caches::DirCache};
use rustls_acme::acme::{LETS_ENCRYPT_STAGING_DIRECTORY, LETS_ENCRYPT_PRODUCTION_DIRECTORY};
use axum_server::tls_rustls::RustlsConfig;
use crate::tls::{TlsConfig, DnsSolver, AcmeDirectory};

pub struct CertificateManager {
    rustls_config: RustlsConfig,
}

impl CertificateManager {
    pub async fn new(
        config: TlsConfig,
        _dns_solver: Arc<dyn DnsSolver>,  // Will be used in Task 9 for DNS challenges
    ) -> Result<Self> {
        tracing::info!("Initializing certificate manager");
        tracing::info!("  ACME directory: {:?}", config.acme_directory());
        tracing::info!("  Contact email: {}", config.email());
        tracing::info!("  Certificate directory: {:?}", config.cert_dir());
        tracing::info!("  Domains: {:?}", config.domains());

        // Create certificate directory
        std::fs::create_dir_all(config.cert_dir())
            .context("Failed to create certificate directory")?;

        // Determine ACME directory URL
        let directory_url = match config.acme_directory() {
            AcmeDirectory::Staging => LETS_ENCRYPT_STAGING_DIRECTORY,
            AcmeDirectory::Production => LETS_ENCRYPT_PRODUCTION_DIRECTORY,
        };

        tracing::info!("  Directory URL: {}", directory_url);

        // This is a placeholder - full implementation in next task
        // For now, create a simple RustlsConfig without ACME
        let rustls_config = RustlsConfig::from_pem_file(
            config.cert_dir().join("cert.pem"),
            config.cert_dir().join("key.pem"),
        )
        .await
        .context("Failed to load certificate (placeholder)")?;

        Ok(Self { rustls_config })
    }

    pub fn rustls_config(&self) -> RustlsConfig {
        self.rustls_config.clone()
    }
}
