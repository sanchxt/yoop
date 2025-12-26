//! Embedded web server for LocalDrop.
//!
//! The web interface requires **zero external infrastructure**. The LocalDrop
//! binary serves a web UI accessible to browsers on the same network.
//!
//! ## Starting Web Mode
//!
//! ```bash
//! localdrop web                     # Default port 8080
//! localdrop web --port 9000         # Custom port
//! localdrop web --localhost-only    # Restrict to localhost
//! localdrop web --auth              # Enable authentication
//! ```
//!
//! ## API Endpoints
//!
//! | Method | Endpoint | Description |
//! |--------|----------|-------------|
//! | GET | / | Web UI (SPA) |
//! | GET | /api/status | Current status |
//! | GET | /api/network | Network info |
//! | POST | /api/share | Start sharing (multipart upload) |
//! | GET | /api/share/code | Get current share code |
//! | DELETE | /api/share | Cancel share |
//! | POST | /api/receive | Connect to share code |
//! | POST | /api/receive/accept | Accept transfer |
//! | POST | /api/receive/decline | Decline transfer |
//! | GET | /api/receive/download | Download received files |
//! | GET | /api/transfer/progress | Progress (SSE) |
//! | GET | /api/history | Transfer history |

pub mod assets;
pub mod error;
pub mod handlers;
pub mod sse;
pub mod state;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    routing::{delete, get, post},
    Router,
};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tower_http::{cors::CorsLayer, limit::RequestBodyLimitLayer};

use crate::error::Result;

pub use error::{ApiError, ApiResult};
pub use state::{AppState, SharedState, WebMode};

/// Configuration for the web server.
#[derive(Debug, Clone)]
pub struct WebServerConfig {
    /// Port to listen on
    pub port: u16,
    /// Bind to localhost only
    pub localhost_only: bool,
    /// Require authentication
    pub auth_enabled: bool,
    /// Authentication password (generated if auth enabled)
    pub auth_password: Option<String>,
}

impl Default for WebServerConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            localhost_only: false,
            auth_enabled: false,
            auth_password: None,
        }
    }
}

impl WebServerConfig {
    /// Generate a random password for authentication.
    #[must_use]
    pub fn generate_password() -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        let mut rng = rand::thread_rng();
        (0..6)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    /// Get the bind address for the server.
    #[must_use]
    pub fn bind_addr(&self) -> SocketAddr {
        if self.localhost_only {
            SocketAddr::from(([127, 0, 0, 1], self.port))
        } else {
            SocketAddr::from(([0, 0, 0, 0], self.port))
        }
    }
}

/// Create the API router with all endpoints.
fn create_router(state: SharedState) -> Router {
    let api_routes = Router::new()
        .route("/status", get(handlers::get_status))
        .route("/network", get(handlers::get_network))
        .route("/share", post(handlers::create_share))
        .route("/share", delete(handlers::cancel_share))
        .route("/share/code", get(handlers::get_share_code))
        .route("/receive", post(handlers::start_receive))
        .route("/receive/accept", post(handlers::accept_receive))
        .route("/receive/decline", post(handlers::decline_receive))
        .route("/receive/download", get(handlers::download_received))
        .route("/transfer/progress", get(sse::progress_sse))
        .route("/history", get(handlers::get_history));

    Router::new()
        .nest("/api", api_routes)
        .fallback(assets::serve_static_fallback)
        .layer(CorsLayer::permissive())
        .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024 * 1024))
        .with_state(state)
}

/// The web server instance.
pub struct WebServer {
    config: WebServerConfig,
    state: SharedState,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl std::fmt::Debug for WebServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebServer")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl WebServer {
    /// Create a new web server with the given configuration.
    #[must_use]
    pub fn new(config: WebServerConfig) -> Self {
        let state = Arc::new(AppState::new(config.clone()));
        Self {
            config,
            state,
            shutdown_tx: None,
        }
    }

    /// Get the server configuration.
    #[must_use]
    pub const fn config(&self) -> &WebServerConfig {
        &self.config
    }

    /// Get the shared application state.
    #[must_use]
    pub fn state(&self) -> SharedState {
        Arc::clone(&self.state)
    }

    /// Start the web server.
    ///
    /// This method will block until the server receives Ctrl+C or `stop()` is called.
    ///
    /// # Errors
    ///
    /// Returns an error if the server cannot bind to the configured address.
    pub async fn start(&mut self) -> Result<()> {
        let addr = self.config.bind_addr();
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| crate::error::Error::Internal(format!("Failed to bind to {addr}: {e}")))?;

        tracing::info!("Web server listening on http://{}", addr);

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        let router = create_router(Arc::clone(&self.state));

        let ctrl_c = async {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("Received shutdown signal");
        };

        let shutdown = async move {
            tokio::select! {
                () = ctrl_c => {}
                _ = shutdown_rx => {}
            }
            tracing::info!("Web server shutting down");
        };

        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(|e| crate::error::Error::Internal(format!("Server error: {e}")))?;

        Ok(())
    }

    /// Stop the web server gracefully.
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
            tracing::info!("Shutdown signal sent to web server");
        }
    }

    /// Get all network addresses the server is accessible from.
    #[must_use]
    pub fn addresses(&self) -> Vec<String> {
        let mut addrs = vec![format!("http://localhost:{}", self.config.port)];

        if !self.config.localhost_only {
            if let Ok(interfaces) = get_local_addresses() {
                for ip in interfaces {
                    addrs.push(format!("http://{}:{}", ip, self.config.port));
                }
            }
        }

        addrs
    }
}

/// Get local network IP addresses (non-loopback IPv4).
fn get_local_addresses() -> std::io::Result<Vec<String>> {
    use std::net::UdpSocket;

    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("8.8.8.8:80")?;
    let local_addr = socket.local_addr()?;

    Ok(vec![local_addr.ip().to_string()])
}

/// WebSocket message from client to server.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum WsClientMessage {
    /// Start sharing files
    Share {
        /// File paths (from upload)
        files: Vec<String>,
    },
    /// Start receiving
    Receive {
        /// Share code
        code: String,
    },
    /// Cancel current operation
    Cancel,
    /// Accept incoming transfer
    Accept,
    /// Decline incoming transfer
    Decline,
}

/// WebSocket message from server to client.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum WsServerMessage {
    /// Status update
    Status {
        /// Current mode
        mode: String,
        /// Status message
        message: String,
    },
    /// Transfer progress
    Progress {
        /// Current file
        file: String,
        /// File index
        file_index: usize,
        /// Total files
        total_files: usize,
        /// Bytes transferred
        bytes_transferred: u64,
        /// Total bytes
        total_bytes: u64,
        /// Speed in bytes per second
        speed_bps: u64,
        /// ETA in seconds
        eta_seconds: Option<f64>,
    },
    /// Device discovered
    Discovered {
        /// Device name
        device_name: String,
        /// Device address
        address: String,
    },
    /// Connected to peer
    Connected {
        /// Peer device name
        device_name: String,
    },
    /// Preview available
    Preview {
        /// File index
        file_index: usize,
        /// Preview data
        preview: crate::preview::Preview,
    },
    /// Transfer complete
    Complete {
        /// Number of files
        files: usize,
        /// Total bytes
        total_bytes: u64,
        /// Duration in seconds
        duration_secs: f64,
    },
    /// Error occurred
    Error {
        /// Error code
        code: Option<String>,
        /// Error message
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = WebServerConfig::default();
        assert_eq!(config.port, 8080);
        assert!(!config.localhost_only);
        assert!(!config.auth_enabled);
        assert!(config.auth_password.is_none());
    }

    #[test]
    fn test_config_bind_addr() {
        let mut config = WebServerConfig::default();
        assert_eq!(config.bind_addr(), SocketAddr::from(([0, 0, 0, 0], 8080)));

        config.localhost_only = true;
        assert_eq!(config.bind_addr(), SocketAddr::from(([127, 0, 0, 1], 8080)));
    }

    #[test]
    fn test_generate_password() {
        let password = WebServerConfig::generate_password();
        assert_eq!(password.len(), 6);
        assert!(password.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_ws_message_serialization() {
        let msg = WsServerMessage::Progress {
            file: "test.txt".into(),
            file_index: 0,
            total_files: 1,
            bytes_transferred: 500,
            total_bytes: 1000,
            speed_bps: 100,
            eta_seconds: Some(5.0),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"Progress\""));
        assert!(json.contains("\"file\":\"test.txt\""));
    }
}
