use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use tokio_rustls::rustls::{
    self, RootCertStore, ServerConfig, pki_types::CertificateDer, server::WebPkiClientVerifier,
};

/// Errors that can occur while building a TLS config
#[derive(Debug, thiserror::Error)]
pub enum TlsError {
    #[error("failed to open file {0}")]
    CertFileIo(std::io::Error),
    #[error("failed to read certificate PEM")]
    CertPem(std::io::Error),
    #[error("failed to parse private key PEM")]
    KeyPem(std::io::Error),
    #[error("no private keys found in {0}")]
    NoPrivateKey(PathBuf),
    #[error("failed to add root CA certificate")]
    RootCertError(rustls::Error),
    #[error("failed to build server config")]
    ServerConfigError(rustls::Error),
    #[error("failed to build client config")]
    ClientConfigError(rustls::Error),
}

#[derive(Debug, Deserialize)]
pub struct TlsConfig {
    pub cert: PathBuf,
    pub key: PathBuf,
    pub root_ca: PathBuf,
    pub domain: String,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            cert: PathBuf::from("./keys/cert.pem"),
            key: PathBuf::from("./keys/private.key"),
            root_ca: PathBuf::from("./keys/rootCA.pem"),
            domain: String::from("localhost"),
        }
    }
}

pub fn server_config(config: &TlsConfig) -> Result<ServerConfig, TlsError> {
    let mut root_store = RootCertStore::empty();
    let ca_file = File::open(&config.root_ca).map_err(TlsError::CertFileIo)?;
    let mut ca_reader = BufReader::new(ca_file);

    for cert in rustls_pemfile::certs(&mut ca_reader) {
        root_store
            .add(cert.map_err(TlsError::CertPem)?)
            .map_err(TlsError::RootCertError)?;
    }

    // We use Arc::new(root_store) because the verifier requires it
    let client_verifier = WebPkiClientVerifier::builder(Arc::new(root_store))
        .build()
        .map_err(|e| TlsError::ServerConfigError(rustls::Error::General(e.to_string())))?;

    let cert_file = File::open(&config.cert).map_err(TlsError::CertFileIo)?;
    let mut cert_reader = BufReader::new(cert_file);
    let cert_chain: Vec<CertificateDer> = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(TlsError::CertPem)?;

    let key_file = File::open(&config.key).map_err(TlsError::CertFileIo)?;
    let mut key_reader = BufReader::new(key_file);
    let key = rustls_pemfile::private_key(&mut key_reader)
        .map_err(TlsError::KeyPem)?
        .ok_or_else(|| TlsError::NoPrivateKey(config.key.clone()))?;

    let config = ServerConfig::builder()
        .with_client_cert_verifier(client_verifier)
        .with_single_cert(cert_chain, key)
        .map_err(TlsError::ServerConfigError)?;

    Ok(config)
}

use tokio_rustls::rustls::ClientConfig;

pub fn client_config(config: &TlsConfig) -> Result<ClientConfig, TlsError> {
    let mut root_store = RootCertStore::empty();
    let ca_file = File::open(&config.root_ca).map_err(TlsError::CertFileIo)?;
    let mut ca_reader = BufReader::new(ca_file);

    for cert in rustls_pemfile::certs(&mut ca_reader) {
        root_store
            .add(cert.map_err(TlsError::CertPem)?)
            .map_err(TlsError::RootCertError)?;
    }

    let cert_file = File::open(&config.cert).map_err(TlsError::CertFileIo)?;
    let mut cert_reader = BufReader::new(cert_file);
    let cert_chain: Vec<CertificateDer> = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(TlsError::CertPem)?;

    let key_file = File::open(&config.key).map_err(TlsError::CertFileIo)?;
    let mut key_reader = BufReader::new(key_file);
    let key = rustls_pemfile::private_key(&mut key_reader)
        .map_err(TlsError::KeyPem)?
        .ok_or_else(|| TlsError::NoPrivateKey(config.key.clone()))?;

    // with_client_auth_cert automatically handles presenting the cert to the server
    let client_config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_client_auth_cert(cert_chain, key)
        .map_err(TlsError::ClientConfigError)?;

    Ok(client_config)
}
