//! Server-Sent Events (SSE) for real-time progress updates.
//!
//! This module provides SSE endpoints for streaming transfer progress
//! to connected web clients.

use std::convert::Infallible;
use std::time::Duration;

use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::{self, Stream};
use serde::Serialize;

use crate::transfer::{TransferProgress, TransferState};

use super::state::SharedState;

/// Progress event sent via SSE.
#[derive(Debug, Serialize)]
pub struct ProgressEvent {
    /// Event type
    #[serde(rename = "type")]
    event_type: String,
    /// Current file name
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    /// Current file index (0-based)
    #[serde(skip_serializing_if = "Option::is_none")]
    file_index: Option<usize>,
    /// Total number of files
    #[serde(skip_serializing_if = "Option::is_none")]
    total_files: Option<usize>,
    /// Bytes transferred for this file
    #[serde(skip_serializing_if = "Option::is_none")]
    bytes_transferred: Option<u64>,
    /// Total bytes for this file
    #[serde(skip_serializing_if = "Option::is_none")]
    total_bytes: Option<u64>,
    /// Transfer speed in bytes per second
    #[serde(skip_serializing_if = "Option::is_none")]
    speed_bps: Option<u64>,
    /// Estimated time remaining in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    eta_seconds: Option<f64>,
    /// Progress percentage (0-100)
    #[serde(skip_serializing_if = "Option::is_none")]
    percentage: Option<f64>,
    /// Number of files (for complete event)
    #[serde(skip_serializing_if = "Option::is_none")]
    files: Option<usize>,
    /// Duration in seconds (for complete event)
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_secs: Option<f64>,
    /// Error message (for error event)
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

impl ProgressEvent {
    /// Create a progress event from transfer progress.
    fn from_progress(progress: &TransferProgress) -> Self {
        Self {
            event_type: "progress".to_string(),
            file: Some(progress.current_file_name.clone()),
            file_index: Some(progress.current_file),
            total_files: Some(progress.total_files),
            bytes_transferred: Some(progress.total_bytes_transferred),
            total_bytes: Some(progress.total_bytes),
            speed_bps: Some(progress.speed_bps),
            eta_seconds: progress.eta.map(|d| d.as_secs_f64()),
            percentage: Some(progress.percentage()),
            files: None,
            duration_secs: None,
            message: None,
        }
    }

    /// Create a complete event.
    fn complete(progress: &TransferProgress) -> Self {
        let duration = progress.started_at.elapsed().as_secs_f64();

        Self {
            event_type: "complete".to_string(),
            file: None,
            file_index: None,
            total_files: None,
            bytes_transferred: None,
            total_bytes: Some(progress.total_bytes),
            speed_bps: None,
            eta_seconds: None,
            percentage: Some(100.0),
            files: Some(progress.total_files),
            duration_secs: Some(duration),
            message: None,
        }
    }

    /// Create an error event.
    fn error(message: &str) -> Self {
        Self {
            event_type: "error".to_string(),
            file: None,
            file_index: None,
            total_files: None,
            bytes_transferred: None,
            total_bytes: None,
            speed_bps: None,
            eta_seconds: None,
            percentage: None,
            files: None,
            duration_secs: None,
            message: Some(message.to_string()),
        }
    }
}

/// GET /api/transfer/progress - SSE stream of progress events.
pub async fn progress_sse(
    State(state): State<SharedState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.progress_rx.clone();

    let stream = stream::unfold(rx, |mut rx| async move {
        loop {
            match tokio::time::timeout(Duration::from_secs(30), rx.changed()).await {
                Ok(Ok(())) => {
                    let progress = rx.borrow().clone();

                    if let Some(p) = progress {
                        let event = match p.state {
                            TransferState::Completed => {
                                let ev = ProgressEvent::complete(&p);
                                let data = serde_json::to_string(&ev).unwrap_or_default();
                                Event::default().event("complete").data(data)
                            }
                            TransferState::Failed => {
                                let ev = ProgressEvent::error("Transfer failed");
                                let data = serde_json::to_string(&ev).unwrap_or_default();
                                Event::default().event("error").data(data)
                            }
                            TransferState::Cancelled => {
                                let ev = ProgressEvent::error("Transfer cancelled");
                                let data = serde_json::to_string(&ev).unwrap_or_default();
                                Event::default().event("error").data(data)
                            }
                            _ => {
                                let ev = ProgressEvent::from_progress(&p);
                                let data = serde_json::to_string(&ev).unwrap_or_default();
                                Event::default().data(data)
                            }
                        };

                        return Some((Ok(event), rx));
                    }
                }
                Ok(Err(_)) => {
                    return None;
                }
                Err(_) => {
                    let event = Event::default().comment("keepalive");
                    return Some((Ok(event), rx));
                }
            }
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_progress_event_from_progress() {
        let mut progress = TransferProgress::new(3, 1000);
        progress.current_file = 1;
        progress.current_file_name = "test.txt".to_string();
        progress.total_bytes_transferred = 500;
        progress.speed_bps = 100;

        let event = ProgressEvent::from_progress(&progress);

        assert_eq!(event.event_type, "progress");
        assert_eq!(event.file, Some("test.txt".to_string()));
        assert_eq!(event.file_index, Some(1));
        assert_eq!(event.total_files, Some(3));
        assert_eq!(event.bytes_transferred, Some(500));
        assert_eq!(event.total_bytes, Some(1000));
        assert_eq!(event.speed_bps, Some(100));
        assert_eq!(event.percentage, Some(50.0));
    }

    #[test]
    fn test_complete_event() {
        let progress = TransferProgress {
            state: TransferState::Completed,
            current_file: 2,
            total_files: 3,
            current_file_name: "last.txt".to_string(),
            file_bytes_transferred: 100,
            file_total_bytes: 100,
            total_bytes_transferred: 1000,
            total_bytes: 1000,
            speed_bps: 500,
            eta: None,
            started_at: Instant::now(),
        };

        let event = ProgressEvent::complete(&progress);

        assert_eq!(event.event_type, "complete");
        assert_eq!(event.files, Some(3));
        assert_eq!(event.total_bytes, Some(1000));
        assert!(event.duration_secs.is_some());
    }

    #[test]
    fn test_error_event() {
        let event = ProgressEvent::error("Connection lost");

        assert_eq!(event.event_type, "error");
        assert_eq!(event.message, Some("Connection lost".to_string()));
    }

    #[test]
    fn test_event_serialization() {
        let event = ProgressEvent::error("Test error");
        let json = serde_json::to_string(&event).unwrap();

        assert!(json.contains("\"type\":\"error\""));
        assert!(json.contains("\"message\":\"Test error\""));
        assert!(!json.contains("file_index"));
    }
}
