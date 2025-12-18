//! History command implementation.

use anyhow::Result;

use super::HistoryArgs;

/// Run the history command.
pub async fn run(args: HistoryArgs) -> Result<()> {
    if args.clear {
        // TODO: Clear history
        println!("History cleared.");
        return Ok(());
    }

    if args.json {
        let output = serde_json::json!({
            "transfers": [],
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    if let Some(index) = args.details {
        println!();
        println!("Transfer #{}", index);
        println!("{}", "─".repeat(40));
        println!("  (no transfer history)");
        return Ok(());
    }

    println!();
    println!("Recent Transfers:");
    println!("{}", "─".repeat(60));
    println!(
        "  {:12}  {:10}  {:16}  {:6}  {:10}",
        "Date", "Direction", "Device", "Files", "Size"
    );
    println!("{}", "─".repeat(60));

    // TODO: Load and display transfer history
    println!("  (no transfer history)");

    println!("{}", "─".repeat(60));

    Ok(())
}
