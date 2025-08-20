use alloy_primitives::FixedBytes;
use ethereum_client::{Config, EthereumClient, Result};
use std::str::FromStr;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // Load configuration from environment
    let config = Config::from_env()?;

    // Create ethereum client
    let client = EthereumClient::new_without_validation(config).await?;

    // Example state ID (you would use a real state ID in practice)
    let state_id =
        FixedBytes::from_str("0x1234567890123456789012345678901234567890123456789012345678901234")
            .unwrap_or_default();

    info!(
        "Getting previous state root for state ID: 0x{}",
        hex::encode(state_id.as_slice())
    );

    // Get the previous state root
    match client.get_previous_state_root(state_id).await {
        Ok(previous_root) => {
            if previous_root == FixedBytes::ZERO {
                info!("No previous state root found (returns zero)");
            } else {
                info!(
                    "Previous state root: 0x{}",
                    hex::encode(previous_root.as_slice())
                );
            }
        }
        Err(e) => {
            eprintln!("Error getting previous state root: {e}");
        }
    }

    // Also compare with current state
    match client.get_current_state(state_id).await {
        Ok(Some(current_state)) => {
            info!(
                "Current state root: 0x{}",
                hex::encode(current_state.state_root.as_slice())
            );
        }
        Ok(None) => {
            info!("No current state found");
        }
        Err(e) => {
            eprintln!("Error getting current state: {e}");
        }
    }

    Ok(())
}
