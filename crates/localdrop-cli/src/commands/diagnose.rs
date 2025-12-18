//! Diagnose command implementation.

use anyhow::Result;

use super::DiagnoseArgs;

/// Run the diagnose command.
pub async fn run(args: DiagnoseArgs) -> Result<()> {
    if args.json {
        let output = serde_json::json!({
            "version": localdrop_core::VERSION,
            "network": {
                "status": "unknown",
            },
            "udp_broadcast": "unknown",
            "mdns": "unknown",
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!();
    println!("Network Diagnostics");
    println!("{}", "─".repeat(43));
    println!();

    // TODO: Detect network interface
    println!("Interface: (detecting...)");
    println!("  IP: (detecting...)");
    println!("  Broadcast: (detecting...)");
    println!("  Status: Checking...");
    println!();

    // TODO: Test UDP broadcast
    println!("UDP Broadcast: Testing...");
    // TODO: Test mDNS
    println!("mDNS: Testing...");

    // TODO: Check firewall
    println!(
        "Firewall: Checking ports {}-{}...",
        localdrop_core::DEFAULT_DISCOVERY_PORT,
        localdrop_core::DEFAULT_TRANSFER_PORT_END
    );

    println!();
    println!("Discovered Devices (not sharing):");
    println!("  (scanning...)");

    println!();
    println!("Trusted Devices Online:");
    let trust_store = localdrop_core::trust::TrustStore::load()?;
    let devices = trust_store.list();
    if devices.is_empty() {
        println!("  (no trusted devices)");
    } else {
        for device in devices {
            println!("  - {} (offline)", device.device_name);
        }
    }

    println!();
    println!(
        "Recommendation: Ensure firewall allows UDP/TCP ports {}-{}",
        localdrop_core::DEFAULT_DISCOVERY_PORT,
        localdrop_core::DEFAULT_TRANSFER_PORT_END
    );

    Ok(())
}
