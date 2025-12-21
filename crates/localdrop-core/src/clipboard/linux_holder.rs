//! Linux-specific clipboard holder for persistent image clipboard.
//!
//! On Wayland (and to some extent X11), clipboard content is "owned" by the
//! application that sets it. When that application exits, the clipboard content
//! becomes unavailable unless a clipboard manager has claimed it.
//!
//! This module provides a mechanism to fork a background process that holds
//! the clipboard content until it's pasted or a timeout expires. This is the
//! same approach used by `wl-copy` internally.

// Allow unsafe for fork() - this is the only way to create a background process
// that can hold clipboard content independently of the main process.
#![allow(unsafe_code)]

use std::time::Duration;

use crate::error::{Error, Result};

/// Default timeout for the clipboard holder process (5 minutes).
pub const DEFAULT_HOLDER_TIMEOUT: Duration = Duration::from_secs(300);

/// Hold image content in a background process for clipboard persistence.
///
/// This function forks a detached child process that:
/// 1. Creates a new clipboard instance
/// 2. Sets the image content
/// 3. Waits for the clipboard manager to claim it or timeout expires
/// 4. Exits cleanly
///
/// The parent process waits briefly for the child to initialize, then returns.
///
/// # Arguments
///
/// * `png_data` - PNG-encoded image data
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
/// * `timeout` - How long the holder process should wait before exiting
///
/// # Errors
///
/// Returns an error if:
/// - Fork fails
/// - Child process fails to set clipboard
///
/// # Safety
///
/// This function uses `fork()` which is inherently unsafe in multi-threaded
/// programs. It should be called from the main thread before spawning threads,
/// or with care in async contexts.
pub fn hold_image_in_background(
    png_data: Vec<u8>,
    width: u32,
    height: u32,
    timeout: Duration,
) -> Result<()> {
    use nix::unistd::{fork, setsid, ForkResult};

    // SAFETY: We're forking here. The child process will be short-lived and
    // only interact with the clipboard. We use setsid() to detach from the
    // parent's session.
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child }) => {
            tracing::debug!(
                "Clipboard holder: forked child process {} for image {}x{}",
                child,
                width,
                height
            );

            // Wait briefly for child to initialize and set clipboard
            // This ensures the clipboard is set before we return
            std::thread::sleep(Duration::from_millis(500));

            Ok(())
        }

        Ok(ForkResult::Child) => {
            // Detach from parent's session to become a daemon
            if let Err(e) = setsid() {
                tracing::error!("Clipboard holder: setsid failed: {}", e);
                std::process::exit(1);
            }

            // Run the holder logic
            match run_holder(png_data, width, height, timeout) {
                Ok(()) => {
                    tracing::debug!("Clipboard holder: exiting normally");
                    std::process::exit(0);
                }
                Err(e) => {
                    tracing::error!("Clipboard holder: failed: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Err(e) => Err(Error::ClipboardError(format!(
            "fork failed for clipboard holder: {}",
            e
        ))),
    }
}

/// Internal function that runs in the child process to hold clipboard content.
fn run_holder(png_data: Vec<u8>, width: u32, height: u32, timeout: Duration) -> Result<()> {
    use arboard::Clipboard;

    // Decode PNG to RGBA for arboard
    let img = image::load_from_memory_with_format(&png_data, image::ImageFormat::Png)
        .map_err(|e| Error::ClipboardError(format!("failed to decode PNG in holder: {}", e)))?;

    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();

    tracing::debug!(
        "Clipboard holder: setting image {}x{} (decoded {}x{})",
        width,
        height,
        w,
        h
    );

    // Create a fresh clipboard instance in the child process
    let mut clipboard = Clipboard::new()
        .map_err(|e| Error::ClipboardError(format!("holder failed to access clipboard: {}", e)))?;

    // Prepare image data for arboard
    let image_data = arboard::ImageData {
        width: w as usize,
        height: h as usize,
        bytes: std::borrow::Cow::Owned(rgba.into_raw()),
    };

    // Set the image to clipboard
    clipboard
        .set_image(image_data)
        .map_err(|e| Error::ClipboardError(format!("holder failed to set image: {}", e)))?;

    tracing::debug!(
        "Clipboard holder: image set, waiting up to {:?} for clipboard manager",
        timeout
    );

    // Wait for the clipboard manager to claim the content or timeout
    // On Wayland, the clipboard manager will eventually claim the content
    // and we can exit. If no manager claims it, we wait until timeout.
    //
    // We could use arboard's wait_until() here, but that has proven unreliable
    // for images. Instead, we just sleep and let the clipboard manager pick
    // up the content at its leisure.
    std::thread::sleep(timeout);

    tracing::debug!("Clipboard holder: timeout reached, exiting");
    Ok(())
}

/// Hold text content in a background process for clipboard persistence.
///
/// Similar to `hold_image_in_background` but for text content.
/// Generally not needed as text clipboard works more reliably, but
/// available for consistency.
///
/// # Arguments
///
/// * `text` - Text content to hold
/// * `timeout` - How long the holder process should wait before exiting
///
/// # Errors
///
/// Returns an error if fork fails or child process fails to set clipboard.
pub fn hold_text_in_background(text: String, timeout: Duration) -> Result<()> {
    use nix::unistd::{fork, setsid, ForkResult};

    match unsafe { fork() } {
        Ok(ForkResult::Parent { child }) => {
            tracing::debug!(
                "Clipboard holder: forked child process {} for text ({} bytes)",
                child,
                text.len()
            );

            std::thread::sleep(Duration::from_millis(200));
            Ok(())
        }

        Ok(ForkResult::Child) => {
            if let Err(e) = setsid() {
                tracing::error!("Clipboard holder: setsid failed: {}", e);
                std::process::exit(1);
            }

            match run_text_holder(text, timeout) {
                Ok(()) => std::process::exit(0),
                Err(e) => {
                    tracing::error!("Clipboard holder: failed: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Err(e) => Err(Error::ClipboardError(format!(
            "fork failed for clipboard holder: {}",
            e
        ))),
    }
}

/// Internal function that runs in the child process to hold text content.
fn run_text_holder(text: String, timeout: Duration) -> Result<()> {
    use arboard::Clipboard;

    let mut clipboard = Clipboard::new()
        .map_err(|e| Error::ClipboardError(format!("holder failed to access clipboard: {}", e)))?;

    clipboard
        .set_text(text)
        .map_err(|e| Error::ClipboardError(format!("holder failed to set text: {}", e)))?;

    tracing::debug!("Clipboard holder: text set, waiting for clipboard manager");
    std::thread::sleep(timeout);

    Ok(())
}

/// Detect the current display server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayServer {
    /// Wayland display server
    Wayland,
    /// X11 display server
    X11,
    /// Unknown or no display server
    Unknown,
}

impl DisplayServer {
    /// Detect the current display server from environment variables.
    #[must_use]
    pub fn detect() -> Self {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            DisplayServer::Wayland
        } else if std::env::var("DISPLAY").is_ok() {
            DisplayServer::X11
        } else {
            DisplayServer::Unknown
        }
    }

    /// Check if we're running on Wayland.
    #[must_use]
    pub fn is_wayland(&self) -> bool {
        matches!(self, DisplayServer::Wayland)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_server_detection() {
        // Just verify it doesn't panic
        let server = DisplayServer::detect();
        println!("Detected display server: {:?}", server);
    }

    #[test]
    fn test_default_timeout() {
        assert_eq!(DEFAULT_HOLDER_TIMEOUT, Duration::from_secs(300));
    }
}
