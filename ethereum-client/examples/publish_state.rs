use alloy_primitives::{Bytes, FixedBytes};
use clap::Parser;
use ethereum_client::{Config, EthereumClient};

#[derive(Parser, Debug)]
#[command(author, version, about = "Publish state to Arithmetic contract", long_about = None)]
struct Args {
    /// State ID (32-byte hex string with or without 0x prefix)
    #[arg(long, required = true)]
    state_id: String,

    /// New state root (32-byte hex string with or without 0x prefix)
    #[arg(long, required = true)]
    state_root: String,

    /// Proof data (hex string with or without 0x prefix)
    #[arg(long, default_value = "0x01020304")]
    proof: String,

    /// Public values (hex string with or without 0x prefix)
    #[arg(long, default_value = "0x05060708")]
    public_values: String,
}

fn parse_hex_to_fixed_bytes(hex_str: &str) -> Result<FixedBytes<32>, Box<dyn std::error::Error>> {
    let hex_str = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    if hex_str.len() != 64 {
        return Err(format!(
            "Invalid hex length: expected 64 characters, got {}",
            hex_str.len()
        )
        .into());
    }
    let bytes = hex::decode(hex_str)?;
    Ok(FixedBytes::from_slice(&bytes))
}

fn parse_hex_to_bytes(hex_str: &str) -> Result<Bytes, Box<dyn std::error::Error>> {
    let hex_str = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    let bytes = hex::decode(hex_str)?;
    Ok(Bytes::from(bytes))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    println!("📝 Publishing State to Arithmetic Contract");
    println!("==========================================");

    // Parse input parameters
    let state_id = parse_hex_to_fixed_bytes(&args.state_id)?;
    let state_root = parse_hex_to_fixed_bytes(&args.state_root)?;
    let proof = parse_hex_to_bytes(&args.proof)?;
    let public_values = parse_hex_to_bytes(&args.public_values)?;

    println!("📋 Parameters:");
    println!("   State ID: {state_id:?}");
    println!("   State Root: {state_root:?}");
    println!("   Proof: {} bytes", proof.len());
    println!("   Public Values: {} bytes", public_values.len());
    println!();

    // Load configuration and create client
    let config = Config::from_env()?;
    let client = EthereumClient::new(config.clone()).await?;

    println!("✅ Ethereum client initialized");
    println!("   Network: {}", config.network.name);
    println!("   Contract: {}", config.contract.arithmetic_contract);
    if let Some(signer) = &config.signer {
        println!("   Signer: {}", signer.address);
    }
    println!();

    // Publish the state
    println!("🚀 Publishing state to contract...");

    match client
        .update_state(state_id, state_root, proof, public_values)
        .await
    {
        Ok(result) => {
            println!("✅ State published successfully!");
            println!("   Transaction hash: {:?}", result.transaction_hash);
            if let Some(block) = result.block_number {
                println!("   Block number: {block}");
            }
            println!("   State ID: {:?}", result.state_id);
            println!("   New state root: {:?}", result.new_state_root);

            // Provide explorer link
            let tx_hash = result.transaction_hash.unwrap_or(FixedBytes::ZERO);
            println!("🔗 View on Sepolia Etherscan: https://sepolia.etherscan.io/tx/{tx_hash:?}");
        }
        Err(e) => {
            println!("❌ Failed to publish state: {e}");

            let error_str = format!("{e}");
            println!("\n💡 Error Analysis:");
            if error_str.contains("0x7fcdd1f4") {
                println!("   UnauthorizedAccess - Your address is not authorized to write to this contract");
                println!("   Check that ETHEREUM_WALLET_PRIVATE_KEY matches the contract deployer");
            } else if error_str.contains("0xf208777e") {
                println!("   Proof verification failed - The proof data is invalid");
                println!("   You need a valid SP1 proof from the proving system");
            } else {
                println!("   Unknown error - check the transaction details");
            }
        }
    }

    Ok(())
}
