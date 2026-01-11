//! HTTP endpoint handlers for the Yoop web interface.
//!
//! This module contains all the handler functions for the REST API endpoints.

#![allow(clippy::missing_errors_doc)]

use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::time::Instant;

use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::Response,
    Json,
};
use axum_extra::extract::Multipart;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio_util::io::ReaderStream;

use crate::code::ShareCode;
use crate::file::FileMetadata;
use crate::history::TransferHistoryEntry;
use crate::transfer::{ReceiveSession, ShareSession, TransferConfig};

use super::error::{ApiError, ApiResult};
use super::state::{
    ActiveReceive, ActiveShare, CompletedReceive, PendingReceive, SharedState, WebMode,
};

// ============================================================================
// Response types
// ============================================================================

/// Status response.
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    /// Current mode
    mode: WebMode,
    /// Share code (if sharing)
    #[serde(skip_serializing_if = "Option::is_none")]
    share_code: Option<String>,
    /// Number of files (if sharing or receiving)
    #[serde(skip_serializing_if = "Option::is_none")]
    file_count: Option<usize>,
    /// Connected device name (if connected)
    #[serde(skip_serializing_if = "Option::is_none")]
    connected_device: Option<String>,
}

/// Network information response.
#[derive(Debug, Serialize)]
pub struct NetworkResponse {
    /// Device name
    device_name: String,
    /// Local IP addresses
    addresses: Vec<String>,
}

/// Share creation response.
#[derive(Debug, Serialize)]
pub struct ShareResponse {
    /// Share code
    code: String,
    /// Expires in seconds
    expires_in: u64,
    /// Files being shared
    files: Vec<FileInfo>,
    /// QR code SVG (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    qr_svg: Option<String>,
    /// Deep link URL for mobile scanning
    deep_link: String,
}

/// File information for responses.
#[derive(Debug, Serialize)]
pub struct FileInfo {
    /// File name
    name: String,
    /// File size in bytes
    size: u64,
    /// Preview information (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    preview: Option<FilePreviewInfo>,
}

/// Preview information for web responses.
#[derive(Debug, Serialize)]
pub struct FilePreviewInfo {
    /// Preview type
    preview_type: String,
    /// MIME type of preview
    mime_type: String,
    /// Preview data (base64 for images, text for text previews, JSON for archives)
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<String>,
    /// Image dimensions (width, height)
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<(u32, u32)>,
    /// Number of files in archive
    #[serde(skip_serializing_if = "Option::is_none")]
    file_count: Option<usize>,
}

impl From<&FileMetadata> for FileInfo {
    fn from(meta: &FileMetadata) -> Self {
        let preview = meta.preview.as_ref().map(|p| {
            let preview_type = match p.preview_type {
                crate::preview::PreviewType::Thumbnail => "thumbnail",
                crate::preview::PreviewType::Text => "text",
                crate::preview::PreviewType::ArchiveListing => "archive",
                crate::preview::PreviewType::Icon => "icon",
                crate::preview::PreviewType::None => "none",
            };
            FilePreviewInfo {
                preview_type: preview_type.to_string(),
                mime_type: p.mime_type.clone(),
                data: if p.data.is_empty() {
                    None
                } else {
                    Some(p.data.clone())
                },
                dimensions: p.metadata.as_ref().and_then(|m| m.dimensions),
                file_count: p.metadata.as_ref().and_then(|m| m.file_count),
            }
        });
        Self {
            name: meta.file_name().to_string(),
            size: meta.size,
            preview,
        }
    }
}

/// Receive connection response.
#[derive(Debug, Serialize)]
pub struct ReceiveResponse {
    /// Sender device name
    sender_name: String,
    /// Sender address
    sender_address: String,
    /// Files offered
    files: Vec<FileInfo>,
    /// Total size in bytes
    total_size: u64,
}

/// Accept response.
#[derive(Debug, Serialize)]
pub struct AcceptResponse {
    /// Success message
    message: String,
}

// ============================================================================
// Request types
// ============================================================================

/// Receive request body.
#[derive(Debug, Deserialize)]
pub struct ReceiveRequest {
    /// Share code
    code: String,
}

// ============================================================================
// Status & Network handlers
// ============================================================================

/// GET /api/status - Get current status.
pub async fn get_status(State(state): State<SharedState>) -> ApiResult<Json<StatusResponse>> {
    let mode = *state.mode.read().await;

    let share_code = match mode {
        WebMode::Sharing => state.current_share_code().await,
        _ => None,
    };

    let (file_count, connected_device) = match mode {
        WebMode::Sharing => {
            let active = state.active_share.lock().await;
            (active.as_ref().map(|s| s.files.len()), None)
        }
        WebMode::Receiving => {
            let pending = state.pending_receive.lock().await;
            pending.as_ref().map_or((None, None), |p| {
                let (_, sender_name) = p.session.sender();
                (Some(p.session.files().len()), Some(sender_name.to_string()))
            })
        }
        WebMode::Transferring => {
            let active_recv = state.active_receive.lock().await;
            active_recv.as_ref().map_or((None, None), |a| {
                let (_, sender_name) = a.session.sender();
                (Some(a.session.files().len()), Some(sender_name.to_string()))
            })
        }
        WebMode::Idle => (None, None),
    };

    Ok(Json(StatusResponse {
        mode,
        share_code,
        file_count,
        connected_device,
    }))
}

/// GET /api/network - Get network information.
pub async fn get_network(State(state): State<SharedState>) -> ApiResult<Json<NetworkResponse>> {
    let addresses = get_local_addresses();

    Ok(Json(NetworkResponse {
        device_name: state.device_name.clone(),
        addresses,
    }))
}

/// Get local network addresses.
fn get_local_addresses() -> Vec<String> {
    let mut addrs = vec!["127.0.0.1".to_string()];

    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(local_addr) = socket.local_addr() {
                addrs.push(local_addr.ip().to_string());
            }
        }
    }

    addrs
}

// ============================================================================
// Share handlers
// ============================================================================

/// POST /api/share - Create a new share from uploaded files.
pub async fn create_share(
    State(state): State<SharedState>,
    mut multipart: Multipart,
) -> ApiResult<Json<ShareResponse>> {
    let mode = *state.mode.read().await;
    if mode != WebMode::Idle {
        return Err(ApiError::conflict("Already in an active session"));
    }

    let upload_dir = state
        .temp_dir
        .join(format!("share-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&upload_dir)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to create upload directory: {e}")))?;

    let mut file_paths: Vec<PathBuf> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::bad_request(format!("Failed to read multipart field: {e}")))?
    {
        let file_name = field
            .file_name()
            .map_or_else(|| format!("file-{}", file_paths.len()), String::from);

        let file_path = upload_dir.join(&file_name);

        let data = field
            .bytes()
            .await
            .map_err(|e| ApiError::bad_request(format!("Failed to read file data: {e}")))?;

        tokio::fs::write(&file_path, &data)
            .await
            .map_err(|e| ApiError::internal(format!("Failed to write file: {e}")))?;

        tracing::info!("Saved uploaded file: {} ({} bytes)", file_name, data.len());
        file_paths.push(file_path);
    }

    if file_paths.is_empty() {
        return Err(ApiError::bad_request("No files uploaded"));
    }

    let config = TransferConfig {
        transfer_port: 0,
        discovery_port: 0,
        ..Default::default()
    };

    let session = ShareSession::new(&file_paths, config)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to create share session: {e}")))?;

    let code = session.code().to_string();
    let files: Vec<FileInfo> = session.files().iter().map(FileInfo::from).collect();
    let file_meta = session.files().to_vec();

    state.set_share_code(code.clone()).await;

    *state.mode.write().await = WebMode::Sharing;
    *state.active_share.lock().await = Some(ActiveShare {
        session,
        files: file_meta,
        created_at: Instant::now(),
    });

    let state_clone = state.clone();
    tokio::spawn(async move {
        share_wait_task(state_clone).await;
    });

    let deep_link = crate::qr::create_deep_link(&code, &crate::qr::QrConfig::default());
    let qr_svg = crate::qr::generate_svg(&code).ok();

    Ok(Json(ShareResponse {
        code,
        expires_in: 300,
        files,
        qr_svg,
        deep_link,
    }))
}

/// Background task that waits for a receiver to connect.
async fn share_wait_task(state: SharedState) {
    let progress_rx = {
        let active_guard = state.active_share.lock().await;
        match active_guard.as_ref() {
            Some(active) => active.session.progress(),
            None => return,
        }
    };

    let mut progress_rx = progress_rx;
    let state_clone = state.clone();

    let progress_task = tokio::spawn(async move {
        while progress_rx.changed().await.is_ok() {
            let progress = progress_rx.borrow().clone();
            state_clone.update_progress(progress);
        }
    });

    let session = {
        let mut active_guard = state.active_share.lock().await;
        active_guard.take().map(|a| a.session)
    };

    if let Some(mut session) = session {
        match session.wait().await {
            Ok(()) => {
                tracing::info!("Share transfer completed");
                state.mark_complete().await;
            }
            Err(e) => {
                tracing::error!("Share transfer failed: {}", e);
                state.mark_failed(&e.to_string()).await;
            }
        }
    }

    progress_task.abort();
    state.reset_to_idle().await;
}

/// DELETE /api/share - Cancel the current share.
pub async fn cancel_share(State(state): State<SharedState>) -> ApiResult<StatusCode> {
    let mode = *state.mode.read().await;
    if mode != WebMode::Sharing {
        return Err(ApiError::conflict("Not currently sharing"));
    }

    let active = state.active_share.lock().await.take();
    if let Some(mut active) = active {
        active.session.cancel().await;
    }

    state.reset_to_idle().await;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/share/code - Get the current share code.
pub async fn get_share_code(
    State(state): State<SharedState>,
) -> ApiResult<Json<serde_json::Value>> {
    state.current_share_code().await.map_or_else(
        || Err(ApiError::not_found("No active share")),
        |c| Ok(Json(serde_json::json!({ "code": c }))),
    )
}

/// GET /api/share/qr - Get QR code SVG for current share.
pub async fn get_share_qr(State(state): State<SharedState>) -> ApiResult<Response> {
    let code = state
        .current_share_code()
        .await
        .ok_or_else(|| ApiError::not_found("No active share"))?;

    let svg = crate::qr::generate_svg(&code)
        .map_err(|e| ApiError::internal(format!("Failed to generate QR code: {e}")))?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/svg+xml")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(svg))
        .unwrap())
}

// ============================================================================
// Receive handlers
// ============================================================================

/// POST /api/receive - Connect to a share code.
pub async fn start_receive(
    State(state): State<SharedState>,
    Json(request): Json<ReceiveRequest>,
) -> ApiResult<Json<ReceiveResponse>> {
    let mode = *state.mode.read().await;
    if mode != WebMode::Idle {
        return Err(ApiError::conflict("Already in an active session"));
    }

    let code = ShareCode::parse(&request.code.to_uppercase())
        .map_err(|e| ApiError::bad_request(format!("Invalid share code: {e}")))?;

    let output_dir = state
        .temp_dir
        .join(format!("receive-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&output_dir)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to create output directory: {e}")))?;

    let config = TransferConfig {
        discovery_timeout: std::time::Duration::from_secs(30),
        ..Default::default()
    };

    let session = ReceiveSession::connect(&code, output_dir.clone(), config)
        .await
        .map_err(|e| match e {
            crate::error::Error::CodeNotFound(_) => ApiError::not_found("Share code not found"),
            crate::error::Error::CodeExpired => ApiError::gone("Share code has expired"),
            _ => ApiError::internal(format!("Failed to connect: {e}")),
        })?;

    let (sender_addr, sender_name) = session.sender();
    let files: Vec<FileInfo> = session.files().iter().map(FileInfo::from).collect();
    let total_size: u64 = session.files().iter().map(|f| f.size).sum();

    let response = ReceiveResponse {
        sender_name: sender_name.to_string(),
        sender_address: sender_addr.to_string(),
        files,
        total_size,
    };

    *state.mode.write().await = WebMode::Receiving;
    *state.pending_receive.lock().await = Some(PendingReceive {
        session,
        created_at: Instant::now(),
    });

    if let Some(ref mut pending) = *state.pending_receive.lock().await {
        if let Err(e) = pending.session.start_keep_alive() {
            tracing::warn!("Failed to start keep-alive: {}", e);
        }
    }

    Ok(Json(response))
}

/// POST /api/receive/accept - Accept the incoming transfer.
pub async fn accept_receive(State(state): State<SharedState>) -> ApiResult<Json<AcceptResponse>> {
    let mode = *state.mode.read().await;
    if mode != WebMode::Receiving {
        return Err(ApiError::conflict("No pending transfer to accept"));
    }

    let pending = state
        .pending_receive
        .lock()
        .await
        .take()
        .ok_or_else(|| ApiError::conflict("No pending transfer"))?;

    let output_dir = pending.session.output_dir().clone();
    let files = pending.session.files().to_vec();

    *state.mode.write().await = WebMode::Transferring;

    *state.active_receive.lock().await = Some(ActiveReceive {
        session: pending.session,
        output_dir: output_dir.clone(),
        accepted_at: Instant::now(),
    });

    let state_clone = state.clone();
    let files_clone = files.clone();
    tokio::spawn(async move {
        receive_transfer_task(state_clone, output_dir, files_clone).await;
    });

    Ok(Json(AcceptResponse {
        message: "Transfer started".to_string(),
    }))
}

/// Background task that handles receiving files.
async fn receive_transfer_task(state: SharedState, output_dir: PathBuf, files: Vec<FileMetadata>) {
    let mut session = {
        let mut active_guard = state.active_receive.lock().await;
        match active_guard.take() {
            Some(active) => active.session,
            None => return,
        }
    };

    let mut progress_rx = session.progress();
    let state_clone = state.clone();

    let progress_task = tokio::spawn(async move {
        while progress_rx.changed().await.is_ok() {
            let progress = progress_rx.borrow().clone();
            state_clone.update_progress(progress);
        }
    });

    match session.accept().await {
        Ok(()) => {
            tracing::info!("Receive transfer completed");
            state.mark_complete().await;

            *state.completed_receive.lock().await = Some(CompletedReceive {
                files,
                output_dir,
                completed_at: Instant::now(),
            });
        }
        Err(e) => {
            tracing::error!("Receive transfer failed: {}", e);
            state.mark_failed(&e.to_string()).await;
        }
    }

    progress_task.abort();
}

/// POST /api/receive/decline - Decline the incoming transfer.
pub async fn decline_receive(State(state): State<SharedState>) -> ApiResult<StatusCode> {
    let mode = *state.mode.read().await;
    if mode != WebMode::Receiving {
        return Err(ApiError::conflict("No pending transfer to decline"));
    }

    let pending = state.pending_receive.lock().await.take();
    if let Some(mut pending) = pending {
        pending.session.decline().await;
    }

    state.reset_to_idle().await;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/receive/download - Download received files.
pub async fn download_received(State(state): State<SharedState>) -> ApiResult<Response> {
    let completed = state.completed_receive.lock().await.take();

    let completed =
        completed.ok_or_else(|| ApiError::not_found("No completed transfer to download"))?;

    if completed.files.len() == 1 {
        let file_meta = &completed.files[0];
        let file_path = completed.output_dir.join(&file_meta.relative_path);

        let file = File::open(&file_path)
            .await
            .map_err(|e| ApiError::internal(format!("Failed to open file: {e}")))?;

        let stream = ReaderStream::new(file);
        let body = Body::from_stream(stream);

        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", file_meta.file_name()),
            )
            .header(header::CONTENT_LENGTH, file_meta.size)
            .body(body)
            .unwrap());
    }

    let zip_path = state
        .temp_dir
        .join(format!("download-{}.zip", uuid::Uuid::new_v4()));

    create_zip(&completed.output_dir, &completed.files, &zip_path)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to create zip: {e}")))?;

    let file = File::open(&zip_path)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to open zip: {e}")))?;

    let metadata = file
        .metadata()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to read zip metadata: {e}")))?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"yoop-files.zip\"",
        )
        .header(header::CONTENT_LENGTH, metadata.len())
        .body(body)
        .unwrap())
}

/// Create a zip file from received files.
async fn create_zip(
    base_dir: &Path,
    files: &[FileMetadata],
    output_path: &Path,
) -> std::io::Result<()> {
    let output_path = output_path.to_path_buf();
    let base_dir = base_dir.to_path_buf();
    let files: Vec<_> = files.iter().map(|f| f.relative_path.clone()).collect();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::create(&output_path)?;
        let mut zip = zip::ZipWriter::new(file);

        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for relative_path in &files {
            let file_path = base_dir.join(relative_path);
            if file_path.exists() {
                let name = relative_path.to_string_lossy();
                zip.start_file(name.as_ref(), options)?;
                let mut f = std::fs::File::open(&file_path)?;
                std::io::copy(&mut f, &mut zip)?;
            }
        }

        zip.finish()?;
        Ok(())
    })
    .await
    .map_err(std::io::Error::other)?
}

// ============================================================================
// History handler
// ============================================================================

/// GET /api/history - Get transfer history.
pub async fn get_history(
    State(state): State<SharedState>,
) -> ApiResult<Json<Vec<TransferHistoryEntry>>> {
    let entries = state.history.lock().await.list(Some(50)).to_vec();

    Ok(Json(entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_info_from_metadata() {
        let meta = FileMetadata {
            relative_path: std::path::PathBuf::from("test.txt"),
            size: 1024,
            mime_type: None,
            created: None,
            modified: None,
            permissions: None,
            is_symlink: false,
            symlink_target: None,
            is_directory: false,
            preview: None,
        };

        let info = FileInfo::from(&meta);
        assert_eq!(info.name, "test.txt");
        assert_eq!(info.size, 1024);
    }

    #[test]
    fn test_get_local_addresses() {
        let addrs = get_local_addresses();
        assert!(!addrs.is_empty());
        assert!(addrs.contains(&"127.0.0.1".to_string()));
    }
}
