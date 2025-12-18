//! Web command implementation.

use anyhow::Result;

use super::WebArgs;

/// Run the web command.
pub async fn run(args: WebArgs) -> Result<()> {
    println!();
    println!("LocalDrop Web UI");
    println!("{}", "─".repeat(40));
    println!();

    let config = localdrop_core::web::WebServerConfig {
        port: args.port,
        localhost_only: args.localhost_only,
        auth_enabled: args.auth,
        auth_password: if args.auth {
            Some(localdrop_core::web::WebServerConfig::generate_password())
        } else {
            None
        },
    };

    println!("  http://localhost:{}", args.port);
    if !args.localhost_only {
        // TODO: Show actual network IP
        println!("  http://192.168.x.x:{} (for other devices)", args.port);
    }

    if let Some(ref password) = config.auth_password {
        println!();
        println!("  Password: {}", password);
    }

    println!();
    println!("Press Ctrl+C to stop the server.");

    let server = localdrop_core::web::WebServer::new(config);
    server.start().await?;

    tokio::signal::ctrl_c().await?;

    server.stop().await;
    println!();
    println!("Server stopped.");

    Ok(())
}
