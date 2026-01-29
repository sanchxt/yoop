//! Connection utilities for direct peer connections.
//!
//! This module provides utilities for connecting directly to peers by IP address,
//! bypassing the normal discovery mechanism. This is useful for VPN/overlay networks
//! (Tailscale, WireGuard, ZeroTier) where UDP broadcast and mDNS discovery don't work.

use std::net::{IpAddr, SocketAddr};

use crate::error::{Error, Result};
use crate::transfer::DEFAULT_TRANSFER_PORT;

/// Parse a host address string into a `SocketAddr`.
///
/// Accepts formats:
/// - `IP` (e.g., `192.168.1.100`) - uses default port 52530
/// - `IP:PORT` (e.g., `192.168.1.100:52540`) - uses specified port
/// - `[IPv6]` (e.g., `[::1]`) - uses default port 52530
/// - `[IPv6]:PORT` (e.g., `[::1]:52540`) - uses specified port
///
/// # Examples
///
/// ```
/// use yoop_core::connection::parse_host_address;
///
/// // IPv4 with default port
/// let addr = parse_host_address("192.168.1.100").unwrap();
/// assert_eq!(addr.port(), 52530);
///
/// // IPv4 with custom port
/// let addr = parse_host_address("192.168.1.100:52540").unwrap();
/// assert_eq!(addr.port(), 52540);
/// ```
///
/// # Errors
///
/// Returns an error if the host string cannot be parsed.
pub fn parse_host_address(host: &str) -> Result<SocketAddr> {
    let host = host.trim();

    if let Ok(addr) = host.parse::<SocketAddr>() {
        return Ok(addr);
    }

    if host.starts_with('[') && host.ends_with(']') {
        let ip_str = &host[1..host.len() - 1];
        let ip: IpAddr = ip_str.parse().map_err(|_| {
            Error::InvalidInput(format!(
                "Invalid host format '{host}'. Use IP or IP:PORT (e.g., 192.168.1.100 or 192.168.1.100:52530)"
            ))
        })?;
        return Ok(SocketAddr::new(ip, DEFAULT_TRANSFER_PORT));
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, DEFAULT_TRANSFER_PORT));
    }

    if let Some((ip_part, port_part)) = host.rsplit_once(':') {
        if !ip_part.contains(':') {
            let ip: IpAddr = ip_part.parse().map_err(|_| {
                Error::InvalidInput(format!(
                    "Invalid host format '{host}'. Use IP or IP:PORT (e.g., 192.168.1.100 or 192.168.1.100:52530)"
                ))
            })?;
            let port: u16 = port_part.parse().map_err(|_| {
                Error::InvalidInput(format!(
                    "Invalid port '{port_part}'. Port must be a number between 1 and 65535"
                ))
            })?;
            return Ok(SocketAddr::new(ip, port));
        }
    }

    Err(Error::InvalidInput(format!(
        "Invalid host format '{host}'. Use IP or IP:PORT (e.g., 192.168.1.100 or 192.168.1.100:52530)"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_host_ipv4_only() {
        let addr = parse_host_address("192.168.1.100").unwrap();
        assert_eq!(addr.ip().to_string(), "192.168.1.100");
        assert_eq!(addr.port(), DEFAULT_TRANSFER_PORT);
    }

    #[test]
    fn test_parse_host_ipv4_with_port() {
        let addr = parse_host_address("192.168.1.100:52540").unwrap();
        assert_eq!(addr.ip().to_string(), "192.168.1.100");
        assert_eq!(addr.port(), 52540);
    }

    #[test]
    fn test_parse_host_ipv6_brackets() {
        let addr = parse_host_address("[::1]").unwrap();
        assert_eq!(addr.ip().to_string(), "::1");
        assert_eq!(addr.port(), DEFAULT_TRANSFER_PORT);
    }

    #[test]
    fn test_parse_host_ipv6_with_port() {
        let addr = parse_host_address("[::1]:52540").unwrap();
        assert_eq!(addr.ip().to_string(), "::1");
        assert_eq!(addr.port(), 52540);
    }

    #[test]
    fn test_parse_host_ipv6_full() {
        let addr = parse_host_address("[2001:db8::1]:52540").unwrap();
        assert_eq!(addr.ip().to_string(), "2001:db8::1");
        assert_eq!(addr.port(), 52540);
    }

    #[test]
    fn test_parse_host_tailscale_ip() {
        let addr = parse_host_address("100.103.164.32").unwrap();
        assert_eq!(addr.ip().to_string(), "100.103.164.32");
        assert_eq!(addr.port(), DEFAULT_TRANSFER_PORT);
    }

    #[test]
    fn test_parse_host_localhost() {
        let addr = parse_host_address("127.0.0.1:8080").unwrap();
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_eq!(addr.port(), 8080);
    }

    #[test]
    fn test_parse_host_invalid() {
        assert!(parse_host_address("not-an-ip").is_err());
        assert!(parse_host_address("192.168.1.100:abc").is_err());
        assert!(parse_host_address("192.168.1.256").is_err());
    }

    #[test]
    fn test_parse_host_whitespace() {
        let addr = parse_host_address("  192.168.1.100  ").unwrap();
        assert_eq!(addr.ip().to_string(), "192.168.1.100");
    }
}
