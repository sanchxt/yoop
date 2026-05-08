//! Trust command implementation.

use std::collections::HashSet;
use std::io::{self, Write};
use std::net::{IpAddr, SocketAddr};
use std::process::Command;
use std::time::Duration;

use anyhow::{bail, Context, Result};

use yoop_core::config::TrustLevel;
use yoop_core::connection::parse_host_address_with_default_port;
use yoop_core::discovery::HybridListener;
use yoop_core::pairing::{self, PairingConfig, PairingIdentity, PairingListener};
use yoop_core::trust::{TrustStore, TrustedDevice};

use super::{TrustAction, TrustArgs};
use crate::ui::parse_duration;

#[derive(Debug, Clone)]
struct PairingCandidate {
    device_name: String,
    address: SocketAddr,
    source: String,
}

/// Run the trust command.
pub async fn run(args: TrustArgs) -> Result<()> {
    let mut trust_store = TrustStore::load()?;

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

        TrustAction::Pair {
            listen,
            host,
            scan,
            port,
            trust_port,
            level,
            yes,
            json,
        } => {
            run_pair(
                &mut trust_store,
                PairArgs {
                    listen,
                    host,
                    scan,
                    port,
                    trust_port,
                    level,
                    yes,
                    json,
                },
            )
            .await?;
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
            let trust_level = parse_trust_level(&level)?;

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

struct PairArgs {
    listen: bool,
    host: Option<String>,
    scan: String,
    port: u16,
    trust_port: u16,
    level: String,
    yes: bool,
    json: bool,
}

async fn run_pair(trust_store: &mut TrustStore, args: PairArgs) -> Result<()> {
    if args.listen && args.host.is_some() {
        bail!("--listen cannot be used with --host");
    }

    let global_config = super::load_config();
    let pairing_config = PairingConfig {
        pairing_port: args.port,
        trust_port: args.trust_port,
        discovery_port: global_config.network.port,
        device_name: global_config.general.device_name,
        ..PairingConfig::default()
    };

    if args.listen {
        return run_pair_listener(
            trust_store,
            pairing_config,
            &args.level,
            args.yes,
            args.json,
        )
        .await;
    }

    if let Some(host) = args.host {
        let addr = parse_host_address_with_default_port(&host, args.port)?;
        return pair_with_address(
            trust_store,
            addr,
            pairing_config,
            &args.level,
            args.yes,
            args.json,
        )
        .await;
    }

    run_pair_scan(
        trust_store,
        pairing_config,
        &args.scan,
        &args.level,
        args.yes,
        args.json,
    )
    .await
}

async fn run_pair_listener(
    trust_store: &mut TrustStore,
    pairing_config: PairingConfig,
    level: &str,
    yes: bool,
    json: bool,
) -> Result<()> {
    let listener = PairingListener::bind(pairing_config).await?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status": "listening",
                "pairing_port": listener.pairing_port(),
            }))?
        );
    } else {
        println!();
        println!("Yoop Trust Pairing");
        println!("{}", "-".repeat(37));
        println!("  Listening on pairing port {}.", listener.pairing_port());
        println!("  On the other device, run: yoop trust pair");
        println!();
    }

    loop {
        let pending = match listener.wait_for_peer().await {
            Ok(pending) => pending,
            Err(yoop_core::Error::ConnectionRejected) => {
                if !json {
                    println!("  Pairing probe/rejection received, still listening...");
                }
                continue;
            }
            Err(e) => {
                if !json {
                    eprintln!("  Pairing attempt failed: {}", e);
                    eprintln!("  Still listening...");
                }
                continue;
            }
        };

        let peer = pending.peer().clone();
        if !json {
            display_pairing_identity("Incoming pairing request", &peer);
        }

        let accepted = yes || prompt_yes_no("Trust this device?", true)?;
        if !accepted {
            let _ = pending
                .finish(false, Some("rejected by user".to_string()))
                .await;
            if !json {
                println!("  Pairing rejected.");
            }
            continue;
        }

        let trust_level = choose_trust_level(level, yes)?;
        let peer = pending.finish(true, None).await?;
        save_trusted_peer(trust_store, &peer, trust_level)?;
        output_pairing_success(&peer, json)?;
        break;
    }

    listener.shutdown().await;
    Ok(())
}

async fn run_pair_scan(
    trust_store: &mut TrustStore,
    pairing_config: PairingConfig,
    scan: &str,
    level: &str,
    yes: bool,
    json: bool,
) -> Result<()> {
    let scan_duration = parse_duration(scan)
        .context("Invalid scan duration. Use formats like '5s', '10s', '30s'")?;

    if !json {
        println!();
        println!("Scanning for Yoop pairing listeners ({scan})...");
        println!();
    }

    let mut candidates = discover_lan_pairing_candidates(&pairing_config, scan_duration).await?;
    candidates.extend(discover_tailscale_pairing_candidates(&pairing_config).await);
    dedupe_candidates(&mut candidates);

    if candidates.is_empty() {
        if json {
            output_candidates_json("no_devices", &candidates)?;
        } else {
            println!("No pairing listeners found.");
            println!("Run `yoop trust pair --listen` on the other device, then try again.");
        }
        return Ok(());
    }

    let selected = if json {
        match choose_json_candidate(&candidates) {
            Ok(selected) => selected,
            Err(error) => {
                output_candidates_json("selection_required", &candidates)?;
                return Err(error);
            }
        }
    } else {
        display_candidates(&candidates);
        choose_candidate(&candidates)?
    };

    pair_with_address(
        trust_store,
        candidates[selected].address,
        pairing_config,
        level,
        yes,
        json,
    )
    .await
}

async fn pair_with_address(
    trust_store: &mut TrustStore,
    addr: SocketAddr,
    pairing_config: PairingConfig,
    level: &str,
    yes: bool,
    json: bool,
) -> Result<()> {
    let pending = pairing::connect(addr, pairing_config).await?;
    let peer = pending.peer().clone();

    if !json {
        display_pairing_identity("Found pairing device", &peer);
    }

    let accepted = yes || prompt_yes_no("Trust this device?", true)?;
    if !accepted {
        pending.reject("rejected by user").await?;
        if !json {
            println!("  Pairing rejected.");
        }
        return Ok(());
    }

    let trust_level = choose_trust_level(level, yes)?;
    let peer = pending.accept().await?;
    save_trusted_peer(trust_store, &peer, trust_level)?;
    output_pairing_success(&peer, json)
}

async fn discover_lan_pairing_candidates(
    pairing_config: &PairingConfig,
    duration: Duration,
) -> Result<Vec<PairingCandidate>> {
    let listener = HybridListener::new(pairing_config.discovery_port).await?;
    let shares = listener.scan(duration).await;

    Ok(shares
        .into_iter()
        .filter(|share| {
            share
                .packet
                .supports
                .iter()
                .any(|support| support.eq_ignore_ascii_case("pairing"))
        })
        .map(|share| PairingCandidate {
            device_name: share.packet.device_name,
            address: SocketAddr::new(share.source.ip(), share.packet.transfer_port),
            source: "lan".to_string(),
        })
        .collect())
}

async fn discover_tailscale_pairing_candidates(
    pairing_config: &PairingConfig,
) -> Vec<PairingCandidate> {
    let Ok(output) = Command::new("tailscale")
        .args(["status", "--json"])
        .output()
    else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    let Ok(status) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return Vec::new();
    };

    let Some(peers) = status.get("Peer").and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };

    let mut candidates = Vec::new();
    for peer in peers.values() {
        if matches!(
            peer.get("Online").and_then(serde_json::Value::as_bool),
            Some(false)
        ) {
            continue;
        }

        let display_name = tailscale_peer_name(peer);
        let Some(ips) = peer
            .get("TailscaleIPs")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };

        for ip_value in ips {
            let Some(ip_str) = ip_value.as_str() else {
                continue;
            };
            let Ok(ip) = ip_str.parse::<IpAddr>() else {
                continue;
            };
            let addr = SocketAddr::new(ip, pairing_config.pairing_port);

            if let Ok(identity) =
                pairing::probe(addr, pairing_config.clone(), Duration::from_millis(900)).await
            {
                candidates.push(PairingCandidate {
                    device_name: identity.device_name,
                    address: addr,
                    source: "tailscale".to_string(),
                });
            } else if let Some(name) = display_name.as_ref() {
                tracing::debug!("No Yoop pairing listener on Tailscale peer {}", name);
            }
        }
    }

    candidates
}

fn tailscale_peer_name(peer: &serde_json::Value) -> Option<String> {
    peer.get("HostName")
        .and_then(serde_json::Value::as_str)
        .or_else(|| peer.get("DNSName").and_then(serde_json::Value::as_str))
        .map(|name| name.trim_end_matches('.').to_string())
}

fn dedupe_candidates(candidates: &mut Vec<PairingCandidate>) {
    let mut seen = HashSet::new();
    candidates.retain(|candidate| seen.insert(candidate.address));
    candidates.sort_by(|a, b| {
        a.device_name
            .to_lowercase()
            .cmp(&b.device_name.to_lowercase())
            .then_with(|| a.address.cmp(&b.address))
    });
}

fn display_candidates(candidates: &[PairingCandidate]) {
    println!("Discovered Yoop devices:");
    println!();
    for (index, candidate) in candidates.iter().enumerate() {
        println!(
            "  {}. {:<24} {:<22} {}",
            index + 1,
            candidate.device_name,
            candidate.address,
            candidate.source
        );
    }
    println!();
}

fn choose_candidate(candidates: &[PairingCandidate]) -> Result<usize> {
    if candidates.len() == 1 {
        return Ok(0);
    }

    loop {
        print!("Trust which device? [1-{}] ", candidates.len());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if let Ok(index) = input.parse::<usize>() {
            if (1..=candidates.len()).contains(&index) {
                return Ok(index - 1);
            }
        }

        println!("Please enter a number between 1 and {}.", candidates.len());
    }
}

fn choose_json_candidate(candidates: &[PairingCandidate]) -> Result<usize> {
    match candidates.len() {
        1 => Ok(0),
        0 => bail!("No pairing listeners found."),
        count => bail!(
            "Found {count} pairing listeners. Run without --json to choose interactively, or pass --host IP:PORT."
        ),
    }
}

fn display_pairing_identity(title: &str, peer: &PairingIdentity) {
    println!();
    println!("{title}:");
    println!("  Name:       {}", peer.device_name);
    println!("  Device ID:  {}", peer.device_id);
    println!("  Address:    {}", peer.address);
    println!();
}

fn prompt_yes_no(question: &str, default_yes: bool) -> Result<bool> {
    let prompt = if default_yes { "[Y/n]" } else { "[y/N]" };
    print!("  {question} {prompt} ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input.is_empty() {
        return Ok(default_yes);
    }

    Ok(input == "y" || input == "yes")
}

fn choose_trust_level(level: &str, yes: bool) -> Result<TrustLevel> {
    if yes {
        return parse_trust_level(level);
    }

    println!("  Trust level:");
    println!("    (1) Full - auto-accept trusted connections");
    println!("    (2) Ask each time - confirm before trusted connections");
    print!("  Choose [1]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if input.trim() == "2" {
        Ok(TrustLevel::AskEachTime)
    } else {
        Ok(TrustLevel::Full)
    }
}

fn parse_trust_level(level: &str) -> Result<TrustLevel> {
    match level.to_lowercase().as_str() {
        "full" => Ok(TrustLevel::Full),
        "ask" | "ask_each_time" => Ok(TrustLevel::AskEachTime),
        _ => bail!("Invalid trust level: {}. Use 'full' or 'ask'.", level),
    }
}

fn save_trusted_peer(
    trust_store: &mut TrustStore,
    peer: &PairingIdentity,
    level: TrustLevel,
) -> Result<()> {
    let mut device = TrustedDevice::new(
        peer.device_id,
        peer.device_name.clone(),
        peer.public_key.clone(),
    )
    .with_trust_level(level)
    .with_address(peer.address.ip(), peer.address.port());
    device.transfer_count = 0;

    trust_store.add(device)?;
    Ok(())
}

fn output_pairing_success(peer: &PairingIdentity, json: bool) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status": "paired",
                "device": peer.device_name,
                "device_id": peer.device_id.to_string(),
                "address": peer.address.to_string(),
            }))?
        );
    } else {
        println!();
        println!("  Device trusted: {} ({})", peer.device_name, peer.address);
        println!(
            "  You can now use: yoop clipboard sync --device \"{}\"",
            peer.device_name
        );
        println!();
    }

    Ok(())
}

fn output_candidates_json(status: &str, candidates: &[PairingCandidate]) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "status": status,
            "devices": candidates.iter().map(|candidate| serde_json::json!({
                "name": candidate.device_name,
                "address": candidate.address.to_string(),
                "source": candidate.source,
            })).collect::<Vec<_>>()
        }))?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn candidate(port: u16) -> PairingCandidate {
        PairingCandidate {
            device_name: format!("device-{port}"),
            address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
            source: "test".to_string(),
        }
    }

    #[test]
    fn json_scan_selects_single_candidate() {
        let candidates = vec![candidate(17777)];

        assert_eq!(choose_json_candidate(&candidates).unwrap(), 0);
    }

    #[test]
    fn json_scan_requires_explicit_selection_for_multiple_candidates() {
        let candidates = vec![candidate(17777), candidate(17778)];

        let error = choose_json_candidate(&candidates).unwrap_err();
        assert!(error
            .to_string()
            .contains("Run without --json to choose interactively"));
    }
}
