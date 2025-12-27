//! Static file serving using rust-embed.
//!
//! This module embeds the web UI assets (HTML, JS, CSS) directly into the binary
//! at compile time, eliminating the need for a separate static file server.

use axum::{
    body::Body,
    http::{header, Request, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

/// Embedded static assets for the web UI.
#[derive(RustEmbed)]
#[folder = "src/web/assets/"]
#[include = "*.html"]
#[include = "*.js"]
#[include = "*.css"]
#[include = "*.ico"]
#[include = "*.png"]
#[include = "*.svg"]
pub struct Assets;

/// Serve a static file from embedded assets.
///
/// Returns the file content with appropriate content-type header.
pub async fn serve_static(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    let path = if path.is_empty() { "index.html" } else { path };

    serve_file(path)
}

/// Serve a static file as a fallback handler.
///
/// This is used as the fallback for the router to serve static files
/// for any path that doesn't match an API route.
pub async fn serve_static_fallback(request: Request<Body>) -> impl IntoResponse {
    let path = request.uri().path().trim_start_matches('/');

    let path = if path.is_empty() || !path.contains('.') {
        "index.html"
    } else {
        path
    };

    serve_file(path)
}

/// Serve a specific file from embedded assets.
fn serve_file(path: &str) -> Response {
    if let Some(content) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();

        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, cache_control_for(path))
            .body(Body::from(content.data.to_vec()))
            .unwrap()
    } else {
        if !path.contains('.') {
            if let Some(content) = Assets::get("index.html") {
                return Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/html")
                    .body(Body::from(content.data.to_vec()))
                    .unwrap();
            }
        }

        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::from("Not Found"))
            .unwrap()
    }
}

/// Check if path has a specific extension (case-insensitive).
fn has_extension(path: &str, ext: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

/// Get appropriate cache-control header for a file type.
fn cache_control_for(path: &str) -> &'static str {
    if has_extension(path, "html") {
        "no-cache, no-store, must-revalidate"
    } else if has_extension(path, "js") || has_extension(path, "css") {
        "public, max-age=3600"
    } else {
        "public, max-age=86400"
    }
}

/// List all embedded assets (for debugging).
#[must_use]
pub fn list_assets() -> Vec<String> {
    Assets::iter().map(|f| f.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assets_embedded() {
        let assets = list_assets();
        if !assets.is_empty() {
            assert!(assets.iter().any(|a| a == "index.html"));
        }
    }

    #[test]
    fn test_cache_control() {
        assert!(cache_control_for("index.html").contains("no-cache"));
        assert!(cache_control_for("app.js").contains("max-age=3600"));
        assert!(cache_control_for("style.css").contains("max-age=3600"));
        assert!(cache_control_for("logo.png").contains("max-age=86400"));
    }

    #[test]
    fn test_mime_type_detection() {
        let html_mime = mime_guess::from_path("index.html").first_or_octet_stream();
        assert_eq!(html_mime.as_ref(), "text/html");

        let js_mime = mime_guess::from_path("app.js").first_or_octet_stream();
        assert!(js_mime.as_ref().contains("javascript"));

        let css_mime = mime_guess::from_path("style.css").first_or_octet_stream();
        assert_eq!(css_mime.as_ref(), "text/css");
    }
}
