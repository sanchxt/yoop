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
//! | POST | /api/share | Start sharing |
//! | POST | /api/receive | Start receiving |
//! | GET | /api/transfer/progress | Progress (SSE) |
//! | WS | /ws | Real-time updates |

use std::net::SocketAddr;

use crate::error::Result;

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

/// The web server instance.
#[derive(Debug)]
pub struct WebServer {
    config: WebServerConfig,
}

impl WebServer {
    /// Create a new web server with the given configuration.
    #[must_use]
    pub const fn new(config: WebServerConfig) -> Self {
        Self { config }
    }

    /// Get the server configuration.
    #[must_use]
    pub const fn config(&self) -> &WebServerConfig {
        &self.config
    }

    /// Start the web server.
    ///
    /// # Errors
    ///
    /// Returns an error if the server cannot be started.
    pub async fn start(&self) -> Result<()> {
        // TODO: Implement web server using axum
        tracing::info!("Starting web server on {}", self.config.bind_addr());
        Ok(())
    }

    /// Stop the web server.
    pub async fn stop(&self) {
        // TODO: Implement graceful shutdown
        tracing::info!("Stopping web server");
    }

    /// Get all network addresses the server is accessible from.
    #[must_use]
    pub fn addresses(&self) -> Vec<String> {
        let addrs = vec![format!("http://localhost:{}", self.config.port)];

        // TODO: Add network interface addresses when !localhost_only
        addrs
    }
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
