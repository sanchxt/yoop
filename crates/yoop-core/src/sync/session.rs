//! Sync session implementation for bidirectional directory synchronization.
//!
//! This module provides the `SyncSession` type which manages the full lifecycle
//! of a sync session, including connection establishment, index exchange,
//! reconciliation, and file transfer.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::task::JoinHandle;
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};

use super::conflict::ResolutionStrategy;
use super::engine::{SyncEngine, SyncPlan};
use super::index::{FileEntry, FileIndex};
use super::watcher::{FileEvent, FileEventKind, FileWatcher};
use super::{FileKind, RelativePath, SyncConfig, SyncOp, SyncStats};
use crate::code::{CodeGenerator, ShareCode};
use crate::crypto::{self, TlsConfig};
use crate::discovery::{DiscoveryPacket, HybridBroadcaster, HybridListener};
use crate::file::{FileChunk, FileChunker, FileWriter};
use crate::protocol::{
    decode_payload, decode_sync_chunk, encode_payload, encode_sync_chunk, read_frame, write_frame,
    HelloPayload, MessageType, SyncCapabilities, SyncChunkAckPayload, SyncChunkPayload,
    SyncCompletePayload, SyncIndexEntry, SyncIndexPayload, SyncInitPayload, SyncOpAckPayload,
    SyncOpPayload, SyncOpType,
};
use crate::transfer::TransferConfig;
use crate::{Error, Result, DEFAULT_CHUNK_SIZE, PROTOCOL_VERSION};

/// Events emitted during sync for UI updates.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SyncEvent {
    /// Connected to peer
    Connected {
        /// Peer device name
        peer_name: String,
    },
    /// Indices exchanged
    IndexExchanged {
        /// Number of local files
        local_files: u64,
        /// Number of remote files
        remote_files: u64,
    },
    /// Starting initial reconciliation
    ReconcileStart {
        /// Number of operations to perform
        ops_count: u64,
    },
    /// Sending a file
    FileSending {
        /// File path
        path: String,
        /// File size
        size: u64,
    },
    /// File sent successfully
    FileSent {
        /// File path
        path: String,
    },
    /// Receiving a file
    FileReceiving {
        /// File path
        path: String,
        /// File size
        size: u64,
    },
    /// File received successfully
    FileReceived {
        /// File path
        path: String,
    },
    /// File deleted
    FileDeleted {
        /// File path
        path: String,
    },
    /// Conflict detected and resolved
    Conflict {
        /// File path
        path: String,
        /// Resolution description
        resolution: String,
    },
    /// Error occurred
    Error {
        /// Error message
        message: String,
    },
    /// Statistics update
    Stats {
        /// Current statistics
        stats: SyncStats,
    },
}

/// A bidirectional sync session.
pub struct SyncSession {
    config: SyncConfig,
    #[allow(dead_code)]
    transfer_config: TransferConfig,
    local_index: FileIndex,
    remote_index: FileIndex,
    peer_name: Option<String>,
    stats: SyncStats,
    sync_engine: SyncEngine,
    op_id_counter: u64,
    tls_stream: Option<TlsStream<TcpStream>>,
    #[allow(dead_code)]
    session_start: Instant,
}

impl std::fmt::Debug for SyncSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncSession")
            .field("config", &self.config)
            .field("peer_name", &self.peer_name)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl SyncSession {
    /// Host a new sync session (waits for connection).
    ///
    /// # Errors
    ///
    /// Returns an error if the session cannot be created or the connection fails.
    pub async fn host(
        config: SyncConfig,
        transfer_config: TransferConfig,
    ) -> Result<(ShareCode, Self)> {
        let code = CodeGenerator::new().generate()?;
        tracing::info!("Hosting sync session with code: {}", code);

        let local_index = FileIndex::build(&config.sync_root, &config)?;
        tracing::debug!(
            "Built local index: {} files, {} bytes",
            local_index.len(),
            local_index.total_size()
        );

        let session_key = crypto::derive_session_key(code.as_str());
        let tls_config = TlsConfig::server()?;
        let server_config = tls_config
            .server_config()
            .ok_or_else(|| Error::Internal("server config not available".to_string()))?;
        let acceptor = TlsAcceptor::from(Arc::new(server_config.clone()));

        let listener =
            TcpListener::bind(format!("0.0.0.0:{}", transfer_config.transfer_port)).await?;
        let local_addr = listener.local_addr()?;

        let device_name = hostname::get().map_or_else(
            |_| "Unknown".to_string(),
            |h| h.to_string_lossy().to_string(),
        );

        let device_id = uuid::Uuid::new_v4();
        let packet = DiscoveryPacket::new(
            &code,
            &device_name,
            device_id,
            local_addr.port(),
            local_index.len(),
            local_index.total_size(),
        );
        let broadcaster = HybridBroadcaster::new(transfer_config.discovery_port).await?;
        broadcaster
            .start(packet, transfer_config.broadcast_interval)
            .await?;

        tracing::info!("Waiting for connection on {}", local_addr);

        let (tcp_stream, peer_addr) = listener.accept().await?;
        tracing::info!("Connection from {}", peer_addr);

        configure_tcp_keepalive(&tcp_stream)?;

        let mut tls_stream = acceptor.accept(tcp_stream).await?;

        broadcaster.stop().await;

        let (peer_name, remote_index) = Self::handshake_host(
            &mut tls_stream,
            &device_name,
            &session_key,
            &local_index,
            &config,
        )
        .await?;

        Ok((
            code,
            Self {
                config,
                transfer_config,
                local_index,
                remote_index,
                peer_name: Some(peer_name),
                stats: SyncStats::new(),
                sync_engine: SyncEngine::new(ResolutionStrategy::default()),
                op_id_counter: 0,
                tls_stream: Some(tokio_rustls::TlsStream::Server(tls_stream)),
                session_start: Instant::now(),
            },
        ))
    }

    /// Connect to a sync session using a code.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails or the handshake fails.
    pub async fn connect(
        code: &str,
        config: SyncConfig,
        transfer_config: TransferConfig,
    ) -> Result<Self> {
        tracing::info!("Connecting to sync session with code: {}", code);

        let local_index = FileIndex::build(&config.sync_root, &config)?;
        tracing::debug!(
            "Built local index: {} files, {} bytes",
            local_index.len(),
            local_index.total_size()
        );

        let session_key = crypto::derive_session_key(code);

        let listener = HybridListener::new(transfer_config.discovery_port).await?;

        let share_code = ShareCode::parse(code)?;
        let announcement = listener
            .find(&share_code, transfer_config.discovery_timeout)
            .await?;

        let peer_addr: SocketAddr = format!(
            "{}:{}",
            announcement.source.ip(),
            announcement.packet.transfer_port
        )
        .parse()
        .map_err(|e| Error::Internal(format!("Invalid peer address: {e}")))?;

        tracing::info!("Found peer at {}", peer_addr);

        let tcp_stream = TcpStream::connect(peer_addr).await?;
        configure_tcp_keepalive(&tcp_stream)?;

        let tls_config = TlsConfig::client()?;
        let client_config = tls_config
            .client_config()
            .ok_or_else(|| Error::Internal("client config not available".to_string()))?;
        let connector = TlsConnector::from(Arc::new(client_config.clone()));
        let domain = ServerName::try_from("yoop.local")
            .map_err(|_| Error::TlsError("invalid server name".to_string()))?;

        let mut tls_stream = connector.connect(domain, tcp_stream).await?;

        let device_name = hostname::get().map_or_else(
            |_| "Unknown".to_string(),
            |h| h.to_string_lossy().to_string(),
        );

        let (peer_name, remote_index) = Self::handshake_client(
            &mut tls_stream,
            &device_name,
            &session_key,
            &local_index,
            &config,
        )
        .await?;

        Ok(Self {
            config,
            transfer_config,
            local_index,
            remote_index,
            peer_name: Some(peer_name),
            stats: SyncStats::new(),
            sync_engine: SyncEngine::new(ResolutionStrategy::default()),
            op_id_counter: 0,
            tls_stream: Some(tokio_rustls::TlsStream::Client(tls_stream)),
            session_start: Instant::now(),
        })
    }

    /// Host-side handshake.
    async fn handshake_host<S>(
        stream: &mut S,
        device_name: &str,
        _session_key: &[u8; 32],
        local_index: &FileIndex,
        config: &SyncConfig,
    ) -> Result<(String, FileIndex)>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let (header, payload) = read_frame(stream).await?;
        if header.message_type != MessageType::Hello {
            return Err(Error::UnexpectedMessage {
                expected: "Hello".to_string(),
                actual: format!("{:?}", header.message_type),
            });
        }

        let hello: HelloPayload = decode_payload(&payload)?;
        let peer_name = hello.device_name.clone();

        let hello_ack = HelloPayload {
            device_name: device_name.to_string(),
            protocol_version: format!("{}.{}", PROTOCOL_VERSION.0, PROTOCOL_VERSION.1),
            device_id: None,
            public_key: None,
        };
        write_frame(stream, MessageType::HelloAck, &encode_payload(&hello_ack)?).await?;

        let sync_init = SyncInitPayload {
            sync_root_name: config
                .sync_root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("sync")
                .to_string(),
            file_count: local_index.len() as u64,
            total_size: local_index.total_size(),
            index_hash: local_index.root_hash(),
            protocol_version: 1,
            capabilities: SyncCapabilities::default(),
        };
        write_frame(stream, MessageType::SyncInit, &encode_payload(&sync_init)?).await?;

        let (header, payload) = read_frame(stream).await?;
        if header.message_type != MessageType::SyncInitAck {
            return Err(Error::ProtocolError("expected SyncInitAck".to_string()));
        }

        let _remote_init: SyncInitPayload = decode_payload(&payload)?;

        let remote_index = Self::exchange_index(stream, local_index).await?;

        Ok((peer_name, remote_index))
    }

    /// Client-side handshake.
    async fn handshake_client<S>(
        stream: &mut S,
        device_name: &str,
        _session_key: &[u8; 32],
        local_index: &FileIndex,
        config: &SyncConfig,
    ) -> Result<(String, FileIndex)>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let hello = HelloPayload {
            device_name: device_name.to_string(),
            protocol_version: format!("{}.{}", PROTOCOL_VERSION.0, PROTOCOL_VERSION.1),
            device_id: None,
            public_key: None,
        };
        write_frame(stream, MessageType::Hello, &encode_payload(&hello)?).await?;

        let (header, payload) = read_frame(stream).await?;
        if header.message_type != MessageType::HelloAck {
            return Err(Error::UnexpectedMessage {
                expected: "HelloAck".to_string(),
                actual: format!("{:?}", header.message_type),
            });
        }

        let hello_ack: HelloPayload = decode_payload(&payload)?;
        let peer_name = hello_ack.device_name.clone();

        let (header, payload) = read_frame(stream).await?;
        if header.message_type != MessageType::SyncInit {
            return Err(Error::ProtocolError("expected SyncInit".to_string()));
        }

        let _remote_init: SyncInitPayload = decode_payload(&payload)?;

        let sync_init_ack = SyncInitPayload {
            sync_root_name: config
                .sync_root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("sync")
                .to_string(),
            file_count: local_index.len() as u64,
            total_size: local_index.total_size(),
            index_hash: local_index.root_hash(),
            protocol_version: 1,
            capabilities: SyncCapabilities::default(),
        };
        write_frame(
            stream,
            MessageType::SyncInitAck,
            &encode_payload(&sync_init_ack)?,
        )
        .await?;

        let remote_index = Self::exchange_index(stream, local_index).await?;

        Ok((peer_name, remote_index))
    }

    /// Exchange file indices with the peer.
    async fn exchange_index<S>(stream: &mut S, local_index: &FileIndex) -> Result<FileIndex>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let local_entries: Vec<SyncIndexEntry> = local_index
            .entries()
            .map(|e| SyncIndexEntry {
                path: e.path.as_str().to_string(),
                kind: e.kind as u8,
                size: e.size,
                #[allow(clippy::cast_possible_wrap)]
                mtime: e
                    .mtime
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
                content_hash: e.content_hash,
            })
            .collect();

        let payload = SyncIndexPayload {
            entries: local_entries,
        };
        write_frame(stream, MessageType::SyncIndex, &encode_payload(&payload)?).await?;

        let (header, payload_data) = read_frame(stream).await?;
        if header.message_type != MessageType::SyncIndex {
            return Err(Error::ProtocolError("expected SyncIndex".to_string()));
        }

        let remote_payload: SyncIndexPayload = decode_payload(&payload_data)?;

        write_frame(stream, MessageType::SyncIndexAck, &[]).await?;

        let (header, _) = read_frame(stream).await?;
        if header.message_type != MessageType::SyncIndexAck {
            return Err(Error::ProtocolError("expected SyncIndexAck".to_string()));
        }

        let mut entries_map = HashMap::new();
        for entry in remote_payload.entries {
            let file_entry = FileEntry {
                path: RelativePath::new(entry.path),
                kind: match entry.kind {
                    1 => FileKind::Directory,
                    2 => FileKind::Symlink,
                    _ => FileKind::File,
                },
                size: entry.size,
                #[allow(clippy::cast_sign_loss)]
                mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(entry.mtime as u64),
                content_hash: entry.content_hash,
            };
            entries_map.insert(file_entry.path.clone(), file_entry);
        }

        Ok(FileIndex::from_entries(entries_map))
    }

    /// Get peer device name.
    #[must_use]
    pub fn peer_name(&self) -> &str {
        self.peer_name.as_deref().unwrap_or("Unknown")
    }

    /// Get sync statistics.
    #[must_use]
    pub fn stats(&self) -> &SyncStats {
        &self.stats
    }

    /// Get next operation ID.
    #[allow(dead_code)]
    fn next_op_id(&mut self) -> u64 {
        let id = self.op_id_counter;
        self.op_id_counter += 1;
        id
    }

    /// Run the initial sync reconciliation.
    ///
    /// # Errors
    ///
    /// Returns an error if reconciliation or file operations fail.
    pub async fn run_initial_sync<F>(&mut self, mut event_callback: F) -> Result<()>
    where
        F: FnMut(SyncEvent),
    {
        tracing::info!("Starting initial sync");

        event_callback(SyncEvent::IndexExchanged {
            local_files: self.local_index.len() as u64,
            remote_files: self.remote_index.len() as u64,
        });

        let (mut local_ops, mut remote_ops, conflicts) = self
            .sync_engine
            .reconcile(&self.local_index, &self.remote_index);

        if !conflicts.is_empty() {
            tracing::info!(
                "Detected {} conflicts, applying resolutions",
                conflicts.len()
            );
            for conflict in &conflicts {
                event_callback(SyncEvent::Conflict {
                    path: conflict.path.as_str().to_string(),
                    resolution: format!("{:?}", self.sync_engine.conflict_detector().strategy()),
                });
            }

            let resolutions = self.sync_engine.apply_conflict_resolutions(
                &conflicts,
                &mut local_ops,
                &mut remote_ops,
            );

            self.stats.conflicts += conflicts.len() as u64;
            tracing::debug!("Applied {} conflict resolutions", resolutions.len());
        }

        let local_plan = SyncPlan::from_ops(local_ops);
        let remote_plan = SyncPlan::from_ops(remote_ops);

        let total_ops = local_plan.total_ops() + remote_plan.total_ops();
        if total_ops == 0 {
            tracing::info!("No sync operations needed, indices are in sync");
            return Ok(());
        }

        event_callback(SyncEvent::ReconcileStart {
            ops_count: total_ops as u64,
        });

        tracing::info!(
            "Executing sync plan: {} local ops, {} remote ops",
            local_plan.total_ops(),
            remote_plan.total_ops()
        );

        for op in local_plan.into_ordered_ops() {
            self.apply_local_op(&op, &mut event_callback).await?;
        }

        event_callback(SyncEvent::Stats {
            stats: self.stats.clone(),
        });

        tracing::info!("Initial sync complete");
        Ok(())
    }

    /// Apply a local operation (received from peer).
    async fn apply_local_op<F>(&mut self, op: &SyncOp, event_callback: &mut F) -> Result<()>
    where
        F: FnMut(SyncEvent),
    {
        match op {
            SyncOp::Create {
                path,
                kind,
                size,
                content_hash: _,
            } => {
                let abs_path = path.to_path(&self.config.sync_root);

                match kind {
                    FileKind::Directory => {
                        tracing::debug!("Creating directory: {}", path.as_str());
                        tokio::fs::create_dir_all(&abs_path).await?;
                    }
                    FileKind::File => {
                        tracing::debug!("Creating file: {} ({} bytes)", path.as_str(), size);

                        if let Some(parent) = abs_path.parent() {
                            tokio::fs::create_dir_all(parent).await?;
                        }

                        event_callback(SyncEvent::FileReceiving {
                            path: path.as_str().to_string(),
                            size: *size,
                        });

                        self.stats.files_received += 1;
                        self.stats.bytes_received += size;

                        event_callback(SyncEvent::FileReceived {
                            path: path.as_str().to_string(),
                        });
                    }
                    FileKind::Symlink => {
                        tracing::warn!("Symlink creation not yet implemented: {}", path.as_str());
                    }
                }
            }
            SyncOp::Modify { path, size, .. } => {
                tracing::debug!("Modifying file: {} ({} bytes)", path.as_str(), size);

                event_callback(SyncEvent::FileReceiving {
                    path: path.as_str().to_string(),
                    size: *size,
                });

                self.stats.files_received += 1;
                self.stats.bytes_received += size;

                event_callback(SyncEvent::FileReceived {
                    path: path.as_str().to_string(),
                });
            }
            SyncOp::Delete { path, kind } => {
                if !self.config.sync_deletions {
                    tracing::debug!(
                        "Skipping deletion (sync_deletions=false): {}",
                        path.as_str()
                    );
                    return Ok(());
                }

                let abs_path = path.to_path(&self.config.sync_root);
                tracing::debug!("Deleting: {}", path.as_str());

                match kind {
                    FileKind::Directory => {
                        if abs_path.exists() {
                            tokio::fs::remove_dir_all(&abs_path).await?;
                        }
                    }
                    FileKind::File | FileKind::Symlink => {
                        if abs_path.exists() {
                            tokio::fs::remove_file(&abs_path).await?;
                        }
                    }
                }

                event_callback(SyncEvent::FileDeleted {
                    path: path.as_str().to_string(),
                });
            }
            SyncOp::Rename { from, to, kind: _ } => {
                let from_path = from.to_path(&self.config.sync_root);
                let to_path = to.to_path(&self.config.sync_root);
                tracing::debug!("Renaming: {} -> {}", from.as_str(), to.as_str());

                if let Some(parent) = to_path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }

                tokio::fs::rename(&from_path, &to_path).await?;
            }
        }

        Ok(())
    }

    /// Send a file operation to the peer.
    #[allow(dead_code)]
    async fn send_file_op<S>(&mut self, stream: &mut S, op: &SyncOp) -> Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let op_id = self.next_op_id();

        let (op_type, path, from_path, kind, size, content_hash) = match op {
            SyncOp::Create {
                path,
                kind,
                size,
                content_hash,
            } => (
                SyncOpType::Create,
                path.as_str().to_string(),
                None,
                *kind as u8,
                Some(*size),
                Some(*content_hash),
            ),
            SyncOp::Modify {
                path,
                size,
                content_hash,
            } => (
                SyncOpType::Modify,
                path.as_str().to_string(),
                None,
                FileKind::File as u8,
                Some(*size),
                Some(*content_hash),
            ),
            SyncOp::Delete { path, kind } => (
                SyncOpType::Delete,
                path.as_str().to_string(),
                None,
                *kind as u8,
                None,
                None,
            ),
            SyncOp::Rename { from, to, kind } => (
                SyncOpType::Rename,
                to.as_str().to_string(),
                Some(from.as_str().to_string()),
                *kind as u8,
                None,
                None,
            ),
        };

        #[allow(clippy::cast_possible_truncation)]
        let chunk_count = size.map(|file_size| {
            if file_size > 0 {
                file_size.div_ceil(DEFAULT_CHUNK_SIZE as u64) as u32
            } else {
                0
            }
        });

        let payload = SyncOpPayload {
            op_id,
            op_type,
            path,
            from_path,
            kind,
            size,
            content_hash,
            chunk_count,
        };

        write_frame(stream, MessageType::SyncOp, &encode_payload(&payload)?).await?;

        if let (Some(file_size), true) = (
            size,
            matches!(op_type, SyncOpType::Create | SyncOpType::Modify),
        ) {
            if file_size > 0 {
                let file_path = match op {
                    SyncOp::Create { path, .. } | SyncOp::Modify { path, .. } => {
                        path.to_path(&self.config.sync_root)
                    }
                    _ => unreachable!(),
                };

                self.send_file_chunks(stream, op_id, &file_path).await?;
            }
        }

        let (header, ack_payload) = read_frame(stream).await?;
        if header.message_type != MessageType::SyncOpAck {
            return Err(Error::ProtocolError("expected SyncOpAck".to_string()));
        }

        let ack: SyncOpAckPayload = decode_payload(&ack_payload)?;
        if !ack.success {
            return Err(Error::SyncOperationFailed(
                ack.error.unwrap_or_else(|| "unknown error".to_string()),
            ));
        }

        Ok(())
    }

    /// Send file chunks for a file operation.
    #[allow(dead_code)]
    async fn send_file_chunks<S>(
        &mut self,
        stream: &mut S,
        op_id: u64,
        file_path: &std::path::Path,
    ) -> Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let chunker = FileChunker::new(DEFAULT_CHUNK_SIZE);
        let chunks = chunker.read_chunks(file_path, 0).await?;

        for chunk in chunks {
            #[allow(clippy::cast_possible_truncation)]
            let chunk_payload = SyncChunkPayload {
                op_id,
                chunk_index: chunk.chunk_index as u32,
                data: chunk.data.clone(),
                checksum: chunk.checksum,
            };

            let data = encode_sync_chunk(&chunk_payload);
            write_frame(stream, MessageType::SyncChunk, &data).await?;

            let (header, ack_data) = read_frame(stream).await?;
            if header.message_type != MessageType::SyncChunkAck {
                return Err(Error::ProtocolError("expected SyncChunkAck".to_string()));
            }

            let ack: SyncChunkAckPayload = decode_payload(&ack_data)?;
            if !ack.success {
                return Err(Error::SyncOperationFailed(
                    "chunk transfer failed".to_string(),
                ));
            }

            self.stats.bytes_sent += chunk.data.len() as u64;
        }

        let complete = SyncCompletePayload {
            op_id,
            content_hash: 0,
        };
        write_frame(
            stream,
            MessageType::SyncComplete,
            &encode_payload(&complete)?,
        )
        .await?;

        self.stats.files_sent += 1;
        Ok(())
    }

    /// Receive a file operation from the peer.
    #[allow(dead_code)]
    async fn receive_file_op<S, F>(
        &mut self,
        stream: &mut S,
        payload: SyncOpPayload,
        event_callback: &mut F,
    ) -> Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
        F: FnMut(SyncEvent),
    {
        let op_id = payload.op_id;

        match payload.op_type {
            SyncOpType::Create | SyncOpType::Modify => {
                if let Some(size) = payload.size {
                    let path = RelativePath::new(&payload.path);
                    let abs_path = path.to_path(&self.config.sync_root);

                    if let Some(parent) = abs_path.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }

                    event_callback(SyncEvent::FileReceiving {
                        path: payload.path.clone(),
                        size,
                    });

                    if size > 0 && payload.chunk_count.unwrap_or(0) > 0 {
                        self.receive_file_chunks(stream, op_id, &abs_path, size)
                            .await?;
                    } else {
                        tokio::fs::File::create(&abs_path).await?;
                    }

                    event_callback(SyncEvent::FileReceived {
                        path: payload.path.clone(),
                    });

                    self.stats.files_received += 1;
                    self.stats.bytes_received += size;
                }
            }
            SyncOpType::Delete => {
                if self.config.sync_deletions {
                    let path = RelativePath::new(&payload.path);
                    let abs_path = path.to_path(&self.config.sync_root);

                    if abs_path.exists() {
                        if abs_path.is_dir() {
                            tokio::fs::remove_dir_all(&abs_path).await?;
                        } else {
                            tokio::fs::remove_file(&abs_path).await?;
                        }
                    }

                    event_callback(SyncEvent::FileDeleted {
                        path: payload.path.clone(),
                    });
                }
            }
            SyncOpType::Rename => {
                if let Some(from_path_str) = payload.from_path {
                    let from_path = RelativePath::new(&from_path_str);
                    let to_path = RelativePath::new(&payload.path);
                    let from_abs = from_path.to_path(&self.config.sync_root);
                    let to_abs = to_path.to_path(&self.config.sync_root);

                    if let Some(parent) = to_abs.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }

                    tokio::fs::rename(&from_abs, &to_abs).await?;
                }
            }
        }

        let ack = SyncOpAckPayload {
            op_id,
            success: true,
            error: None,
            content_hash: None,
        };
        write_frame(stream, MessageType::SyncOpAck, &encode_payload(&ack)?).await?;

        Ok(())
    }

    /// Receive file chunks for a file operation.
    #[allow(dead_code)]
    async fn receive_file_chunks<S>(
        &self,
        stream: &mut S,
        op_id: u64,
        output_path: &std::path::Path,
        expected_size: u64,
    ) -> Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        use crate::crypto::xxhash64;

        let mut writer = FileWriter::new(output_path.to_path_buf(), expected_size).await?;

        loop {
            let (header, chunk_data) = read_frame(stream).await?;

            match header.message_type {
                MessageType::SyncChunk => {
                    let chunk_payload = decode_sync_chunk(&chunk_data)?;

                    if chunk_payload.op_id != op_id {
                        return Err(Error::ProtocolError(format!(
                            "chunk op_id mismatch: expected {}, got {}",
                            op_id, chunk_payload.op_id
                        )));
                    }

                    let computed_checksum = xxhash64(&chunk_payload.data);
                    if computed_checksum != chunk_payload.checksum {
                        let ack = SyncChunkAckPayload {
                            op_id,
                            chunk_index: chunk_payload.chunk_index,
                            success: false,
                        };
                        write_frame(stream, MessageType::SyncChunkAck, &encode_payload(&ack)?)
                            .await?;
                        return Err(Error::ChecksumMismatch {
                            file: output_path.display().to_string(),
                            chunk: u64::from(chunk_payload.chunk_index),
                        });
                    }

                    let file_chunk = FileChunk {
                        file_index: 0,
                        chunk_index: u64::from(chunk_payload.chunk_index),
                        data: chunk_payload.data,
                        checksum: chunk_payload.checksum,
                        is_last: false,
                    };

                    writer.write_chunk(&file_chunk).await?;

                    let ack = SyncChunkAckPayload {
                        op_id,
                        chunk_index: chunk_payload.chunk_index,
                        success: true,
                    };
                    write_frame(stream, MessageType::SyncChunkAck, &encode_payload(&ack)?).await?;
                }
                MessageType::SyncComplete => {
                    let _complete: SyncCompletePayload = decode_payload(&chunk_data)?;
                    writer.finalize().await?;
                    break;
                }
                _ => {
                    return Err(Error::UnexpectedMessage {
                        expected: "SyncChunk or SyncComplete".to_string(),
                        actual: format!("{:?}", header.message_type),
                    });
                }
            }
        }

        Ok(())
    }

    /// Run the live sync session with continuous bidirectional synchronization.
    ///
    /// This method starts the live sync loop which monitors local file changes
    /// and synchronizes them with the peer in real-time. It runs until:
    /// - The user presses Ctrl+C
    /// - A fatal error occurs
    /// - The connection is lost and reconnection fails
    ///
    /// # Errors
    ///
    /// Returns an error if the sync session cannot be started or a fatal error occurs.
    pub async fn run<F>(&mut self, mut event_callback: F) -> Result<SyncStats>
    where
        F: FnMut(SyncEvent) + Send + 'static,
    {
        let start_time = Instant::now();

        self.run_initial_sync(&mut event_callback).await?;

        let tls_stream = self
            .tls_stream
            .take()
            .ok_or_else(|| Error::Internal("TLS stream not available".to_string()))?;

        let stream = Arc::new(Mutex::new(tls_stream));

        let (outbound_tx, outbound_rx) = mpsc::channel::<SyncOp>(100);
        let (shutdown_tx, _) = broadcast::channel::<()>(10);

        let mut watcher = FileWatcher::new(self.config.clone())?;
        watcher.start()?;

        let watcher_handle = Self::spawn_watcher_task(
            watcher,
            outbound_tx.clone(),
            shutdown_tx.subscribe(),
            Arc::new(Mutex::new(self.local_index.clone())),
        );

        let outbound_handle = Self::spawn_outbound_task(
            Arc::clone(&stream),
            outbound_rx,
            shutdown_tx.subscribe(),
            Arc::new(Mutex::new(self.stats.clone())),
            Arc::new(Mutex::new(self.op_id_counter)),
            self.config.clone(),
        );

        let inbound_handle = Self::spawn_inbound_task(
            Arc::clone(&stream),
            shutdown_tx.subscribe(),
            Arc::new(Mutex::new(self.stats.clone())),
            Arc::new(Mutex::new(self.local_index.clone())),
            self.config.clone(),
            event_callback,
        );

        let keepalive_handle =
            Self::spawn_keepalive_task(Arc::clone(&stream), shutdown_tx.subscribe());

        tokio::select! {
            result = watcher_handle => {
                tracing::info!("Watcher task completed: {:?}", result);
            }
            result = outbound_handle => {
                tracing::info!("Outbound task completed: {:?}", result);
            }
            result = inbound_handle => {
                tracing::info!("Inbound task completed: {:?}", result);
            }
            result = keepalive_handle => {
                tracing::info!("Keepalive task completed: {:?}", result);
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Ctrl+C received, shutting down");
            }
        }

        let _ = shutdown_tx.send(());

        self.stats.duration = start_time.elapsed();
        Ok(self.stats.clone())
    }

    /// Spawn the file watcher task.
    fn spawn_watcher_task(
        mut watcher: FileWatcher,
        outbound_tx: mpsc::Sender<SyncOp>,
        mut shutdown_rx: broadcast::Receiver<()>,
        local_index: Arc<Mutex<FileIndex>>,
    ) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    event_opt = watcher.next_event() => {
                        if let Some(event) = event_opt {
                            if let Some(op) = Self::event_to_sync_op(&event, &local_index).await? {
                                if let Err(e) = outbound_tx.send(op).await {
                                    tracing::error!("Failed to queue outbound operation: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::debug!("Watcher task received shutdown signal");
                        break;
                    }
                }
            }
            Ok(())
        })
    }

    /// Spawn the outbound task to send operations to the peer.
    fn spawn_outbound_task(
        stream: Arc<Mutex<TlsStream<TcpStream>>>,
        mut outbound_rx: mpsc::Receiver<SyncOp>,
        mut shutdown_rx: broadcast::Receiver<()>,
        stats: Arc<Mutex<SyncStats>>,
        op_id_counter: Arc<Mutex<u64>>,
        config: SyncConfig,
    ) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    op_opt = outbound_rx.recv() => {
                        if let Some(op) = op_opt {
                            let mut counter = op_id_counter.lock().await;
                            let op_id = *counter;
                            *counter += 1;
                            drop(counter);

                            let mut stream_guard = stream.lock().await;
                            if let Err(e) = Self::send_sync_op(&mut *stream_guard, &op, op_id, &config).await {
                                tracing::error!("Failed to send sync operation: {}", e);
                                break;
                            }
                            drop(stream_guard);

                            let mut stats_guard = stats.lock().await;
                            match &op {
                                SyncOp::Create { size, .. } | SyncOp::Modify { size, .. } => {
                                    stats_guard.files_sent += 1;
                                    stats_guard.bytes_sent += size;
                                }
                                _ => {}
                            }
                        } else {
                            break;
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::debug!("Outbound task received shutdown signal");
                        break;
                    }
                }
            }
            Ok(())
        })
    }

    /// Spawn the inbound task to receive operations from the peer.
    #[allow(clippy::needless_pass_by_value)]
    fn spawn_inbound_task<F>(
        stream: Arc<Mutex<TlsStream<TcpStream>>>,
        mut shutdown_rx: broadcast::Receiver<()>,
        stats: Arc<Mutex<SyncStats>>,
        #[allow(unused_variables)] local_index: Arc<Mutex<FileIndex>>,
        config: SyncConfig,
        mut event_callback: F,
    ) -> JoinHandle<Result<()>>
    where
        F: FnMut(SyncEvent) + Send + 'static,
    {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    frame_result = async {
                        let mut stream_guard = stream.lock().await;
                        read_frame(&mut *stream_guard).await
                    } => {
                        match frame_result {
                            Ok((header, payload)) => {
                                match header.message_type {
                                    MessageType::SyncOp => {
                                        let op_payload: SyncOpPayload = decode_payload(&payload)?;
                                        let mut stream_guard = stream.lock().await;
                                        if let Err(e) = Self::receive_sync_op(
                                            &mut *stream_guard,
                                            op_payload,
                                            &config,
                                            &stats,
                                            &mut event_callback,
                                        )
                                        .await
                                        {
                                            tracing::error!("Failed to receive sync operation: {}", e);
                                        }
                                    }
                                    MessageType::Ping => {
                                        write_frame(&mut *stream.lock().await, MessageType::Pong, &[]).await?;
                                    }
                                    _ => {
                                        tracing::debug!("Received unexpected message type: {:?}", header.message_type);
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("Error reading frame: {}", e);
                                break;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::debug!("Inbound task received shutdown signal");
                        break;
                    }
                }
            }
            Ok(())
        })
    }

    /// Spawn the keepalive task to send periodic pings.
    fn spawn_keepalive_task(
        stream: Arc<Mutex<TlsStream<TcpStream>>>,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = write_frame(&mut *stream.lock().await, MessageType::Ping, &[]).await {
                            tracing::error!("Failed to send ping: {}", e);
                            break;
                        }
                        tracing::debug!("Sent keepalive ping");
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::debug!("Keepalive task received shutdown signal");
                        break;
                    }
                }
            }
            Ok(())
        })
    }

    /// Convert a file event to a sync operation.
    async fn event_to_sync_op(
        event: &FileEvent,
        local_index: &Arc<Mutex<FileIndex>>,
    ) -> Result<Option<SyncOp>> {
        match event.kind {
            FileEventKind::Created => {
                let metadata =
                    tokio::fs::metadata(event.path.to_path(&std::path::PathBuf::new())).await?;
                let kind = if metadata.is_dir() {
                    FileKind::Directory
                } else if metadata.is_file() {
                    FileKind::File
                } else {
                    FileKind::Symlink
                };

                let (size, hash) = if kind == FileKind::File {
                    let data =
                        tokio::fs::read(event.path.to_path(&std::path::PathBuf::new())).await?;
                    (data.len() as u64, crate::crypto::xxhash64(&data))
                } else {
                    (0, 0)
                };

                Ok(Some(SyncOp::Create {
                    path: event.path.clone(),
                    kind,
                    size,
                    content_hash: hash,
                }))
            }
            FileEventKind::Modified => {
                let data = tokio::fs::read(event.path.to_path(&std::path::PathBuf::new())).await?;
                let hash = crate::crypto::xxhash64(&data);

                Ok(Some(SyncOp::Modify {
                    path: event.path.clone(),
                    size: data.len() as u64,
                    content_hash: hash,
                }))
            }
            FileEventKind::Deleted => {
                let entry = local_index.lock().await.remove(&event.path);
                let kind = entry.map_or(FileKind::File, |e| e.kind);

                Ok(Some(SyncOp::Delete {
                    path: event.path.clone(),
                    kind,
                }))
            }
        }
    }

    /// Send a sync operation to the peer (simplified for live sync).
    async fn send_sync_op<S>(
        stream: &mut S,
        op: &SyncOp,
        op_id: u64,
        config: &SyncConfig,
    ) -> Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let (op_type, path, from_path, kind, size, content_hash) = match op {
            SyncOp::Create {
                path,
                kind,
                size,
                content_hash,
            } => (
                SyncOpType::Create,
                path.as_str().to_string(),
                None,
                *kind as u8,
                Some(*size),
                Some(*content_hash),
            ),
            SyncOp::Modify {
                path,
                size,
                content_hash,
            } => (
                SyncOpType::Modify,
                path.as_str().to_string(),
                None,
                FileKind::File as u8,
                Some(*size),
                Some(*content_hash),
            ),
            SyncOp::Delete { path, kind } => (
                SyncOpType::Delete,
                path.as_str().to_string(),
                None,
                *kind as u8,
                None,
                None,
            ),
            SyncOp::Rename { from, to, kind } => (
                SyncOpType::Rename,
                to.as_str().to_string(),
                Some(from.as_str().to_string()),
                *kind as u8,
                None,
                None,
            ),
        };

        #[allow(clippy::cast_possible_truncation)]
        let chunk_count = size.map(|file_size| {
            if file_size > 0 {
                file_size.div_ceil(DEFAULT_CHUNK_SIZE as u64) as u32
            } else {
                0
            }
        });

        let payload = SyncOpPayload {
            op_id,
            op_type,
            path,
            from_path,
            kind,
            size,
            content_hash,
            chunk_count,
        };

        write_frame(stream, MessageType::SyncOp, &encode_payload(&payload)?).await?;

        if let (Some(file_size), true) = (
            size,
            matches!(op_type, SyncOpType::Create | SyncOpType::Modify),
        ) {
            if file_size > 0 {
                let file_path = match op {
                    SyncOp::Create { path, .. } | SyncOp::Modify { path, .. } => {
                        path.to_path(&config.sync_root)
                    }
                    _ => unreachable!(),
                };

                Self::send_file_chunks_simple(stream, op_id, &file_path).await?;
            }
        }

        let (header, ack_payload) = read_frame(stream).await?;
        if header.message_type != MessageType::SyncOpAck {
            return Err(Error::ProtocolError("expected SyncOpAck".to_string()));
        }

        let ack: SyncOpAckPayload = decode_payload(&ack_payload)?;
        if !ack.success {
            return Err(Error::SyncOperationFailed(
                ack.error.unwrap_or_else(|| "unknown error".to_string()),
            ));
        }

        Ok(())
    }

    /// Send file chunks (simplified version).
    async fn send_file_chunks_simple<S>(
        stream: &mut S,
        op_id: u64,
        file_path: &std::path::Path,
    ) -> Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let chunker = FileChunker::new(DEFAULT_CHUNK_SIZE);
        let chunks = chunker.read_chunks(file_path, 0).await?;

        for chunk in chunks {
            #[allow(clippy::cast_possible_truncation)]
            let chunk_payload = SyncChunkPayload {
                op_id,
                chunk_index: chunk.chunk_index as u32,
                data: chunk.data.clone(),
                checksum: chunk.checksum,
            };

            write_frame(
                stream,
                MessageType::SyncChunk,
                &encode_sync_chunk(&chunk_payload),
            )
            .await?;

            let (header, ack_data) = read_frame(stream).await?;
            if header.message_type != MessageType::SyncChunkAck {
                return Err(Error::ProtocolError("expected SyncChunkAck".to_string()));
            }

            let ack: SyncChunkAckPayload = decode_payload(&ack_data)?;
            if !ack.success {
                return Err(Error::SyncOperationFailed(
                    "chunk transfer failed".to_string(),
                ));
            }
        }

        let complete = SyncCompletePayload {
            op_id,
            content_hash: 0,
        };
        write_frame(
            stream,
            MessageType::SyncComplete,
            &encode_payload(&complete)?,
        )
        .await?;

        Ok(())
    }

    /// Receive a sync operation from the peer (simplified for live sync).
    async fn receive_sync_op<S, F>(
        stream: &mut S,
        payload: SyncOpPayload,
        config: &SyncConfig,
        stats: &Arc<Mutex<SyncStats>>,
        event_callback: &mut F,
    ) -> Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
        F: FnMut(SyncEvent),
    {
        let op_id = payload.op_id;

        match payload.op_type {
            SyncOpType::Create | SyncOpType::Modify => {
                if let Some(size) = payload.size {
                    let path = RelativePath::new(&payload.path);
                    let abs_path = path.to_path(&config.sync_root);

                    if let Some(parent) = abs_path.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }

                    event_callback(SyncEvent::FileReceiving {
                        path: payload.path.clone(),
                        size,
                    });

                    if size > 0 && payload.chunk_count.unwrap_or(0) > 0 {
                        Self::receive_file_chunks_simple(stream, op_id, &abs_path, size).await?;
                    } else {
                        tokio::fs::File::create(&abs_path).await?;
                    }

                    event_callback(SyncEvent::FileReceived {
                        path: payload.path.clone(),
                    });

                    let mut stats_guard = stats.lock().await;
                    stats_guard.files_received += 1;
                    stats_guard.bytes_received += size;
                }
            }
            SyncOpType::Delete => {
                if config.sync_deletions {
                    let path = RelativePath::new(&payload.path);
                    let abs_path = path.to_path(&config.sync_root);

                    if abs_path.exists() {
                        if abs_path.is_dir() {
                            tokio::fs::remove_dir_all(&abs_path).await?;
                        } else {
                            tokio::fs::remove_file(&abs_path).await?;
                        }
                    }

                    event_callback(SyncEvent::FileDeleted {
                        path: payload.path.clone(),
                    });
                }
            }
            SyncOpType::Rename => {
                if let Some(from_path_str) = payload.from_path {
                    let from_path = RelativePath::new(&from_path_str);
                    let to_path = RelativePath::new(&payload.path);
                    let from_abs = from_path.to_path(&config.sync_root);
                    let to_abs = to_path.to_path(&config.sync_root);

                    if let Some(parent) = to_abs.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }

                    tokio::fs::rename(&from_abs, &to_abs).await?;
                }
            }
        }

        let ack = SyncOpAckPayload {
            op_id,
            success: true,
            error: None,
            content_hash: None,
        };
        write_frame(stream, MessageType::SyncOpAck, &encode_payload(&ack)?).await?;

        Ok(())
    }

    /// Receive file chunks (simplified version).
    async fn receive_file_chunks_simple<S>(
        stream: &mut S,
        op_id: u64,
        output_path: &std::path::Path,
        expected_size: u64,
    ) -> Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        use crate::crypto::xxhash64;

        let mut writer = FileWriter::new(output_path.to_path_buf(), expected_size).await?;

        loop {
            let (header, chunk_data) = read_frame(stream).await?;

            match header.message_type {
                MessageType::SyncChunk => {
                    let chunk_payload = decode_sync_chunk(&chunk_data)?;

                    if chunk_payload.op_id != op_id {
                        return Err(Error::ProtocolError(format!(
                            "chunk op_id mismatch: expected {}, got {}",
                            op_id, chunk_payload.op_id
                        )));
                    }

                    let computed_checksum = xxhash64(&chunk_payload.data);
                    if computed_checksum != chunk_payload.checksum {
                        let ack = SyncChunkAckPayload {
                            op_id,
                            chunk_index: chunk_payload.chunk_index,
                            success: false,
                        };
                        write_frame(stream, MessageType::SyncChunkAck, &encode_payload(&ack)?)
                            .await?;
                        return Err(Error::ChecksumMismatch {
                            file: output_path.display().to_string(),
                            chunk: u64::from(chunk_payload.chunk_index),
                        });
                    }

                    let file_chunk = FileChunk {
                        file_index: 0,
                        chunk_index: u64::from(chunk_payload.chunk_index),
                        data: chunk_payload.data,
                        checksum: chunk_payload.checksum,
                        is_last: false,
                    };

                    writer.write_chunk(&file_chunk).await?;

                    let ack = SyncChunkAckPayload {
                        op_id,
                        chunk_index: chunk_payload.chunk_index,
                        success: true,
                    };
                    write_frame(stream, MessageType::SyncChunkAck, &encode_payload(&ack)?).await?;
                }
                MessageType::SyncComplete => {
                    let _complete: SyncCompletePayload = decode_payload(&chunk_data)?;
                    writer.finalize().await?;
                    break;
                }
                _ => {
                    return Err(Error::UnexpectedMessage {
                        expected: "SyncChunk or SyncComplete".to_string(),
                        actual: format!("{:?}", header.message_type),
                    });
                }
            }
        }

        Ok(())
    }
}

/// Helper to configure TCP keepalive on a socket.
fn configure_tcp_keepalive(stream: &TcpStream) -> Result<()> {
    use socket2::{SockRef, TcpKeepalive};

    let socket_ref = SockRef::from(stream);
    let keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(10))
        .with_interval(Duration::from_secs(5));

    socket_ref
        .set_tcp_keepalive(&keepalive)
        .map_err(|e| Error::Io(std::io::Error::other(e)))?;

    tracing::debug!("TCP keep-alive enabled on socket");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sync_session_debug() {
        let temp_dir = TempDir::new().unwrap();
        let config = SyncConfig {
            sync_root: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let session = SyncSession {
            config,
            transfer_config: TransferConfig::default(),
            local_index: FileIndex::default(),
            remote_index: FileIndex::default(),
            peer_name: Some("TestPeer".to_string()),
            stats: SyncStats::new(),
            sync_engine: SyncEngine::new(ResolutionStrategy::default()),
            op_id_counter: 0,
            tls_stream: None,
            session_start: Instant::now(),
        };

        let debug_str = format!("{session:?}");
        assert!(debug_str.contains("SyncSession"));
    }

    #[test]
    fn test_sync_session_peer_name() {
        let temp_dir = TempDir::new().unwrap();
        let config = SyncConfig {
            sync_root: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let session = SyncSession {
            config,
            transfer_config: TransferConfig::default(),
            local_index: FileIndex::default(),
            remote_index: FileIndex::default(),
            peer_name: Some("TestPeer".to_string()),
            stats: SyncStats::new(),
            sync_engine: SyncEngine::new(ResolutionStrategy::default()),
            op_id_counter: 0,
            tls_stream: None,
            session_start: Instant::now(),
        };

        assert_eq!(session.peer_name(), "TestPeer");
    }

    #[test]
    fn test_sync_session_peer_name_unknown() {
        let temp_dir = TempDir::new().unwrap();
        let config = SyncConfig {
            sync_root: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let session = SyncSession {
            config,
            transfer_config: TransferConfig::default(),
            local_index: FileIndex::default(),
            remote_index: FileIndex::default(),
            peer_name: None,
            stats: SyncStats::new(),
            sync_engine: SyncEngine::new(ResolutionStrategy::default()),
            op_id_counter: 0,
            tls_stream: None,
            session_start: Instant::now(),
        };

        assert_eq!(session.peer_name(), "Unknown");
    }

    #[test]
    fn test_sync_session_stats() {
        let temp_dir = TempDir::new().unwrap();
        let config = SyncConfig {
            sync_root: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let session = SyncSession {
            config,
            transfer_config: TransferConfig::default(),
            local_index: FileIndex::default(),
            remote_index: FileIndex::default(),
            peer_name: None,
            stats: SyncStats::new(),
            sync_engine: SyncEngine::new(ResolutionStrategy::default()),
            op_id_counter: 0,
            tls_stream: None,
            session_start: Instant::now(),
        };

        let stats = session.stats();
        assert_eq!(stats.files_sent, 0);
        assert_eq!(stats.files_received, 0);
    }

    #[test]
    fn test_sync_session_next_op_id() {
        let temp_dir = TempDir::new().unwrap();
        let config = SyncConfig {
            sync_root: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let mut session = SyncSession {
            config,
            transfer_config: TransferConfig::default(),
            local_index: FileIndex::default(),
            remote_index: FileIndex::default(),
            peer_name: None,
            stats: SyncStats::new(),
            sync_engine: SyncEngine::new(ResolutionStrategy::default()),
            op_id_counter: 0,
            tls_stream: None,
            session_start: Instant::now(),
        };

        assert_eq!(session.next_op_id(), 0);
        assert_eq!(session.next_op_id(), 1);
        assert_eq!(session.next_op_id(), 2);
    }

    #[test]
    fn test_sync_event_debug() {
        let event = SyncEvent::Connected {
            peer_name: "TestPeer".to_string(),
        };
        let debug_str = format!("{event:?}");
        assert!(debug_str.contains("Connected"));
        assert!(debug_str.contains("TestPeer"));
    }
}
