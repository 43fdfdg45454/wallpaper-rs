use crate::{Error, Result, hex};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio_rustls::rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use tokio_rustls::rustls::crypto::{
    CryptoProvider, aws_lc_rs, verify_tls12_signature, verify_tls13_signature,
};
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use tokio_rustls::rustls::{self, CertificateError, ClientConfig, DigitallySignedStruct, SignatureScheme};

#[derive(Debug)]
struct Pinned {
    pin: Option<String>,
    provider: Arc<CryptoProvider>,
}

pub fn fingerprint(der: &[u8]) -> String {
    hex(&Sha256::digest(der))
}

pub fn valid_pin(pin: &str) -> bool {
    pin.len() == 64 && pin.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        match &self.pin {
            Some(pin) if *pin != fingerprint(end_entity) => {
                Err(rustls::Error::InvalidCertificate(CertificateError::NotValidForName))
            }
            _ => Ok(ServerCertVerified::assertion()),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

pub(crate) fn config(pin: Option<&str>) -> Result<ClientConfig> {
    let provider = Arc::new(aws_lc_rs::default_provider());
    let verifier = Arc::new(Pinned { pin: pin.map(str::to_ascii_lowercase), provider: provider.clone() });
    let builder = ClientConfig::builder_with_provider(provider).with_safe_default_protocol_versions();
    let builder = builder.map_err(|e| Error::Invalid(e.to_string()))?;
    Ok(builder.dangerous().with_custom_certificate_verifier(verifier).with_no_client_auth())
}

pub async fn peek(base: &str) -> Result<String> {
    let url =
        reqwest::Url::parse(base).map_err(|_| Error::Invalid("the server address is not a URL".into()))?;
    let host = url.host_str().unwrap_or_default().trim_matches(['[', ']']).to_owned();
    let address = (host.clone(), url.port_or_known_default().unwrap_or(443));
    let name = ServerName::try_from(host).map_err(|e| Error::Invalid(e.to_string()))?;
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config(None)?));
    let wait = std::time::Duration::from_secs(10);
    let shaken = tokio::time::timeout(wait, async {
        let tcp = tokio::net::TcpStream::connect(address).await?;
        connector.connect(name, tcp).await
    });
    let stream = shaken.await.map_err(|_| Error::Network("the server did not answer".into()))?;
    let stream = stream.map_err(|e| Error::Network(e.to_string()))?;
    let leaf = stream.get_ref().1.peer_certificates().and_then(|chain| chain.first().cloned());
    leaf.map(|der| fingerprint(&der)).ok_or_else(|| Error::Network("the server sent no certificate".into()))
}

pub fn untrusted(e: &(dyn std::error::Error + 'static)) -> bool {
    let mut cause = Some(e);
    while let Some(e) = cause {
        if let Some(rustls::Error::InvalidCertificate(_)) = e.downcast_ref::<rustls::Error>() {
            return true;
        }
        let inner = e.downcast_ref::<std::io::Error>().and_then(|io| io.get_ref());
        cause = inner.map(|e| e as &(dyn std::error::Error + 'static)).or_else(|| e.source());
    }
    false
}
