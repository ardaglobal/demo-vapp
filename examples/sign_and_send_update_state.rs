use alloy_primitives::{Bytes, FixedBytes, Address};
use alloy_provider::ProviderBuilder;
use alloy_signer_local::PrivateKeySigner;
use alloy_sol_types::sol;
use hex;

// Define the contract interface
sol! {
    interface IArithmetic {
        function updateState(
            bytes32 stateId,
            bytes32 newStateRoot,
            bytes proof,
            bytes publicValues
        ) external;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup provider
    let rpc_url = "http://localhost:8545"; // or your RPC URL
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(PrivateKeySigner::random()) // or load from private key
        .on_http(rpc_url.parse()?);

    // 2. Contract address (replace with your deployed contract)
    let contract_address: Address = "0x1234567890123456789012345678901234567890".parse()?;

    // 3. Prepare transaction parameters
    let state_id = FixedBytes::<32>::from([1u8; 32]);
    let new_state_root = FixedBytes::<32>::from([2u8; 32]);
    let proof = Bytes::from(vec![0x01, 0x02, 0x03]); // Your SP1 proof bytes
    let public_values = Bytes::from(15i32.to_be_bytes().to_vec()); // Result as bytes

    // 4. Create contract instance
    let contract = IArithmetic::new(contract_address, &provider);

    println!("Signing and sending updateState transaction...");
    println!("Contract: {}", contract_address);
    println!("State ID: 0x{}", hex::encode(state_id));
    println!("New State Root: 0x{}", hex::encode(new_state_root));

    // 5. Build and send transaction (alloy handles signing automatically)
    let tx_builder = contract.updateState(state_id, new_state_root, proof, public_values);
    
    // Send the transaction - this signs and broadcasts it
    let pending_tx = tx_builder.send().await?;
    
    println!("Transaction sent! Hash: {}", pending_tx.tx_hash());
    
    // 6. Wait for confirmation
    let receipt = pending_tx.get_receipt().await?;
    
    println!("Transaction confirmed!");
    println!("Block number: {}", receipt.block_number.unwrap_or(0));
    println!("Gas used: {}", receipt.gas_used);
    println!("Status: {}", if receipt.status() { "Success" } else { "Failed" });
    
    Ok(())
}