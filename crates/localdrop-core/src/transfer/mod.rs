//! File transfer engine for LocalDrop.
//!
//! This module handles the actual transfer of files between devices:
//!
//! - Establishing connections
//! - Chunked file transfers
//! - Progress tracking
//! - Resume support
//!
//! ## Transfer Protocol
//!
//! - Default chunk size: 1MB
//! - Adaptive sizing based on network conditions
//! - Parallel chunks: Up to 4 concurrent streams
//! - Checksum: xxHash64 per chunk, SHA-256 for complete file

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio_rustls::{TlsAcceptor, TlsConnector};
use uuid::Uuid;

use crate::code::{CodeGenerator, ShareCode};
use crate::crypto::{self, TlsConfig};
use crate::discovery::{Broadcaster, DiscoveryPacket, Listener, DEFAULT_DISCOVERY_PORT};
use crate::error::{Error, Result};
use crate::file::{
    enumerate_files, EnumerateOptions, FileChunk, FileChunker, FileMetadata, FileWriter,
};
use crate::protocol::{
    self, ChunkAckPayload, ChunkDataPayload, ChunkStartPayload, CodeVerifyAckPayload,
    CodeVerifyPayload, FileListAckPayload, FileListPayload, HelloPayload, MessageType,
};

/// Default transfer port.
pub const DEFAULT_TRANSFER_PORT: u16 = 52530;

/// Transfer direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    /// Sending files
    Send,
    /// Receiving files
    Receive,
}

/// Transfer state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferState {
    /// Preparing transfer
    Preparing,
    /// Waiting for connection
    Waiting,
    /// Connected, awaiting confirmation
    Connected,
    /// Transferring files
    Transferring,
    /// Transfer completed successfully
    Completed,
    /// Transfer was cancelled
    Cancelled,
    /// Transfer failed
    Failed,
}

/// Progress information for a transfer.
#[derive(Debug, Clone)]
pub struct TransferProgress {
    /// Current state
    pub state: TransferState,
    /// Current file index
    pub current_file: usize,
    /// Total number of files
    pub total_files: usize,
    /// Current file name
    pub current_file_name: String,
    /// Bytes transferred for current file
    pub file_bytes_transferred: u64,
    /// Total bytes for current file
    pub file_total_bytes: u64,
    /// Total bytes transferred across all files
    pub total_bytes_transferred: u64,
    /// Total bytes across all files
    pub total_bytes: u64,
    /// Transfer speed in bytes per second
    pub speed_bps: u64,
    /// Estimated time remaining
    pub eta: Option<Duration>,
    /// When transfer started
    pub started_at: Instant,
}

impl TransferProgress {
    /// Create a new progress tracker.
    #[must_use]
    pub fn new(total_files: usize, total_bytes: u64) -> Self {
        Self {
            state: TransferState::Preparing,
            current_file: 0,
            total_files,
            current_file_name: String::new(),
            file_bytes_transferred: 0,
            file_total_bytes: 0,
            total_bytes_transferred: 0,
            total_bytes,
            speed_bps: 0,
            eta: None,
            started_at: Instant::now(),
        }
    }

    /// Get overall progress as a percentage (0.0 - 100.0).
    #[must_use]
    pub fn percentage(&self) -> f64 {
        if self.total_bytes == 0 {
            100.0
        } else {
            (self.total_bytes_transferred as f64 / self.total_bytes as f64) * 100.0
        }
    }
}

/// Configuration for a transfer session.
#[derive(Debug, Clone)]
pub struct TransferConfig {
    /// Chunk size in bytes
    pub chunk_size: usize,
    /// Number of parallel streams
    pub parallel_streams: usize,
    /// Bandwidth limit (bytes per second)
    pub bandwidth_limit: Option<u64>,
    /// Enable compression
    pub compress: bool,
    /// Verify checksums
    pub verify_checksums: bool,
    /// Discovery port
    pub discovery_port: u16,
    /// Transfer port
    pub transfer_port: u16,
    /// Discovery timeout
    pub discovery_timeout: Duration,
    /// Broadcast interval for discovery announcements
    pub broadcast_interval: Duration,
}

impl Default for TransferConfig {
    fn default() -> Self {
        Self {
            chunk_size: crate::DEFAULT_CHUNK_SIZE,
            parallel_streams: crate::DEFAULT_PARALLEL_CHUNKS,
            bandwidth_limit: None,
            compress: false,
            verify_checksums: true,
            discovery_port: DEFAULT_DISCOVERY_PORT,
            transfer_port: DEFAULT_TRANSFER_PORT,
            discovery_timeout: Duration::from_secs(30),
            broadcast_interval: Duration::from_secs(2),
        }
    }
}

/// A share session (sender side).
pub struct ShareSession {
    /// Share code
    code: ShareCode,
    /// Files being shared
    files: Vec<FileMetadata>,
    /// File paths (for reading)
    file_paths: Vec<PathBuf>,
    /// Transfer configuration
    config: TransferConfig,
    /// Device name
    device_name: String,
    /// Device ID (reserved for future use)
    _device_id: Uuid,
    /// Session key for HMAC verification
    session_key: [u8; 32],
    /// Progress sender
    progress_tx: watch::Sender<TransferProgress>,
    /// Progress receiver (for cloning to observers)
    progress_rx: watch::Receiver<TransferProgress>,
    /// TCP listener
    listener: TcpListener,
    /// TLS config
    tls_config: TlsConfig,
    /// UDP broadcaster
    broadcaster: Broadcaster,
}

impl std::fmt::Debug for ShareSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShareSession")
            .field("code", &self.code)
            .field("files", &self.files)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl ShareSession {
    /// Create a new share session.
    ///
    /// # Arguments
    ///
    /// * `paths` - Paths to share
    /// * `config` - Transfer configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the session cannot be created.
    pub async fn new(paths: &[PathBuf], config: TransferConfig) -> Result<Self> {
        let code = CodeGenerator::new().generate()?;

        let options = EnumerateOptions::default();
        let files = enumerate_files(paths, &options)?;

        if files.is_empty() {
            return Err(Error::FileNotFound("no files to share".to_string()));
        }

        let file_paths: Vec<PathBuf> = paths.to_vec();

        let total_bytes: u64 = files.iter().map(|f| f.size).sum();
        let progress = TransferProgress::new(files.len(), total_bytes);
        let (progress_tx, progress_rx) = watch::channel(progress);

        let session_key = crypto::derive_session_key(code.as_str());

        let tls_config = TlsConfig::server()?;

        let listener = TcpListener::bind(format!("0.0.0.0:{}", config.transfer_port)).await?;
        let local_addr = listener.local_addr()?;

        let device_name = hostname::get().map_or_else(
            |_| "Unknown".to_string(),
            |h| h.to_string_lossy().to_string(),
        );

        let broadcaster = Broadcaster::new(config.discovery_port).await?;

        let device_id = Uuid::new_v4();
        let packet = DiscoveryPacket::new(
            &code,
            &device_name,
            device_id,
            local_addr.port(),
            files.len(),
            total_bytes,
        );

        broadcaster.start(packet, config.broadcast_interval).await?;

        Ok(Self {
            code,
            files,
            file_paths,
            config,
            device_name,
            _device_id: device_id,
            session_key,
            progress_tx,
            progress_rx,
            listener,
            tls_config,
            broadcaster,
        })
    }

    /// Get the share code.
    #[must_use]
    pub fn code(&self) -> &ShareCode {
        &self.code
    }

    /// Get the files being shared.
    #[must_use]
    pub fn files(&self) -> &[FileMetadata] {
        &self.files
    }

    /// Get a progress receiver.
    #[must_use]
    pub fn progress(&self) -> watch::Receiver<TransferProgress> {
        self.progress_rx.clone()
    }

    /// Wait for a receiver to connect and complete the transfer.
    ///
    /// # Errors
    ///
    /// Returns an error if the transfer fails.
    pub async fn wait(&mut self) -> Result<()> {
        self.update_state(TransferState::Waiting);

        let (stream, peer_addr) = self.listener.accept().await?;
        tracing::info!("Connection from {}", peer_addr);

        let acceptor = TlsAcceptor::from(Arc::new(
            self.tls_config
                .server_config()
                .ok_or_else(|| Error::TlsError("no server config".to_string()))?
                .clone(),
        ));
        let mut tls_stream = acceptor
            .accept(stream)
            .await
            .map_err(|e| Error::TlsError(format!("TLS handshake failed: {e}")))?;

        self.update_state(TransferState::Connected);

        self.do_handshake(&mut tls_stream).await?;

        self.do_code_verification(&mut tls_stream).await?;

        let accepted = self.do_file_list_exchange(&mut tls_stream).await?;
        if !accepted {
            self.update_state(TransferState::Cancelled);
            return Err(Error::TransferRejected);
        }

        self.update_state(TransferState::Transferring);

        self.do_transfer(&mut tls_stream).await?;

        self.broadcaster.stop().await;

        self.update_state(TransferState::Completed);

        Ok(())
    }

    /// Cancel the share session.
    pub async fn cancel(&mut self) {
        self.broadcaster.stop().await;
        self.update_state(TransferState::Cancelled);
    }

    fn update_state(&self, state: TransferState) {
        let mut progress = self.progress_rx.borrow().clone();
        progress.state = state;
        let _ = self.progress_tx.send(progress);
    }

    async fn do_handshake<S>(&self, stream: &mut S) -> Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let hello = HelloPayload {
            device_name: self.device_name.clone(),
            protocol_version: "1.0".to_string(),
        };
        let payload = protocol::encode_payload(&hello)?;
        protocol::write_frame(stream, MessageType::Hello, &payload).await?;

        let (header, _payload) = protocol::read_frame(stream).await?;
        if header.message_type != MessageType::HelloAck {
            return Err(Error::UnexpectedMessage {
                expected: "HelloAck".to_string(),
                actual: format!("{:?}", header.message_type),
            });
        }

        Ok(())
    }

    async fn do_code_verification<S>(&self, stream: &mut S) -> Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let (header, payload) = protocol::read_frame(stream).await?;
        if header.message_type != MessageType::CodeVerify {
            return Err(Error::UnexpectedMessage {
                expected: "CodeVerify".to_string(),
                actual: format!("{:?}", header.message_type),
            });
        }

        let verify: CodeVerifyPayload = protocol::decode_payload(&payload)?;

        let expected_hmac = crypto::hmac_sha256(&self.session_key, self.code.as_str().as_bytes());

        let success = crypto::constant_time_eq(&verify.code_hmac, &expected_hmac);

        let ack = CodeVerifyAckPayload {
            success,
            error: if success {
                None
            } else {
                Some("Invalid code".to_string())
            },
        };
        let ack_payload = protocol::encode_payload(&ack)?;
        protocol::write_frame(stream, MessageType::CodeVerifyAck, &ack_payload).await?;

        if !success {
            return Err(Error::CodeNotFound(self.code.to_string()));
        }

        Ok(())
    }

    async fn do_file_list_exchange<S>(&self, stream: &mut S) -> Result<bool>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let file_list = FileListPayload {
            files: self.files.clone(),
            total_size: self.files.iter().map(|f| f.size).sum(),
        };
        let payload = protocol::encode_payload(&file_list)?;
        protocol::write_frame(stream, MessageType::FileList, &payload).await?;

        let (header, ack_payload) = protocol::read_frame(stream).await?;
        if header.message_type != MessageType::FileListAck {
            return Err(Error::UnexpectedMessage {
                expected: "FileListAck".to_string(),
                actual: format!("{:?}", header.message_type),
            });
        }

        let ack: FileListAckPayload = protocol::decode_payload(&ack_payload)?;
        Ok(ack.accepted)
    }

    async fn do_transfer<S>(&self, stream: &mut S) -> Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let chunker = FileChunker::new(self.config.chunk_size);

        for (file_index, file) in self.files.iter().enumerate() {
            {
                let mut progress = self.progress_rx.borrow().clone();
                progress.current_file = file_index;
                progress.current_file_name = file.file_name().to_string();
                progress.file_bytes_transferred = 0;
                progress.file_total_bytes = file.size;
                let _ = self.progress_tx.send(progress);
            }

            let file_path = self.find_file_path(&file.relative_path)?;

            let chunks = chunker.read_chunks(&file_path, file_index).await?;
            let total_chunks = chunks.len() as u64;

            for chunk in chunks {
                let start = ChunkStartPayload {
                    file_index,
                    chunk_index: chunk.chunk_index,
                    total_chunks,
                };
                let start_payload = protocol::encode_payload(&start)?;
                protocol::write_frame(stream, MessageType::ChunkStart, &start_payload).await?;

                let data = ChunkDataPayload {
                    file_index,
                    chunk_index: chunk.chunk_index,
                    data: chunk.data.clone(),
                    checksum: chunk.checksum,
                };
                let data_payload = protocol::encode_chunk_data(&data);
                protocol::write_frame(stream, MessageType::ChunkData, &data_payload).await?;

                let (header, ack_payload) = protocol::read_frame(stream).await?;
                if header.message_type != MessageType::ChunkAck {
                    return Err(Error::UnexpectedMessage {
                        expected: "ChunkAck".to_string(),
                        actual: format!("{:?}", header.message_type),
                    });
                }

                let ack: ChunkAckPayload = protocol::decode_payload(&ack_payload)?;
                if !ack.success {
                    return Err(Error::ChecksumMismatch {
                        file: file.file_name().to_string(),
                        chunk: chunk.chunk_index,
                    });
                }

                {
                    let mut progress = self.progress_rx.borrow().clone();
                    progress.file_bytes_transferred += chunk.data.len() as u64;
                    progress.total_bytes_transferred += chunk.data.len() as u64;
                    let elapsed = progress.started_at.elapsed().as_secs_f64();
                    if elapsed > 0.0 {
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        {
                            progress.speed_bps =
                                (progress.total_bytes_transferred as f64 / elapsed) as u64;
                        }
                        let remaining = progress.total_bytes - progress.total_bytes_transferred;
                        if progress.speed_bps > 0 {
                            progress.eta =
                                Some(Duration::from_secs(remaining / progress.speed_bps));
                        }
                    }
                    let _ = self.progress_tx.send(progress);
                }
            }
        }

        protocol::write_frame(stream, MessageType::TransferComplete, &[]).await?;

        Ok(())
    }

    fn find_file_path(&self, relative_path: &Path) -> Result<PathBuf> {
        if self.file_paths.len() == 1 && self.file_paths[0].is_file() {
            return Ok(self.file_paths[0].clone());
        }

        for file_path in &self.file_paths {
            if file_path.is_file() {
                if let Some(name) = file_path.file_name() {
                    if name == relative_path.as_os_str() || file_path.ends_with(relative_path) {
                        return Ok(file_path.clone());
                    }
                }
            }
        }

        for base_path in &self.file_paths {
            if base_path.is_dir() {
                let full_path = base_path.join(relative_path);
                if full_path.exists() {
                    return Ok(full_path);
                }
            }
        }

        Err(Error::FileNotFound(relative_path.display().to_string()))
    }
}

/// A receive session (receiver side).
pub struct ReceiveSession {
    /// Sender information
    sender_addr: SocketAddr,
    /// Sender device name
    sender_name: String,
    /// Files being received
    files: Vec<FileMetadata>,
    /// Output directory
    output_dir: PathBuf,
    /// Transfer configuration (reserved for future use)
    _config: TransferConfig,
    /// Share code (reserved for future use)
    _code: ShareCode,
    /// Session key for HMAC verification (reserved for future use)
    _session_key: [u8; 32],
    /// Progress sender
    progress_tx: watch::Sender<TransferProgress>,
    /// Progress receiver
    progress_rx: watch::Receiver<TransferProgress>,
    /// TLS stream (stored after connect)
    tls_stream: Option<tokio_rustls::client::TlsStream<TcpStream>>,
}

impl std::fmt::Debug for ReceiveSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReceiveSession")
            .field("sender_addr", &self.sender_addr)
            .field("sender_name", &self.sender_name)
            .field("files", &self.files)
            .field("output_dir", &self.output_dir)
            .finish_non_exhaustive()
    }
}

impl ReceiveSession {
    /// Connect to a sender using a share code.
    ///
    /// # Arguments
    ///
    /// * `code` - The share code
    /// * `output_dir` - Directory to save files
    /// * `config` - Transfer configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails.
    pub async fn connect(
        code: &ShareCode,
        output_dir: PathBuf,
        config: TransferConfig,
    ) -> Result<Self> {
        let listener = Listener::new(config.discovery_port).await?;
        let discovered = listener.find(code, config.discovery_timeout).await?;

        tracing::info!(
            "Found share from {} at {}",
            discovered.packet.device_name,
            discovered.source
        );

        let transfer_addr =
            SocketAddr::new(discovered.source.ip(), discovered.packet.transfer_port);
        let stream = TcpStream::connect(transfer_addr).await?;

        let tls_config = TlsConfig::client()?;
        let connector = TlsConnector::from(Arc::new(
            tls_config
                .client_config()
                .ok_or_else(|| Error::TlsError("no client config".to_string()))?
                .clone(),
        ));
        let mut tls_stream = connector
            .connect("localhost".try_into().unwrap(), stream)
            .await
            .map_err(|e| Error::TlsError(format!("TLS handshake failed: {e}")))?;

        let session_key = crypto::derive_session_key(code.as_str());

        Self::do_handshake(&mut tls_stream).await?;

        Self::do_code_verification(&mut tls_stream, code, &session_key).await?;

        let files = Self::receive_file_list(&mut tls_stream).await?;

        let total_bytes: u64 = files.iter().map(|f| f.size).sum();
        let progress = TransferProgress::new(files.len(), total_bytes);
        let (progress_tx, progress_rx) = watch::channel(progress);

        Ok(Self {
            sender_addr: transfer_addr,
            sender_name: discovered.packet.device_name,
            files,
            output_dir,
            _config: config,
            _code: code.clone(),
            _session_key: session_key,
            progress_tx,
            progress_rx,
            tls_stream: Some(tls_stream),
        })
    }

    /// Get sender information.
    #[must_use]
    pub fn sender(&self) -> (&SocketAddr, &str) {
        (&self.sender_addr, &self.sender_name)
    }

    /// Get the files being received.
    #[must_use]
    pub fn files(&self) -> &[FileMetadata] {
        &self.files
    }

    /// Get the output directory.
    #[must_use]
    pub fn output_dir(&self) -> &PathBuf {
        &self.output_dir
    }

    /// Get a progress receiver.
    #[must_use]
    pub fn progress(&self) -> watch::Receiver<TransferProgress> {
        self.progress_rx.clone()
    }

    /// Accept the transfer and start receiving.
    ///
    /// # Errors
    ///
    /// Returns an error if the transfer fails.
    pub async fn accept(&mut self) -> Result<()> {
        let mut stream = self
            .tls_stream
            .take()
            .ok_or_else(|| Error::Internal("no TLS stream".to_string()))?;

        let ack = FileListAckPayload {
            accepted: true,
            accepted_files: None,
        };
        let ack_payload = protocol::encode_payload(&ack)?;
        protocol::write_frame(&mut stream, MessageType::FileListAck, &ack_payload).await?;

        self.update_state(TransferState::Transferring);

        self.do_receive(&mut stream).await?;

        self.update_state(TransferState::Completed);

        Ok(())
    }

    /// Accept specific files only.
    ///
    /// # Arguments
    ///
    /// * `indices` - Indices of files to accept
    ///
    /// # Errors
    ///
    /// Returns an error if the transfer fails.
    pub async fn accept_files(&mut self, indices: &[usize]) -> Result<()> {
        let mut stream = self
            .tls_stream
            .take()
            .ok_or_else(|| Error::Internal("no TLS stream".to_string()))?;

        let ack = FileListAckPayload {
            accepted: true,
            accepted_files: Some(indices.to_vec()),
        };
        let ack_payload = protocol::encode_payload(&ack)?;
        protocol::write_frame(&mut stream, MessageType::FileListAck, &ack_payload).await?;

        self.update_state(TransferState::Transferring);

        self.do_receive(&mut stream).await?;

        self.update_state(TransferState::Completed);

        Ok(())
    }

    /// Decline the transfer.
    pub async fn decline(&mut self) {
        if let Some(mut stream) = self.tls_stream.take() {
            let ack = FileListAckPayload {
                accepted: false,
                accepted_files: None,
            };
            if let Ok(ack_payload) = protocol::encode_payload(&ack) {
                let _ = protocol::write_frame(&mut stream, MessageType::FileListAck, &ack_payload)
                    .await;
            }
            let _ = stream.shutdown().await;
        }
        self.update_state(TransferState::Cancelled);
    }

    fn update_state(&self, state: TransferState) {
        let mut progress = self.progress_rx.borrow().clone();
        progress.state = state;
        let _ = self.progress_tx.send(progress);
    }

    async fn do_handshake<S>(stream: &mut S) -> Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let (header, payload) = protocol::read_frame(stream).await?;
        if header.message_type != MessageType::Hello {
            return Err(Error::UnexpectedMessage {
                expected: "Hello".to_string(),
                actual: format!("{:?}", header.message_type),
            });
        }

        let _hello: HelloPayload = protocol::decode_payload(&payload)?;

        let device_name = hostname::get().map_or_else(
            |_| "Unknown".to_string(),
            |h| h.to_string_lossy().to_string(),
        );
        let ack = HelloPayload {
            device_name,
            protocol_version: "1.0".to_string(),
        };
        let ack_payload = protocol::encode_payload(&ack)?;
        protocol::write_frame(stream, MessageType::HelloAck, &ack_payload).await?;

        Ok(())
    }

    async fn do_code_verification<S>(
        stream: &mut S,
        code: &ShareCode,
        session_key: &[u8; 32],
    ) -> Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let hmac = crypto::hmac_sha256(session_key, code.as_str().as_bytes());
        let verify = CodeVerifyPayload {
            code_hmac: hmac.to_vec(),
        };
        let payload = protocol::encode_payload(&verify)?;
        protocol::write_frame(stream, MessageType::CodeVerify, &payload).await?;

        let (header, ack_payload) = protocol::read_frame(stream).await?;
        if header.message_type != MessageType::CodeVerifyAck {
            return Err(Error::UnexpectedMessage {
                expected: "CodeVerifyAck".to_string(),
                actual: format!("{:?}", header.message_type),
            });
        }

        let ack: CodeVerifyAckPayload = protocol::decode_payload(&ack_payload)?;
        if !ack.success {
            return Err(Error::CodeNotFound(code.to_string()));
        }

        Ok(())
    }

    async fn receive_file_list<S>(stream: &mut S) -> Result<Vec<FileMetadata>>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let (header, payload) = protocol::read_frame(stream).await?;
        if header.message_type != MessageType::FileList {
            return Err(Error::UnexpectedMessage {
                expected: "FileList".to_string(),
                actual: format!("{:?}", header.message_type),
            });
        }

        let file_list: FileListPayload = protocol::decode_payload(&payload)?;
        Ok(file_list.files)
    }

    async fn do_receive<S>(&self, stream: &mut S) -> Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let mut current_writer: Option<FileWriter> = None;
        let mut current_file_index: Option<usize> = None;

        loop {
            let (header, payload) = protocol::read_frame(stream).await?;

            match header.message_type {
                MessageType::ChunkStart => {
                    let start: ChunkStartPayload = protocol::decode_payload(&payload)?;

                    if current_file_index != Some(start.file_index) {
                        if let Some(writer) = current_writer.take() {
                            let _sha256 = writer.finalize().await?;
                        }

                        let file = &self.files[start.file_index];
                        let output_path = self.output_dir.join(&file.relative_path);
                        current_writer = Some(FileWriter::new(output_path, file.size).await?);
                        current_file_index = Some(start.file_index);

                        {
                            let mut progress = self.progress_rx.borrow().clone();
                            progress.current_file = start.file_index;
                            progress.current_file_name = file.file_name().to_string();
                            progress.file_bytes_transferred = 0;
                            progress.file_total_bytes = file.size;
                            let _ = self.progress_tx.send(progress);
                        }
                    }
                }
                MessageType::ChunkData => {
                    let chunk_data = protocol::decode_chunk_data(&payload)?;

                    let chunk = FileChunk {
                        file_index: chunk_data.file_index,
                        chunk_index: chunk_data.chunk_index,
                        data: chunk_data.data.clone(),
                        checksum: chunk_data.checksum,
                        is_last: false,
                    };

                    let success = if let Some(ref mut writer) = current_writer {
                        writer.write_chunk(&chunk).await.is_ok()
                    } else {
                        false
                    };

                    let ack = ChunkAckPayload {
                        file_index: chunk_data.file_index,
                        chunk_index: chunk_data.chunk_index,
                        success,
                    };
                    let ack_payload = protocol::encode_payload(&ack)?;
                    protocol::write_frame(stream, MessageType::ChunkAck, &ack_payload).await?;

                    if !success {
                        return Err(Error::ChecksumMismatch {
                            file: self.files[chunk_data.file_index].file_name().to_string(),
                            chunk: chunk_data.chunk_index,
                        });
                    }

                    {
                        let mut progress = self.progress_rx.borrow().clone();
                        progress.file_bytes_transferred += chunk_data.data.len() as u64;
                        progress.total_bytes_transferred += chunk_data.data.len() as u64;
                        let elapsed = progress.started_at.elapsed().as_secs_f64();
                        if elapsed > 0.0 {
                            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                            {
                                progress.speed_bps =
                                    (progress.total_bytes_transferred as f64 / elapsed) as u64;
                            }
                            let remaining = progress.total_bytes - progress.total_bytes_transferred;
                            if progress.speed_bps > 0 {
                                progress.eta =
                                    Some(Duration::from_secs(remaining / progress.speed_bps));
                            }
                        }
                        let _ = self.progress_tx.send(progress);
                    }
                }
                MessageType::TransferComplete => {
                    if let Some(writer) = current_writer.take() {
                        let _sha256 = writer.finalize().await?;
                    }
                    break;
                }
                MessageType::TransferCancel => {
                    return Err(Error::TransferCancelled);
                }
                _ => {
                    return Err(Error::UnexpectedMessage {
                        expected: "ChunkStart, ChunkData, or TransferComplete".to_string(),
                        actual: format!("{:?}", header.message_type),
                    });
                }
            }
        }

        Ok(())
    }
}

/// Resume information for interrupted transfers.
#[derive(Debug, Clone)]
pub struct ResumeState {
    /// Transfer ID
    pub transfer_id: uuid::Uuid,
    /// Files in the transfer
    pub files: Vec<FileMetadata>,
    /// Completed chunks per file
    pub completed_chunks: std::collections::HashMap<usize, Vec<u64>>,
    /// Sender device name
    pub sender_device: String,
    /// When transfer started
    pub started_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_transfer_progress() {
        let progress = TransferProgress::new(5, 1000);
        assert_eq!(progress.state, TransferState::Preparing);
        assert_eq!(progress.total_files, 5);
        assert_eq!(progress.total_bytes, 1000);
        assert_eq!(progress.percentage(), 0.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_transfer_progress_percentage() {
        let mut progress = TransferProgress::new(2, 1000);
        progress.total_bytes_transferred = 500;
        assert_eq!(progress.percentage(), 50.0);

        progress.total_bytes_transferred = 1000;
        assert_eq!(progress.percentage(), 100.0);
    }

    #[test]
    fn test_transfer_config_default() {
        let config = TransferConfig::default();
        assert_eq!(config.chunk_size, crate::DEFAULT_CHUNK_SIZE);
        assert!(config.verify_checksums);
        assert_eq!(config.discovery_port, DEFAULT_DISCOVERY_PORT);
    }

    #[tokio::test]
    async fn test_share_session_creation() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "Hello, world!").expect("write file");

        let config = TransferConfig {
            transfer_port: 0,
            discovery_port: 0,
            ..Default::default()
        };

        let session = ShareSession::new(&[test_file], config).await;
        assert!(session.is_ok(), "ShareSession should be created");

        let mut session = session.unwrap();
        assert_eq!(session.files().len(), 1);
        assert_eq!(session.code().as_str().len(), 4);

        session.cancel().await;
    }
}
