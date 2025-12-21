//! Internal commands for clipboard holder subprocess.
//!
//! These commands are not user-facing. They are invoked by the main process
//! to spawn a subprocess that holds clipboard content on Linux (Wayland/X11).

use std::io::Read;
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "linux")]
use std::sync::Arc;
use std::time::Duration;
#[cfg(target_os = "linux")]
use std::time::Instant;

use anyhow::{bail, Result};
use arboard::Clipboard;

/// Run the internal clipboard hold command.
///
/// This is called by a spawned subprocess to hold clipboard content.
/// It reads binary data from stdin, sets the clipboard, and holds until timeout.
///
/// # Arguments
///
/// * `content_type` - "image" or "text"
/// * `timeout_secs` - How long to hold the clipboard before exiting
///
/// # Errors
///
/// Returns an error if clipboard cannot be set.
#[allow(clippy::too_many_lines)]
pub fn run_clipboard_hold(content_type: &str, timeout_secs: u64) -> Result<()> {
    let mut data = Vec::new();
    std::io::stdin().read_to_end(&mut data)?;

    if data.is_empty() {
        bail!("No data received on stdin");
    }

    eprintln!(
        "Clipboard holder: received {} bytes of {} data",
        data.len(),
        content_type
    );

    #[cfg(target_os = "linux")]
    {
        use arboard::SetExtLinux;

        let deadline = Instant::now() + Duration::from_secs(timeout_secs);

        let mut clipboard = Clipboard::new()
            .map_err(|e| anyhow::anyhow!("Failed to access clipboard in holder process: {}", e))?;

        match content_type {
            "image" => {
                let watchdog_timeout = timeout_secs;
                let watchdog_active = Arc::new(AtomicBool::new(true));
                let watchdog_flag = Arc::clone(&watchdog_active);

                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(watchdog_timeout));
                    if watchdog_flag.load(Ordering::SeqCst) {
                        eprintln!(
                            "Clipboard holder: safety timeout reached after {} seconds, exiting",
                            watchdog_timeout
                        );
                        std::process::exit(0);
                    }
                });

                let img = image::load_from_memory(&data)
                    .map_err(|e| anyhow::anyhow!("Failed to decode image: {}", e))?;

                let rgba = img.to_rgba8();
                let (width, height) = rgba.dimensions();

                eprintln!(
                    "Clipboard holder: setting image {}x{} to clipboard",
                    width, height
                );

                let image_data = arboard::ImageData {
                    width: width as usize,
                    height: height as usize,
                    bytes: std::borrow::Cow::Owned(rgba.into_raw()),
                };

                eprintln!(
                    "Clipboard holder: waiting for clipboard to be claimed (up to {} seconds)",
                    watchdog_timeout
                );
                clipboard
                    .set()
                    .wait()
                    .image(image_data)
                    .map_err(|e| anyhow::anyhow!("Failed to set image in holder: {}", e))?;

                watchdog_active.store(false, Ordering::SeqCst);
                eprintln!("Clipboard holder: clipboard was overwritten by another application");
            }
            "text" => {
                let text = String::from_utf8(data)
                    .map_err(|e| anyhow::anyhow!("Invalid UTF-8 text: {}", e))?;

                eprintln!(
                    "Clipboard holder: setting {} bytes of text to clipboard",
                    text.len()
                );

                clipboard
                    .set()
                    .wait_until(deadline)
                    .text(text)
                    .map_err(|e| anyhow::anyhow!("Failed to set text in holder: {}", e))?;

                eprintln!("Clipboard holder: text set and wait completed");
            }
            other => {
                bail!("Unknown content type: {}", other);
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        let mut clipboard = Clipboard::new()
            .map_err(|e| anyhow::anyhow!("Failed to access clipboard in holder process: {}", e))?;

        match content_type {
            "image" => {
                let img = image::load_from_memory(&data)
                    .map_err(|e| anyhow::anyhow!("Failed to decode image: {}", e))?;

                let rgba = img.to_rgba8();
                let (width, height) = rgba.dimensions();

                eprintln!(
                    "Clipboard holder: setting image {}x{} to clipboard",
                    width, height
                );

                let image_data = arboard::ImageData {
                    width: width as usize,
                    height: height as usize,
                    bytes: std::borrow::Cow::Owned(rgba.into_raw()),
                };

                clipboard
                    .set_image(image_data)
                    .map_err(|e| anyhow::anyhow!("Failed to set image in holder: {}", e))?;

                eprintln!("Clipboard holder: image set successfully");
            }
            "text" => {
                let text = String::from_utf8(data)
                    .map_err(|e| anyhow::anyhow!("Invalid UTF-8 text: {}", e))?;

                eprintln!(
                    "Clipboard holder: setting {} bytes of text to clipboard",
                    text.len()
                );

                clipboard
                    .set_text(text)
                    .map_err(|e| anyhow::anyhow!("Failed to set text in holder: {}", e))?;

                eprintln!("Clipboard holder: text set successfully");
            }
            other => {
                bail!("Unknown content type: {}", other);
            }
        }

        eprintln!(
            "Clipboard holder: holding clipboard for up to {} seconds",
            timeout_secs
        );
        std::thread::sleep(Duration::from_secs(timeout_secs));
    }

    eprintln!("Clipboard holder: exiting");
    Ok(())
}
