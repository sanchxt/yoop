//! Trust command implementation.

use anyhow::Result;

use super::{TrustAction, TrustArgs};

/// Run the trust command.
pub async fn run(args: TrustArgs) -> Result<()> {
    let mut trust_store = localdrop_core::trust::TrustStore::load()?;

    match args.action {
        TrustAction::List => {
            let devices = trust_store.list();
            if devices.is_empty() {
                println!("No trusted devices.");
            } else {
                println!();
                println!("Trusted Devices:");
                println!("{}", "─".repeat(60));
                for device in devices {
                    println!(
                        "  {} - {:?} ({} transfers)",
                        device.device_name, device.trust_level, device.transfer_count
                    );
                }
                println!("{}", "─".repeat(60));
            }
        }

        TrustAction::Remove { device } => {
            let device_id = trust_store
                .find_by_name(&device)
                .map(|d| d.device_id)
                .or_else(|| uuid::Uuid::parse_str(&device).ok());

            if let Some(id) = device_id {
                if trust_store.remove(&id)? {
                    println!("Removed device: {}", device);
                } else {
                    println!("Device not found: {}", device);
                }
            } else {
                println!("Device not found: {}", device);
            }
        }

        TrustAction::Set { device, level } => {
            let trust_level = match level.to_lowercase().as_str() {
                "full" => localdrop_core::config::TrustLevel::Full,
                "ask" | "ask_each_time" => localdrop_core::config::TrustLevel::AskEachTime,
                _ => {
                    anyhow::bail!("Invalid trust level: {}. Use 'full' or 'ask'.", level);
                }
            };

            let device_id = trust_store
                .find_by_name(&device)
                .map(|d| d.device_id)
                .or_else(|| uuid::Uuid::parse_str(&device).ok());

            if let Some(id) = device_id {
                if trust_store.set_trust_level(&id, trust_level)? {
                    println!("Set trust level for {} to {:?}", device, trust_level);
                } else {
                    println!("Device not found: {}", device);
                }
            } else {
                println!("Device not found: {}", device);
            }
        }
    }

    Ok(())
}
