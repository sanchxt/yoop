//! Send command implementation (trusted devices).

use anyhow::Result;

use super::SendArgs;

/// Run the send command.
pub async fn run(args: SendArgs) -> Result<()> {
    if !args.quiet {
        println!();
        println!("LocalDrop v{}", localdrop_core::VERSION);
        println!("{}", "─".repeat(37));
        println!();
        println!("  Sending to trusted device: {}", args.device);
    }

    // TODO: Load trust store and find device
    let _trust_store = localdrop_core::trust::TrustStore::load()?;

    // TODO: Connect directly to trusted device and transfer

    Ok(())
}
