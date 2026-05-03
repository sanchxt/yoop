//! Device pairing without requiring a file transfer.
//!
//! Pairing exposes only Yoop device identity metadata over a single-use TLS
//! connection. Both sides prove possession of their Ed25519 private keys before
//! the caller stores the peer in the trust database.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use base64::prelude::*;
use rand::RngCore;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;
use tokio_rustls::client::TlsStream as ClientTlsStream;
use tokio_rustls::server::TlsStream as ServerTlsStream;
use tokio_rustls::{TlsAcceptor, TlsConnector};
use uuid::Uuid;

use crate::crypto::{DeviceIdentity, TlsConfig};
use crate::discovery::{DiscoveryPacket, HybridBroadcaster};
use crate::error::{Error, Result};
use crate::protocol::{
    self, MessageType, PairingAckPayload, PairingHelloPayload, PairingResultPayload,
};
use crate::{DEFAULT_DISCOVERY_PORT, DEFAULT_PAIRING_PORT, DEFAULT_TRANSFER_PORT_START};

/// Pairing runtime configuration.
#[derive(Debug, Clone)]
pub struct PairingConfig {
    /// TCP port used only for pairing identity exchange.
    pub pairing_port: u16,
    /// TCP port that should be stored for future trusted connections.
    pub trust_port: u16,
    /// UDP discovery port used to announce pairing availability.
    pub discovery_port: u16,
    /// How often the pairing listener broadcasts its availability.
    pub broadcast_interval: Duration,
    /// Local device display name.
    pub device_name: String,
}

impl Default for PairingConfig {
    fn default() -> Self {
        Self {
            pairing_port: DEFAULT_PAIRING_PORT,
            trust_port: DEFAULT_TRANSFER_PORT_START,
            discovery_port: DEFAULT_DISCOVERY_PORT,
            broadcast_interval: Duration::from_secs(2),
            device_name: hostname::get().map_or_else(
                |_| "Yoop Device".to_string(),
                |h| h.to_string_lossy().to_string(),
            ),
        }
    }
}

/// Identity metadata exchanged during pairing.
#[derive(Debug, Clone)]
pub struct PairingIdentity {
    /// Peer display name.
    pub device_name: String,
    /// Peer stable device ID.
    pub device_id: Uuid,
    /// Peer base64-encoded Ed25519 public key.
    pub public_key: String,
    /// Address to store for future trusted connections.
    pub address: SocketAddr,
}

/// Pairing listener that advertises and accepts identity exchanges.
pub struct PairingListener {
    listener: TcpListener,
    broadcaster: HybridBroadcaster,
    identity: DeviceIdentity,
    config: PairingConfig,
}

impl PairingListener {
    /// Bind a pairing listener and start advertising it via discovery.
    ///
    /// # Errors
    ///
    /// Returns an error if the identity, TCP listener, TLS config, or discovery
    /// broadcaster cannot be initialized.
    pub async fn bind(config: PairingConfig) -> Result<Self> {
        validate_trust_port(config.trust_port)?;

        let identity = DeviceIdentity::load_or_generate()?;
        let listener = TcpListener::bind(("0.0.0.0", config.pairing_port)).await?;
        let local_port = listener.local_addr()?.port();

        let broadcaster = HybridBroadcaster::new(config.discovery_port).await?;
        let packet = DiscoveryPacket {
            protocol: "yoop".to_string(),
            version: "1.0".to_string(),
            code: "PAIR".to_string(),
            device_name: config.device_name.clone(),
            device_id: identity.device_id(),
            expires_at: 0,
            transfer_port: local_port,
            supports: vec!["tcp".to_string(), "pairing".to_string()],
            file_count: 0,
            total_size: 0,
            preview_available: false,
        };
        broadcaster.start(packet, config.broadcast_interval).await?;

        Ok(Self {
            listener,
            broadcaster,
            identity,
            config: PairingConfig {
                pairing_port: local_port,
                ..config
            },
        })
    }

    /// Get the port this listener is bound to.
    #[must_use]
    pub const fn pairing_port(&self) -> u16 {
        self.config.pairing_port
    }

    /// Wait for one peer to request pairing.
    ///
    /// The returned pending pairing must be explicitly accepted or rejected by
    /// calling [`PendingHostPairing::finish`].
    ///
    /// # Errors
    ///
    /// Returns an error if the connection, TLS handshake, protocol exchange, or
    /// peer identity verification fails.
    pub async fn wait_for_peer(&self) -> Result<PendingHostPairing> {
        let (stream, peer_addr) = self.listener.accept().await?;

        let acceptor = TlsAcceptor::from(Arc::new(
            TlsConfig::server()?
                .server_config()
                .ok_or_else(|| Error::TlsError("no server config".to_string()))?
                .clone(),
        ));

        let mut tls_stream = acceptor
            .accept(stream)
            .await
            .map_err(|e| Error::TlsError(format!("TLS handshake failed: {e}")))?;

        let nonce = send_pairing_hello(
            &mut tls_stream,
            &self.identity,
            &self.config.device_name,
            self.config.trust_port,
        )
        .await?;

        let (header, payload) = protocol::read_frame(&mut tls_stream).await?;
        if header.message_type != MessageType::PairingAck {
            return Err(Error::UnexpectedMessage {
                expected: "PairingAck".to_string(),
                actual: format!("{:?}", header.message_type),
            });
        }

        let ack: PairingAckPayload = protocol::decode_payload(&payload)?;
        if !ack.accepted {
            return Err(Error::ConnectionRejected);
        }

        let device_name = ack
            .device_name
            .ok_or_else(|| Error::ProtocolError("Missing device_name in PairingAck".to_string()))?;
        let device_id = ack
            .device_id
            .ok_or_else(|| Error::ProtocolError("Missing device_id in PairingAck".to_string()))?;
        let public_key = ack
            .public_key
            .ok_or_else(|| Error::ProtocolError("Missing public_key in PairingAck".to_string()))?;
        let signature = ack.nonce_signature.ok_or_else(|| {
            Error::ProtocolError("Missing nonce_signature in PairingAck".to_string())
        })?;
        let trust_port = ack
            .trust_port
            .ok_or_else(|| Error::ProtocolError("Missing trust_port in PairingAck".to_string()))?;

        validate_identity(device_id, &public_key, &nonce, &signature)?;

        Ok(PendingHostPairing {
            peer: PairingIdentity {
                device_name,
                device_id,
                public_key,
                address: SocketAddr::new(peer_addr.ip(), trust_port),
            },
            stream: tls_stream,
        })
    }

    /// Stop discovery announcements and release mDNS resources.
    pub async fn shutdown(self) {
        self.broadcaster.stop().await;
        let _ = self.broadcaster.shutdown();
    }
}

/// A host-side pairing waiting for local approval.
pub struct PendingHostPairing {
    peer: PairingIdentity,
    stream: ServerTlsStream<TcpStream>,
}

impl PendingHostPairing {
    /// Get the verified peer identity.
    #[must_use]
    pub const fn peer(&self) -> &PairingIdentity {
        &self.peer
    }

    /// Finish the pairing by accepting or rejecting the peer.
    ///
    /// # Errors
    ///
    /// Returns an error if the final result frame cannot be written.
    pub async fn finish(
        mut self,
        accepted: bool,
        error: Option<String>,
    ) -> Result<PairingIdentity> {
        let result = PairingResultPayload { accepted, error };
        let payload = protocol::encode_payload(&result)?;
        protocol::write_frame(&mut self.stream, MessageType::PairingResult, &payload).await?;

        if accepted {
            Ok(self.peer)
        } else {
            Err(Error::ConnectionRejected)
        }
    }
}

/// A client-side pairing waiting for local approval.
pub struct PendingClientPairing {
    peer: PairingIdentity,
    remote_nonce: Vec<u8>,
    stream: ClientTlsStream<TcpStream>,
    identity: DeviceIdentity,
    config: PairingConfig,
}

impl PendingClientPairing {
    /// Get the verified peer identity.
    #[must_use]
    pub const fn peer(&self) -> &PairingIdentity {
        &self.peer
    }

    /// Accept pairing with the peer and wait for the peer to accept us too.
    ///
    /// # Errors
    ///
    /// Returns an error if either side rejects or if the protocol exchange
    /// fails.
    pub async fn accept(mut self) -> Result<PairingIdentity> {
        let signature = self.identity.sign(&self.remote_nonce);
        let ack = PairingAckPayload {
            accepted: true,
            device_name: Some(self.config.device_name.clone()),
            device_id: Some(self.identity.device_id()),
            public_key: Some(self.identity.public_key_base64()),
            nonce_signature: Some(BASE64_STANDARD.encode(signature)),
            trust_port: Some(self.config.trust_port),
            error: None,
        };
        let payload = protocol::encode_payload(&ack)?;
        protocol::write_frame(&mut self.stream, MessageType::PairingAck, &payload).await?;

        let (header, payload) = protocol::read_frame(&mut self.stream).await?;
        if header.message_type != MessageType::PairingResult {
            return Err(Error::UnexpectedMessage {
                expected: "PairingResult".to_string(),
                actual: format!("{:?}", header.message_type),
            });
        }

        let result: PairingResultPayload = protocol::decode_payload(&payload)?;
        if result.accepted {
            Ok(self.peer)
        } else {
            Err(Error::ConnectionRejected)
        }
    }

    /// Reject pairing with the peer.
    ///
    /// # Errors
    ///
    /// Returns an error if the rejection frame cannot be written.
    pub async fn reject(mut self, reason: impl Into<String>) -> Result<()> {
        let ack = PairingAckPayload {
            accepted: false,
            device_name: None,
            device_id: None,
            public_key: None,
            nonce_signature: None,
            trust_port: None,
            error: Some(reason.into()),
        };
        let payload = protocol::encode_payload(&ack)?;
        protocol::write_frame(&mut self.stream, MessageType::PairingAck, &payload).await
    }
}

/// Connect to a pairing listener and return its verified identity.
///
/// # Errors
///
/// Returns an error if the TCP/TLS connection or identity verification fails.
pub async fn connect(addr: SocketAddr, config: PairingConfig) -> Result<PendingClientPairing> {
    validate_trust_port(config.trust_port)?;

    let stream = TcpStream::connect(addr).await?;
    let connector = TlsConnector::from(Arc::new(
        TlsConfig::client()?
            .client_config()
            .ok_or_else(|| Error::TlsError("no client config".to_string()))?
            .clone(),
    ));

    let mut tls_stream = connector
        .connect("localhost".try_into().unwrap(), stream)
        .await
        .map_err(|e| Error::TlsError(format!("TLS handshake failed: {e}")))?;

    let (header, payload) = protocol::read_frame(&mut tls_stream).await?;
    if header.message_type != MessageType::PairingHello {
        return Err(Error::UnexpectedMessage {
            expected: "PairingHello".to_string(),
            actual: format!("{:?}", header.message_type),
        });
    }

    let hello: PairingHelloPayload = protocol::decode_payload(&payload)?;
    let remote_nonce = validate_identity(
        hello.device_id,
        &hello.public_key,
        &hello.nonce,
        &hello.nonce_signature,
    )?;

    Ok(PendingClientPairing {
        peer: PairingIdentity {
            device_name: hello.device_name,
            device_id: hello.device_id,
            public_key: hello.public_key,
            address: SocketAddr::new(addr.ip(), hello.trust_port),
        },
        remote_nonce,
        stream: tls_stream,
        identity: DeviceIdentity::load_or_generate()?,
        config,
    })
}

/// Probe a pairing listener and reject the pairing after reading identity.
///
/// # Errors
///
/// Returns an error if the probe cannot connect or verify the listener
/// identity before the timeout expires.
pub async fn probe(
    addr: SocketAddr,
    config: PairingConfig,
    probe_timeout: Duration,
) -> Result<PairingIdentity> {
    let pending = timeout(probe_timeout, connect(addr, config))
        .await
        .map_err(|_| Error::Timeout(probe_timeout.as_secs()))??;
    let peer = pending.peer().clone();
    let _ = pending.reject("probe only").await;
    Ok(peer)
}

async fn send_pairing_hello<S>(
    stream: &mut S,
    identity: &DeviceIdentity,
    device_name: &str,
    trust_port: u16,
) -> Result<String>
where
    S: tokio::io::AsyncWriteExt + Unpin,
{
    let mut nonce = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let signature = identity.sign(&nonce);
    let nonce_b64 = BASE64_STANDARD.encode(nonce);

    let hello = PairingHelloPayload {
        device_name: device_name.to_string(),
        protocol_version: "1.0".to_string(),
        device_id: identity.device_id(),
        public_key: identity.public_key_base64(),
        nonce: nonce_b64.clone(),
        nonce_signature: BASE64_STANDARD.encode(signature),
        trust_port,
    };
    let payload = protocol::encode_payload(&hello)?;
    protocol::write_frame(stream, MessageType::PairingHello, &payload).await?;

    Ok(nonce_b64)
}

fn validate_identity(
    device_id: Uuid,
    public_key: &str,
    nonce_b64: &str,
    signature_b64: &str,
) -> Result<Vec<u8>> {
    let derived_device_id = DeviceIdentity::derive_device_id_from_public_key_base64(public_key)?;
    if derived_device_id != device_id {
        return Err(Error::TrustError(format!(
            "Device ID mismatch: expected derived ID {}, got {}",
            derived_device_id, device_id
        )));
    }

    let nonce = BASE64_STANDARD
        .decode(nonce_b64)
        .map_err(|e| Error::ProtocolError(format!("Invalid nonce: {e}")))?;
    let signature_bytes = BASE64_STANDARD
        .decode(signature_b64)
        .map_err(|e| Error::ProtocolError(format!("Invalid signature: {e}")))?;
    let signature: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| Error::ProtocolError("Invalid signature length".to_string()))?;

    if !DeviceIdentity::verify_base64(public_key, &nonce, &signature) {
        return Err(Error::TrustError("Invalid pairing signature".to_string()));
    }

    Ok(nonce)
}

fn validate_trust_port(port: u16) -> Result<()> {
    if port != 0 {
        return Ok(());
    }

    Err(Error::InvalidConfig {
        key: "trust_port".to_string(),
        reason: "must be a non-zero TCP port".to_string(),
    })
}
