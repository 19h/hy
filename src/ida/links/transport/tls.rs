//! Session TLS contexts shared by direct requests and explicit proxy tunnels.

use std::path::Path;
use std::sync::Arc;

use rustls::pki_types::pem::PemObject;
use rustls::{ClientConfig, RootCertStore};

use crate::error::{Error, Result};

#[derive(Clone)]
pub(super) struct Contexts {
    pub(super) origin: Arc<ClientConfig>,
    pub(super) proxy: Arc<ClientConfig>,
}

impl Contexts {
    pub(super) fn from_environment() -> Result<Self> {
        let file = std::env::var_os("SSL_CERT_FILE").filter(|value| !value.is_empty());
        Self::from_ca_file(file.as_deref().map(Path::new))
    }

    fn from_ca_file(file: Option<&Path>) -> Result<Self> {
        let Some(file) = file else {
            return Ok(Self::default());
        };
        let roots = read_ca_file(file)?;
        let mut proxy_roots = default_roots();
        proxy_roots.roots.extend(roots.roots.iter().cloned());
        Ok(Self {
            // HTTPX replaces origin roots; HTTPcore adds its bundled roots to
            // the HTTPS proxy's default context. OS root loading is separate.
            origin: client(roots),
            proxy: client(proxy_roots),
        })
    }

    #[cfg(test)]
    pub(super) fn shared(config: Arc<ClientConfig>) -> Self {
        Self {
            origin: config.clone(),
            proxy: config,
        }
    }
}

impl Default for Contexts {
    fn default() -> Self {
        let config = client(default_roots());
        Self {
            origin: config.clone(),
            proxy: config,
        }
    }
}

fn default_roots() -> RootCertStore {
    RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    }
}

fn client(roots: RootCertStore) -> Arc<ClientConfig> {
    let mut config = ClientConfig::builder().with_root_certificates(roots).with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Arc::new(config)
}

fn read_ca_file(file: &Path) -> Result<RootCertStore> {
    let mut roots = RootCertStore::empty();
    let certificates = rustls::pki_types::CertificateDer::pem_file_iter(file)
        .map_err(|error| Error::Other(format!("KE CA file: {error}")))?;
    for certificate in certificates {
        let certificate =
            certificate.map_err(|error| Error::Other(format!("KE CA certificate: {error}")))?;
        roots
            .add(certificate)
            .map_err(|error| Error::Other(format!("KE CA certificate: {error}")))?;
    }
    if roots.is_empty() {
        return Err(Error::Other("KE CA file contains no certificates".into()));
    }
    Ok(roots)
}

#[cfg(test)]
mod tests;
