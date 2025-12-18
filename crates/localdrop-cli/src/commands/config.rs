//! Config command implementation.

use anyhow::Result;

use super::{ConfigAction, ConfigArgs};

/// Run the config command.
pub async fn run(args: ConfigArgs) -> Result<()> {
    let mut config = localdrop_core::config::Config::load()?;

    match args.action {
        ConfigAction::Get { key } => {
            let value = get_config_value(&config, &key);
            match value {
                Some(v) => println!("{}: {}", key, v),
                None => println!("Unknown configuration key: {}", key),
            }
        }

        ConfigAction::Set { key, value } => {
            if set_config_value(&mut config, &key, &value)? {
                config.save()?;
                println!("Set {} = {}", key, value);
            } else {
                println!("Unknown configuration key: {}", key);
            }
        }

        ConfigAction::Show => {
            println!();
            println!("LocalDrop Configuration");
            println!("{}", "─".repeat(50));
            println!();
            println!("[general]");
            println!("  device_name = \"{}\"", config.general.device_name);
            println!("  default_expire = \"{:?}\"", config.general.default_expire);
            println!();
            println!("[network]");
            println!("  port = {}", config.network.port);
            println!(
                "  transfer_port_range = \"{}-{}\"",
                config.network.transfer_port_range.0, config.network.transfer_port_range.1
            );
            println!();
            println!("[transfer]");
            println!("  chunk_size = {}", config.transfer.chunk_size);
            println!("  parallel_chunks = {}", config.transfer.parallel_chunks);
            println!("  verify_checksum = {}", config.transfer.verify_checksum);
            println!();
            println!("[security]");
            println!("  require_pin = {}", config.security.require_pin);
            println!("  require_approval = {}", config.security.require_approval);
            println!();
        }

        ConfigAction::Reset => {
            let config = localdrop_core::config::Config::default();
            config.save()?;
            println!("Configuration reset to defaults.");
        }
    }

    Ok(())
}

fn get_config_value(config: &localdrop_core::config::Config, key: &str) -> Option<String> {
    match key {
        "device_name" => Some(config.general.device_name.clone()),
        "default_expire" => Some(format!("{}s", config.general.default_expire.as_secs())),
        "port" => Some(config.network.port.to_string()),
        "chunk_size" => Some(config.transfer.chunk_size.to_string()),
        "parallel_chunks" => Some(config.transfer.parallel_chunks.to_string()),
        "require_pin" => Some(config.security.require_pin.to_string()),
        "require_approval" => Some(config.security.require_approval.to_string()),
        "verify_checksum" => Some(config.transfer.verify_checksum.to_string()),
        _ => None,
    }
}

fn set_config_value(
    config: &mut localdrop_core::config::Config,
    key: &str,
    value: &str,
) -> Result<bool> {
    match key {
        "device_name" => {
            config.general.device_name = value.to_string();
            Ok(true)
        }
        "port" => {
            config.network.port = value.parse()?;
            Ok(true)
        }
        "chunk_size" => {
            config.transfer.chunk_size = value.parse()?;
            Ok(true)
        }
        "parallel_chunks" => {
            config.transfer.parallel_chunks = value.parse()?;
            Ok(true)
        }
        "require_pin" => {
            config.security.require_pin = value.parse()?;
            Ok(true)
        }
        "require_approval" => {
            config.security.require_approval = value.parse()?;
            Ok(true)
        }
        "verify_checksum" => {
            config.transfer.verify_checksum = value.parse()?;
            Ok(true)
        }
        _ => Ok(false),
    }
}
