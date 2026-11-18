use crate::error::ServerError;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// Loads and validates a PEM certificate chain + private key using `rustls` ring provider.
pub fn load_certified_key(
    cert_path: &Path,
    key_path: &Path,
) -> Result<Arc<CertifiedKey>, ServerError> {
    let cert_file = File::open(cert_path).map_err(|err| {
        ServerError::Tls(format!(
            "cannot open certificate file {}: {err}",
            cert_path.display()
        ))
    })?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| {
            ServerError::Tls(format!(
                "invalid PEM certificate chain {}: {err}",
                cert_path.display()
            ))
        })?;
    if certs.is_empty() {
        return Err(ServerError::Tls(format!(
            "certificate file contains no certificates: {}",
            cert_path.display()
        )));
    }

    let key_file = File::open(key_path).map_err(|err| {
        ServerError::Tls(format!(
            "cannot open private key file {}: {err}",
            key_path.display()
        ))
    })?;
    let mut key_reader = BufReader::new(key_file);
    let private_key: PrivateKeyDer<'static> = rustls_pemfile::private_key(&mut key_reader)
        .map_err(|err| {
            ServerError::Tls(format!(
                "invalid PEM private key {}: {err}",
                key_path.display()
            ))
        })?
        .ok_or_else(|| {
            ServerError::Tls(format!(
                "private key file contains no supported key: {}",
                key_path.display()
            ))
        })?;

    let provider = rustls::crypto::ring::default_provider();
    let signing_key = provider
        .key_provider
        .load_private_key(private_key)
        .map_err(|err| {
            ServerError::Tls(format!(
                "failed to load signing key {}: {err}",
                key_path.display()
            ))
        })?;

    let certified_key = CertifiedKey::new(certs, signing_key);
    certified_key.keys_match().map_err(|err| {
        ServerError::Tls(format!(
            "certificate {} does not match key {}: {err}",
            cert_path.display(),
            key_path.display()
        ))
    })?;

    Ok(Arc::new(certified_key))
}

/// Thread-safe dynamic SNI resolver updated atomically on SIGHUP hot reloads.
#[derive(Debug)]
pub struct DynamicSniResolver {
    certs: RwLock<HashMap<String, Arc<CertifiedKey>>>,
}

impl DynamicSniResolver {
    #[must_use]
    pub fn new(initial: HashMap<String, Arc<CertifiedKey>>) -> Self {
        Self {
            certs: RwLock::new(initial),
        }
    }

    pub fn swap(&self, next: HashMap<String, Arc<CertifiedKey>>) -> Result<(), ServerError> {
        let mut guard = self
            .certs
            .write()
            .map_err(|_| ServerError::Tls("SNI resolver write lock poisoned".to_string()))?;
        *guard = next;
        Ok(())
    }
}

impl ResolvesServerCert for DynamicSniResolver {
    fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let sni = client_hello
            .server_name()?
            .trim()
            .trim_end_matches('.')
            .to_ascii_lowercase();
        let guard = self.certs.read().ok()?;
        guard.get(&sni).cloned()
    }
}

/// Builds `rustls::ServerConfig` with SNI resolution and ALPN (`h2`, `http/1.1`).
pub fn build_tls_server_config(
    resolver: Arc<DynamicSniResolver>,
) -> Result<Arc<rustls::ServerConfig>, ServerError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut cfg = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|err| ServerError::Tls(format!("TLS protocol version error: {err}")))?
        .with_no_client_auth()
        .with_cert_resolver(resolver);
    cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(Arc::new(cfg))
}
