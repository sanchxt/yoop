//! Scan command implementation.

use anyhow::Result;

use super::ScanArgs;

/// Run the scan command.
pub async fn run(args: ScanArgs) -> Result<()> {
    println!();
    println!("Active Shares on Network:");
    println!("{}", "─".repeat(60));
    println!(
        "  {:6}  {:16}  {:6}  {:10}  {:7}",
        "Code", "Device", "Files", "Size", "Expires"
    );
    println!("{}", "─".repeat(60));

    // TODO: Scan network for active shares

    if args.json {
        let output = serde_json::json!({
            "shares": [],
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("  (no active shares found)");
    }

    println!("{}", "─".repeat(60));

    if args.interactive {
        println!();
        println!("Enter a code to connect, or 'q' to quit:");
        // TODO: Interactive mode
    }

    Ok(())
}
