use anyhow::{bail, Context, Result};
use base64::prelude::*;
use sha2::{Digest as Sha2Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;

use rustls_acme::acme::{
    Account, AuthStatus, Challenge, ChallengeType, Directory, Identifier, OrderStatus,
    LETS_ENCRYPT_PRODUCTION_DIRECTORY, LETS_ENCRYPT_STAGING_DIRECTORY,
};
use rustls_acme::caches::DirCache;
use rustls_acme::futures_rustls::pki_types::TrustAnchor;
use rustls_acme::futures_rustls::rustls::{ClientConfig, RootCertStore};
use rustls_acme::{AccountCache, CertCache};

use axum_server::tls_rustls::RustlsConfig;

use crate::tls::{AcmeDirectory, DnsSolver, TlsConfig};

/// DNS propagation wait time in seconds.
/// After setting a TXT record, we wait for DNS to propagate before
/// telling the ACME server to validate the challenge.
const DNS_PROPAGATION_WAIT_SECS: u64 = 120;

/// Maximum number of times to poll authorization status before giving up.
const MAX_AUTH_POLL_ATTEMPTS: u32 = 10;

/// Delay between authorization status polls in seconds.
const AUTH_POLL_DELAY_SECS: u64 = 5;

/// Maximum number of times to poll order processing status.
const MAX_ORDER_POLL_ATTEMPTS: u32 = 10;

/// Minimum days of certificate validity before requesting renewal.
const MIN_CERT_VALIDITY_DAYS: i64 = 30;

/// Background renewal check interval (12 hours).
const RENEWAL_CHECK_INTERVAL_HOURS: u64 = 12;

/// Retry interval after failed renewal (1 hour).
const RENEWAL_RETRY_INTERVAL_HOURS: u64 = 1;

/// Warning threshold: alert if certificate expires in < 7 days.
const CERT_EXPIRATION_WARNING_DAYS: i64 = 7;

pub struct CertificateManager {
    rustls_config: RustlsConfig,
}

impl CertificateManager {
    pub async fn new(
        config: TlsConfig,
        dns_solver: Arc<dyn DnsSolver>,
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

        // Try to load cached certificate first
        let cache = DirCache::new(config.cert_dir().clone());
        let cached_cert = cache
            .load_cert(config.domains(), directory_url)
            .await
            .ok()
            .flatten();

        let pem_data = if let Some(pem_data) = cached_cert {
            tracing::info!("Found cached certificate, validating...");

            if is_cert_valid(&pem_data, MIN_CERT_VALIDITY_DAYS) {
                tracing::info!("Cached certificate is still valid");
                pem_data
            } else {
                tracing::info!("Cached certificate is expired or expiring soon, requesting new one");
                acquire_certificate_dns01(
                    directory_url,
                    config.email(),
                    config.domains(),
                    &dns_solver,
                    &cache,
                )
                .await?
            }
        } else {
            tracing::info!("No cached certificate found, requesting new one via ACME DNS-01");
            acquire_certificate_dns01(
                directory_url,
                config.email(),
                config.domains(),
                &dns_solver,
                &cache,
            )
            .await?
        };

        // Parse PEM data into separate cert and key components
        let (key_pem, cert_pem) =
            split_pem(&pem_data).context("Failed to parse certificate PEM data")?;

        let rustls_config = RustlsConfig::from_pem(cert_pem, key_pem)
            .await
            .context("Failed to create RustlsConfig from certificate")?;

        // Spawn background renewal task
        spawn_renewal_task(
            directory_url.to_string(),
            config.email().to_string(),
            config.domains().to_vec(),
            config.cert_dir().clone(),
            dns_solver.clone(),
        );

        tracing::info!("Certificate manager initialized successfully");

        Ok(Self { rustls_config })
    }

    pub fn rustls_config(&self) -> RustlsConfig {
        self.rustls_config.clone()
    }
}

/// Spawn a background task that periodically checks certificate expiration
/// and renews certificates when necessary.
fn spawn_renewal_task(
    directory_url: String,
    email: String,
    domains: Vec<String>,
    cert_dir: PathBuf,
    dns_solver: Arc<dyn DnsSolver>,
) {
    tokio::spawn(async move {
        tracing::info!("Background certificate renewal task started");

        let cache = DirCache::new(cert_dir);
        let mut check_interval = std::time::Duration::from_secs(RENEWAL_CHECK_INTERVAL_HOURS * 3600);

        loop {
            tokio::time::sleep(check_interval).await;

            tracing::debug!("Checking certificate expiration...");

            // Load cached certificate
            let cached_cert = match cache.load_cert(&domains, &directory_url).await {
                Ok(Some(pem_data)) => pem_data,
                Ok(None) => {
                    tracing::warn!("No cached certificate found during renewal check");
                    check_interval = std::time::Duration::from_secs(RENEWAL_RETRY_INTERVAL_HOURS * 3600);
                    continue;
                }
                Err(e) => {
                    tracing::error!("Failed to load certificate during renewal check: {:?}", e);
                    check_interval = std::time::Duration::from_secs(RENEWAL_RETRY_INTERVAL_HOURS * 3600);
                    continue;
                }
            };

            // Check days until expiration
            let days_remaining = match get_days_until_expiration(&cached_cert) {
                Some(days) => days,
                None => {
                    tracing::error!("Failed to parse certificate expiration date");
                    check_interval = std::time::Duration::from_secs(RENEWAL_RETRY_INTERVAL_HOURS * 3600);
                    continue;
                }
            };

            // Log warning if certificate expires soon
            if days_remaining < CERT_EXPIRATION_WARNING_DAYS {
                tracing::warn!(
                    "Certificate will expire in {} days! Attempting renewal...",
                    days_remaining
                );
            } else if days_remaining < MIN_CERT_VALIDITY_DAYS {
                tracing::info!(
                    "Certificate expires in {} days, triggering renewal (threshold: {} days)",
                    days_remaining,
                    MIN_CERT_VALIDITY_DAYS
                );
            } else {
                tracing::debug!(
                    "Certificate is valid for {} more days, no renewal needed",
                    days_remaining
                );
                // Reset to normal check interval
                check_interval = std::time::Duration::from_secs(RENEWAL_CHECK_INTERVAL_HOURS * 3600);
                continue;
            }

            // Attempt renewal
            match acquire_certificate_dns01(
                &directory_url,
                &email,
                &domains,
                &dns_solver,
                &cache,
            )
            .await
            {
                Ok(_) => {
                    tracing::info!("Certificate renewed successfully in background task");
                    tracing::warn!(
                        "New certificate is cached, but server restart required to load it"
                    );
                    // Reset to normal check interval after successful renewal
                    check_interval = std::time::Duration::from_secs(RENEWAL_CHECK_INTERVAL_HOURS * 3600);
                }
                Err(e) => {
                    tracing::error!("Failed to renew certificate: {:?}", e);
                    tracing::info!("Continuing with existing certificate, will retry in {} hour(s)", RENEWAL_RETRY_INTERVAL_HOURS);
                    // Set retry interval
                    check_interval = std::time::Duration::from_secs(RENEWAL_RETRY_INTERVAL_HOURS * 3600);
                }
            }
        }
    });
}

/// Build a TLS client config for making ACME API requests.
fn build_acme_client_config() -> Result<Arc<ClientConfig>> {
    let mut root_store = RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
        let ta = ta.to_owned();
        TrustAnchor {
            subject: ta.subject.into(),
            subject_public_key_info: ta.subject_public_key_info.into(),
            name_constraints: ta.name_constraints.map(Into::into),
        }
    }));

    let client_config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Ok(Arc::new(client_config))
}

/// Acquire a certificate via ACME DNS-01 challenge flow.
///
/// This uses rustls-acme's public ACME protocol types (Account, Directory, Order)
/// with a custom DNS-01 challenge handler that uses our DnsSolver trait.
///
/// Note: rustls-acme natively only supports tls-alpn-01 challenges. This function
/// implements the DNS-01 flow using the library's low-level ACME primitives.
async fn acquire_certificate_dns01(
    directory_url: &str,
    email: &str,
    domains: &[String],
    dns_solver: &Arc<dyn DnsSolver>,
    cache: &DirCache<std::path::PathBuf>,
) -> Result<Vec<u8>> {
    let client_config = build_acme_client_config()?;
    let contact: Vec<String> = vec![format!("mailto:{}", email)];

    // Step 1: Discover ACME directory
    tracing::info!("Discovering ACME directory...");
    let directory = Directory::discover(&client_config, directory_url)
        .await
        .context("Failed to discover ACME directory")?;

    // Step 2: Create or load ACME account
    tracing::info!("Creating/loading ACME account...");
    let account_key = match cache
        .load_account(&contact, directory_url)
        .await
        .ok()
        .flatten()
    {
        Some(key) => {
            tracing::info!("Loaded cached account key");
            key
        }
        None => {
            let key = Account::generate_key_pair();
            tracing::info!("Generated new account key");
            if let Err(e) = cache.store_account(&contact, directory_url, &key).await {
                tracing::warn!("Failed to cache account key: {:?}", e);
            }
            key
        }
    };

    let account = Account::create_with_keypair(&client_config, directory, &contact, &account_key)
        .await
        .context("Failed to create ACME account")?;

    // Step 3: Create new order
    tracing::info!("Creating certificate order for domains: {:?}", domains);
    let (order_url, mut order) = account
        .new_order(&client_config, domains.to_vec())
        .await
        .context("Failed to create ACME order")?;

    // Generate key pair for the certificate (we need this before finalization)
    let cert_key_pair = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)
        .context("Failed to generate certificate key pair")?;

    // Step 4: Process order through its lifecycle
    loop {
        match order.status {
            OrderStatus::Pending => {
                tracing::info!("Order is pending, processing authorizations...");

                for auth_url in &order.authorizations.clone() {
                    authorize_dns01(
                        &client_config,
                        &account,
                        auth_url,
                        dns_solver,
                        &account_key,
                    )
                    .await?;
                }

                tracing::info!("All authorizations completed");
                order = account
                    .order(&client_config, &order_url)
                    .await
                    .context("Failed to fetch order status")?;
            }
            OrderStatus::Ready => {
                tracing::info!("Order is ready, finalizing with CSR...");

                let mut params = rcgen::CertificateParams::new(domains.to_vec())
                    .context("Failed to create certificate params")?;
                params.distinguished_name = rcgen::DistinguishedName::new();
                let csr = params
                    .serialize_request(&cert_key_pair)
                    .context("Failed to serialize CSR")?;

                order = account
                    .finalize(&client_config, &order.finalize, csr.der())
                    .await
                    .context("Failed to finalize order")?;
            }
            OrderStatus::Processing => {
                tracing::info!("Order is processing, waiting...");
                for i in 0..MAX_ORDER_POLL_ATTEMPTS {
                    tokio::time::sleep(std::time::Duration::from_secs(1u64 << i.min(4))).await;
                    order = account
                        .order(&client_config, &order_url)
                        .await
                        .context("Failed to fetch order status")?;
                    if order.status != OrderStatus::Processing {
                        break;
                    }
                }
                if order.status == OrderStatus::Processing {
                    bail!("Order processing timed out");
                }
            }
            OrderStatus::Valid { certificate } => {
                tracing::info!("Downloading certificate...");
                let cert_chain_pem = account
                    .certificate(&client_config, &certificate)
                    .await
                    .context("Failed to download certificate")?;

                // Combine: private key PEM + certificate chain PEM
                let pem_data =
                    format!("{}\n{}", cert_key_pair.serialize_pem(), cert_chain_pem).into_bytes();

                // Cache the certificate
                if let Err(e) = cache.store_cert(domains, directory_url, &pem_data).await {
                    tracing::warn!("Failed to cache certificate: {:?}", e);
                }

                tracing::info!("Certificate acquired successfully");
                return Ok(pem_data);
            }
            OrderStatus::Invalid => {
                let error_detail = order
                    .error
                    .as_ref()
                    .map(|e| {
                        format!(
                            "type={}, detail={}",
                            e.typ.as_deref().unwrap_or("unknown"),
                            e.detail.as_deref().unwrap_or("no detail")
                        )
                    })
                    .unwrap_or_else(|| "no error details".to_string());
                bail!("ACME order is invalid: {}", error_detail);
            }
        }
    }
}

/// Complete DNS-01 authorization for a single domain.
async fn authorize_dns01(
    client_config: &Arc<ClientConfig>,
    account: &Account,
    auth_url: &str,
    dns_solver: &Arc<dyn DnsSolver>,
    account_key_pkcs8: &[u8],
) -> Result<()> {
    let auth = account
        .auth(client_config, auth_url)
        .await
        .context("Failed to fetch authorization")?;

    match auth.status {
        AuthStatus::Valid => {
            tracing::info!("Authorization already valid");
            return Ok(());
        }
        AuthStatus::Pending => {}
        _ => {
            bail!("Unexpected authorization status: {:?}", auth.status);
        }
    }

    let Identifier::Dns(ref domain) = auth.identifier;
    tracing::info!("Processing DNS-01 challenge for domain: {}", domain);

    // Find DNS-01 challenge
    let challenge = find_dns01_challenge(&auth.challenges)
        .context(format!("No DNS-01 challenge found for domain: {}", domain))?;

    // Compute the DNS-01 challenge response value
    let dns_value = compute_dns01_value(account_key_pkcs8, &challenge.token)
        .context("Failed to compute DNS-01 challenge value")?;

    // Set DNS TXT record
    tracing::info!("Setting DNS TXT record for _acme-challenge.{}", domain);
    dns_solver
        .set_txt_record(domain, &dns_value)
        .await
        .context(format!("Failed to set DNS TXT record for {}", domain))?;

    // Wait for DNS propagation
    tracing::info!(
        "Waiting {}s for DNS propagation...",
        DNS_PROPAGATION_WAIT_SECS
    );
    tokio::time::sleep(std::time::Duration::from_secs(DNS_PROPAGATION_WAIT_SECS)).await;

    // Tell ACME server to validate the challenge
    tracing::info!("Requesting ACME validation for {}", domain);
    account
        .challenge(client_config, &challenge.url)
        .await
        .context("Failed to submit challenge for validation")?;

    // Poll until authorization is valid or fails
    for attempt in 1..=MAX_AUTH_POLL_ATTEMPTS {
        tokio::time::sleep(std::time::Duration::from_secs(AUTH_POLL_DELAY_SECS)).await;

        let auth = account
            .auth(client_config, auth_url)
            .await
            .context("Failed to poll authorization status")?;

        match auth.status {
            AuthStatus::Valid => {
                tracing::info!("Authorization for {} is valid", domain);
                // Clean up DNS record (best-effort)
                if let Err(e) = dns_solver.delete_txt_record(domain, &dns_value).await {
                    tracing::warn!("Failed to clean up DNS TXT record for {}: {:?}", domain, e);
                }
                return Ok(());
            }
            AuthStatus::Pending => {
                tracing::info!(
                    "Authorization for {} still pending (attempt {}/{})",
                    domain,
                    attempt,
                    MAX_AUTH_POLL_ATTEMPTS
                );
                // Re-trigger challenge validation
                if let Err(e) = account.challenge(client_config, &challenge.url).await {
                    tracing::warn!("Failed to re-trigger challenge: {:?}", e);
                }
            }
            AuthStatus::Invalid => {
                let _ = dns_solver.delete_txt_record(domain, &dns_value).await;
                bail!("Authorization for {} failed (invalid)", domain);
            }
            _ => {
                let _ = dns_solver.delete_txt_record(domain, &dns_value).await;
                bail!(
                    "Authorization for {} has unexpected status: {:?}",
                    domain,
                    auth.status
                );
            }
        }
    }

    let _ = dns_solver.delete_txt_record(domain, &dns_value).await;
    bail!(
        "Authorization for {} timed out after {} attempts",
        domain,
        MAX_AUTH_POLL_ATTEMPTS
    );
}

/// Find the DNS-01 challenge from a list of challenges.
fn find_dns01_challenge(challenges: &[Challenge]) -> Option<&Challenge> {
    challenges.iter().find(|c| c.typ == ChallengeType::Dns01)
}

/// Compute the DNS-01 challenge response value from the account's PKCS8 key and the challenge token.
///
/// For DNS-01, the TXT record value is:
///   base64url(SHA256(key_authorization))
///
/// where:
///   key_authorization = token + "." + JWK_Thumbprint
///   JWK_Thumbprint = base64url(SHA256(canonical_jwk_json))
///   canonical_jwk_json = {"crv":"P-256","kty":"EC","x":"<base64url>","y":"<base64url>"}
///
/// The x and y values are extracted from the EC public key in the PKCS8 structure.
fn compute_dns01_value(account_key_pkcs8: &[u8], token: &str) -> Result<String> {
    // Extract the EC public key point from the PKCS8 structure.
    // For ECDSA P-256, the PKCS8 structure contains the private key which includes
    // the public key as an uncompressed point (0x04 || x[32] || y[32]).
    let (x, y) = extract_ec_pubkey_from_pkcs8(account_key_pkcs8)
        .context("Failed to extract public key from account key")?;

    // Compute JWK Thumbprint (RFC 7638)
    // Canonical JSON with members in lexicographic order
    let jwk_json = format!(
        r#"{{"crv":"P-256","kty":"EC","x":"{}","y":"{}"}}"#,
        BASE64_URL_SAFE_NO_PAD.encode(&x),
        BASE64_URL_SAFE_NO_PAD.encode(&y)
    );

    let thumbprint = {
        let mut hasher = Sha256::new();
        hasher.update(jwk_json.as_bytes());
        BASE64_URL_SAFE_NO_PAD.encode(hasher.finalize())
    };

    // Compute key authorization
    let key_authorization = format!("{}.{}", token, thumbprint);

    // DNS-01 value = base64url(SHA256(key_authorization))
    let mut hasher = Sha256::new();
    hasher.update(key_authorization.as_bytes());
    let dns_value = BASE64_URL_SAFE_NO_PAD.encode(hasher.finalize());

    Ok(dns_value)
}

/// Extract the EC P-256 public key (x, y) coordinates from a PKCS8-encoded key.
///
/// The PKCS8 structure for ECDSA P-256 contains the public key as a BIT STRING
/// with the uncompressed point format: 0x04 || x[32] || y[32].
/// We search for this 65-byte uncompressed point in the DER structure.
fn extract_ec_pubkey_from_pkcs8(pkcs8: &[u8]) -> Result<([u8; 32], [u8; 32])> {
    // The PKCS8 structure for EC keys embeds the public key as a BIT STRING.
    // For P-256, we look for the 65-byte uncompressed point (0x04 prefix).
    // The public key appears near the end of the PKCS8 structure.
    //
    // Rather than implementing a full ASN.1 parser, we scan for the pattern:
    // BIT STRING tag (0x03) + length (0x42 = 66) + unused bits (0x00) + 0x04 + 64 bytes
    for i in 0..pkcs8.len().saturating_sub(67) {
        if pkcs8[i] == 0x03
            && pkcs8[i + 1] == 0x42
            && pkcs8[i + 2] == 0x00
            && pkcs8[i + 3] == 0x04
        {
            let point_start = i + 4;
            if point_start + 64 <= pkcs8.len() {
                let mut x = [0u8; 32];
                let mut y = [0u8; 32];
                x.copy_from_slice(&pkcs8[point_start..point_start + 32]);
                y.copy_from_slice(&pkcs8[point_start + 32..point_start + 64]);
                return Ok((x, y));
            }
        }
    }

    bail!("Could not find EC public key point in PKCS8 structure")
}

/// Check if a PEM-encoded certificate is still valid for at least `min_days` days.
fn is_cert_valid(pem_data: &[u8], min_days: i64) -> bool {
    let pems = match pem::parse_many(pem_data) {
        Ok(pems) => pems,
        Err(_) => return false,
    };

    for p in &pems {
        if p.tag() == "CERTIFICATE" {
            match x509_parser::parse_x509_certificate(p.contents()) {
                Ok((_, cert)) => {
                    let not_after = cert.validity().not_after.timestamp();
                    let now = chrono::Utc::now().timestamp();
                    let remaining_days = (not_after - now) / 86400;
                    return remaining_days >= min_days;
                }
                Err(_) => return false,
            }
        }
    }

    false
}

/// Get the number of days until a PEM-encoded certificate expires.
/// Returns None if the certificate cannot be parsed.
fn get_days_until_expiration(pem_data: &[u8]) -> Option<i64> {
    let pems = pem::parse_many(pem_data).ok()?;

    for p in &pems {
        if p.tag() == "CERTIFICATE" {
            if let Ok((_, cert)) = x509_parser::parse_x509_certificate(p.contents()) {
                let not_after = cert.validity().not_after.timestamp();
                let now = chrono::Utc::now().timestamp();
                let remaining_days = (not_after - now) / 86400;
                return Some(remaining_days);
            }
        }
    }

    None
}

/// Split combined PEM data (key + certs) into separate key and cert components.
fn split_pem(pem_data: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    let pems = pem::parse_many(pem_data).context("Failed to parse PEM data")?;

    let mut key_pems = Vec::new();
    let mut cert_pems = Vec::new();

    for p in pems {
        let tag = p.tag().to_string();
        let encoded = pem::encode(&p);
        if tag.contains("PRIVATE KEY") {
            key_pems.push(encoded);
        } else {
            cert_pems.push(encoded);
        }
    }

    if key_pems.is_empty() {
        bail!("No private key found in PEM data");
    }
    if cert_pems.is_empty() {
        bail!("No certificates found in PEM data");
    }

    Ok((
        key_pems.join("\n").into_bytes(),
        cert_pems.join("\n").into_bytes(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_dns01_value_from_known_key() {
        // Generate a test key pair and verify the computation doesn't panic
        let key = Account::generate_key_pair();
        let result = compute_dns01_value(&key, "test-token-123");
        assert!(result.is_ok());

        let value = result.unwrap();
        // DNS-01 value should be base64url-encoded SHA256 (43 chars without padding)
        assert_eq!(value.len(), 43);
        // Should only contain base64url characters
        assert!(value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn test_extract_ec_pubkey_from_pkcs8() {
        let key = Account::generate_key_pair();
        let result = extract_ec_pubkey_from_pkcs8(&key);
        assert!(result.is_ok());

        let (x, y) = result.unwrap();
        // x and y should not be all zeros
        assert!(x.iter().any(|&b| b != 0));
        assert!(y.iter().any(|&b| b != 0));
    }

    #[test]
    fn test_find_dns01_challenge() {
        // Empty challenges
        let challenges: Vec<Challenge> = vec![];
        assert!(find_dns01_challenge(&challenges).is_none());
    }

    #[test]
    fn test_split_pem_valid() {
        let pem_data = b"-----BEGIN PRIVATE KEY-----\nMIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQg\n-----END PRIVATE KEY-----\n-----BEGIN CERTIFICATE-----\nMIIBdjCCAR2gAwIBAgIUGJRx\n-----END CERTIFICATE-----\n";
        let result = split_pem(pem_data);
        assert!(result.is_ok());
        let (key, cert) = result.unwrap();
        assert!(String::from_utf8_lossy(&key).contains("PRIVATE KEY"));
        assert!(String::from_utf8_lossy(&cert).contains("CERTIFICATE"));
    }

    #[test]
    fn test_split_pem_no_key() {
        let pem_data = b"-----BEGIN CERTIFICATE-----\nMIIBdjCCAR2gAwIBAgIUGJRx\n-----END CERTIFICATE-----\n";
        let result = split_pem(pem_data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No private key"));
    }

    #[test]
    fn test_split_pem_no_cert() {
        let pem_data = b"-----BEGIN PRIVATE KEY-----\nMIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQg\n-----END PRIVATE KEY-----\n";
        let result = split_pem(pem_data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No certificates"));
    }

    #[test]
    fn test_is_cert_valid_invalid_data() {
        assert!(!is_cert_valid(b"not pem data", 30));
        assert!(!is_cert_valid(b"", 30));
    }

    #[test]
    fn test_get_days_until_expiration_invalid_data() {
        assert!(get_days_until_expiration(b"not pem data").is_none());
        assert!(get_days_until_expiration(b"").is_none());
    }
}
