//! Update command implementation.

use anyhow::Result;
use serde_json::json;

use super::UpdateArgs;
use yoop_core::config::Config;
use yoop_core::error::Error as YoopError;
use yoop_core::migration::MigrationManager;
use yoop_core::update::package_manager::PackageManager;
use yoop_core::update::version_check::VersionChecker;
use yoop_core::update::SchemaVersion;

#[allow(clippy::too_many_lines)]
pub async fn run(args: UpdateArgs) -> Result<()> {
    if args.rollback {
        return handle_rollback(args).await;
    }

    let mut config = Config::load()?;

    let checker = VersionChecker::new();
    let status = checker
        .check()
        .await
        .inspect_err(|e| handle_error(&YoopError::UpdateCheckFailed(e.to_string())))?;

    if args.check {
        return handle_check(&status, &args);
    }

    if !status.update_available && !args.force {
        if args.json {
            let output = json!({
                "current_version": status.current_version.to_string(),
                "latest_version": status.latest_version.to_string(),
                "update_available": false,
                "message": "Already on the latest version",
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else if !args.quiet {
            println!("Already on the latest version ({})", status.current_version);
        }
        return Ok(());
    }

    if !args.quiet {
        println!();
        println!("Yoop Update");
        println!("{}", "═".repeat(60));
        println!();
        println!("Checking for updates...");
        println!("  Current: {}", status.current_version);
        println!("  Latest:  {}", status.latest_version);
        println!();
    }

    if let Some(pm_str) = &args.package_manager {
        let pm_kind = match pm_str.to_lowercase().as_str() {
            "npm" => yoop_core::config::PackageManagerKind::Npm,
            "pnpm" => yoop_core::config::PackageManagerKind::Pnpm,
            "yarn" => yoop_core::config::PackageManagerKind::Yarn,
            "bun" => yoop_core::config::PackageManagerKind::Bun,
            _ => anyhow::bail!("Invalid package manager. Use: npm, pnpm, yarn, or bun"),
        };
        config.update.package_manager = Some(pm_kind);
    }

    let pm = PackageManager::detect(&config.update).map_err(|e| {
        if let YoopError::Internal(ref msg) = e {
            if msg.contains("not found in PATH") {
                handle_error(&YoopError::PackageManagerNotFound("npm".to_string()));
            }
        }
        e
    })?;

    if !args.quiet {
        println!("Creating backup...");
    }

    let data_dir = Config::config_path()
        .parent()
        .ok_or_else(|| anyhow::anyhow!("unable to determine config directory"))?
        .to_path_buf();

    let migration_manager = MigrationManager::new(data_dir.clone());

    let backup_manager = yoop_core::migration::BackupManager::new(data_dir);
    let backup_id = backup_manager
        .create_backup(&status.current_version.to_string())
        .inspect_err(|e| handle_error(&YoopError::BackupFailed(e.to_string())))?;

    if !args.quiet {
        println!("  Backup ID: {backup_id}");
        println!();
    }

    let current_schema = SchemaVersion::parse(&status.current_version.to_string())?;
    let target_schema = SchemaVersion::parse(&status.latest_version.to_string())?;

    let pending_migrations = migration_manager.get_pending(&current_schema, &target_schema);

    if !pending_migrations.is_empty() {
        if !args.quiet {
            println!("Running migrations...");
        }

        migration_manager
            .run(&current_schema, &target_schema, false)
            .inspect_err(|e| {
                handle_error(&YoopError::MigrationFailed {
                    from: current_schema.to_string(),
                    to: target_schema.to_string(),
                    reason: e.to_string(),
                });
            })?;

        if !args.quiet {
            for migration in pending_migrations {
                println!(
                    "  ✓ {} → {}: {}",
                    migration.from_version(),
                    migration.to_version(),
                    migration.description()
                );
            }
            println!();
        }
    }

    if !args.quiet {
        println!("Updating via {}...", pm);
    }

    let cmd_parts = pm.update_command("yoop", None);
    let mut cmd = std::process::Command::new(&cmd_parts[0]);
    for arg in &cmd_parts[1..] {
        cmd.arg(arg);
    }

    if !args.quiet {
        println!("  $ {}", cmd_parts.join(" "));
        println!();
    }

    let output = cmd
        .output()
        .inspect_err(|e| handle_error(&YoopError::UpdateCommandFailed(e.to_string())))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        handle_error(&YoopError::UpdateCommandFailed(stderr.to_string()));
        anyhow::bail!("Update command failed: {}", stderr);
    }

    if !args.quiet {
        println!("Verifying installation...");
    }

    let verify = std::process::Command::new("yoop")
        .arg("--version")
        .output()?;

    if verify.status.success() {
        let version_output = String::from_utf8_lossy(&verify.stdout);
        let new_version = version_output.trim().trim_start_matches("yoop ");

        if !args.quiet {
            println!("  ✓ yoop {new_version} installed successfully");
            println!();
            println!("Update complete!");
            println!();
        } else if args.json {
            let output = json!({
                "success": true,
                "previous_version": status.current_version.to_string(),
                "new_version": new_version,
                "backup_id": backup_id,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
    } else {
        handle_error(&YoopError::UpdateCommandFailed(
            "verification failed".to_string(),
        ));
        anyhow::bail!("Failed to verify installation");
    }

    Ok(())
}

#[allow(clippy::too_many_lines)]
async fn handle_rollback(args: UpdateArgs) -> Result<()> {
    if !args.quiet {
        println!();
        println!("Yoop Rollback");
        println!("{}", "═".repeat(60));
        println!();
    }

    let data_dir = Config::config_path()
        .parent()
        .ok_or_else(|| anyhow::anyhow!("unable to determine config directory"))?
        .to_path_buf();

    let migration_manager = MigrationManager::new(data_dir);

    let backups = migration_manager.list_backups().inspect_err(|e| {
        handle_error(&YoopError::RollbackFailed(format!(
            "failed to list backups: {e}"
        )));
    })?;

    if backups.is_empty() {
        handle_error(&YoopError::NoBackupAvailable);
        anyhow::bail!("No backup available for rollback");
    }

    if !args.quiet {
        println!("Available backups:");
        for (i, backup) in backups.iter().enumerate() {
            let time_ago = format_time_ago(backup.timestamp);
            println!(
                "  {}. {} ({}, {})",
                i + 1,
                backup.app_version,
                backup.timestamp.format("%Y-%m-%d %H:%M"),
                time_ago
            );
        }
        println!();
    }

    let latest_backup = migration_manager
        .latest_backup()
        .inspect_err(|e| {
            handle_error(&YoopError::RollbackFailed(format!(
                "failed to get latest backup: {e}"
            )));
        })?
        .ok_or_else(|| {
            handle_error(&YoopError::NoBackupAvailable);
            anyhow::anyhow!("No backup available")
        })?;

    if !args.quiet {
        println!("Restoring from backup {}...", latest_backup.id);
    }

    migration_manager
        .rollback(&latest_backup.id)
        .inspect_err(|e| handle_error(&YoopError::RollbackFailed(e.to_string())))?;

    if !args.quiet {
        for file in &latest_backup.files {
            println!("  ✓ {file}");
        }
        println!();
    }

    let target_version = &latest_backup.app_version.to_string();

    let config = Config::load()?;
    let pm = PackageManager::detect(&config.update)?;

    if !args.quiet {
        println!("Installing yoop@{target_version}...");
    }

    let cmd_parts = pm.install_command("yoop", target_version);
    let mut cmd = std::process::Command::new(&cmd_parts[0]);
    for arg in &cmd_parts[1..] {
        cmd.arg(arg);
    }

    if !args.quiet {
        println!("  $ {}", cmd_parts.join(" "));
        println!();
    }

    let output = cmd.output().inspect_err(|e| {
        handle_error(&YoopError::RollbackFailed(format!(
            "failed to install previous version: {e}"
        )));
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        handle_error(&YoopError::RollbackFailed(format!(
            "installation failed: {stderr}"
        )));
        anyhow::bail!("Failed to install previous version: {}", stderr);
    }

    if !args.quiet {
        println!("Rollback complete! Now running yoop {target_version}");
        println!();
    } else if args.json {
        let output = json!({
            "success": true,
            "rolled_back_to": target_version,
            "backup_id": latest_backup.id,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    }

    Ok(())
}

fn handle_check(
    status: &yoop_core::update::version_check::UpdateStatus,
    args: &UpdateArgs,
) -> Result<()> {
    if args.json {
        let output = json!({
            "current_version": status.current_version.to_string(),
            "latest_version": status.latest_version.to_string(),
            "update_available": status.update_available,
            "release_url": status.release_url,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if args.quiet {
        if status.update_available {
            println!("{}", status.latest_version);
        }
    } else {
        println!();
        println!("Yoop Update Check");
        println!("{}", "═".repeat(60));
        println!();
        println!("  Current version: {}", status.current_version);
        println!("  Latest version:  {}", status.latest_version);
        println!();
        if status.update_available {
            println!("  Status: Update available");
            println!();
            println!("Run 'yoop update' to upgrade.");
        } else {
            println!("  Status: Up to date");
        }
        println!();
    }
    Ok(())
}

fn handle_error(err: &YoopError) {
    eprintln!("Error: {err}");

    if let Some(suggestion) = err.suggestion() {
        eprintln!();
        eprintln!("Suggestion:");
        for line in suggestion.lines() {
            eprintln!("  {line}");
        }
    }
}

fn format_time_ago(timestamp: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(timestamp);

    if duration.num_days() > 0 {
        let days = duration.num_days();
        if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{days} days ago")
        }
    } else if duration.num_hours() > 0 {
        let hours = duration.num_hours();
        if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{hours} hours ago")
        }
    } else if duration.num_minutes() > 0 {
        let minutes = duration.num_minutes();
        if minutes == 1 {
            "1 minute ago".to_string()
        } else {
            format!("{minutes} minutes ago")
        }
    } else {
        "just now".to_string()
    }
}
