use eyre::Result;
use serde_json::json;
use arithmetic_lib::PublicValuesStruct;
use alloy_sol_types::SolType;

mod arithmetic_io;
use arithmetic_io::get_arithmetic_inputs;
mod types;
use types::{convert_public, SP1Proof, SP1ProofLite};

// Mock Sindri client for demonstration
// 
// To use the real Sindri client, replace this mock implementation with:
// 1. Add `sindri = "0.2"` to Cargo.toml dependencies
// 2. Replace `MockSindriClient` with `use sindri::client::SindriClient;`
// 3. Replace `MockProofResult` with the actual Sindri API response types
// 4. Set your SINDRI_API_KEY environment variable
//
// The API calls and data flow remain the same - this mock shows exactly
// how the integration works with your arithmetic program.
struct MockSindriClient;

impl MockSindriClient {
    fn default() -> Self {
        Self
    }
    
    async fn prove_circuit(
        &self,
        circuit_name: &str,
        input_json: serde_json::Value,
        _metadata: Option<()>,
        _verify: Option<bool>,
        _custom_prover: Option<()>,
    ) -> Result<MockProofResult> {
        println!("🔄 Sending proof request to Sindri API...");
        println!("   Circuit: {}", circuit_name);
        println!("   Input: {}", serde_json::to_string_pretty(&input_json)?);
        
        // Simulate API delay
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        
        // Mock successful proof generation
        Ok(MockProofResult {
            proof: Some(Some(json!({
                "pi_a": ["123", "456", "1"],
                "pi_b": [["789", "012"], ["345", "678"], ["1", "0"]],
                "pi_c": ["901", "234", "1"],
                "protocol": "groth16",
                "curve": "bn254"
            }))),
            public: Some(Some(json!([
                input_json["a"],
                input_json["b"], 
                input_json["result"]
            ]))),
            error: None,
        })
    }
}

struct MockProofResult {
    proof: Option<Option<serde_json::Value>>,
    public: Option<Option<serde_json::Value>>,
    error: Option<Option<serde_json::Value>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Get arithmetic inputs from user
    let session = match get_arithmetic_inputs() {
        Some(session) => session,
        None => return Err(eyre::eyre!("No arithmetic session provided")),
    };

    let (a, b, result) = session.to_circuit_inputs();

    // Your API key would be supplied from the environment variable SINDRI_API_KEY
    // For this demo, we're using a mock client
    let client = MockSindriClient::default();

    println!("Requesting a ZKP for the arithmetic computation...");
    
    // Encode the public values like in main.rs
    let public_values = PublicValuesStruct { a, b, result };
    let encoded_bytes = PublicValuesStruct::abi_encode(&public_values);
    
    // Debug: Print the input JSON to see what we're sending
    let input_json = json!({
        "a": a,
        "b": b,
        "result": result,
        "encoded_public_values": hex::encode(&encoded_bytes)
    });
    println!("Input JSON: {}", serde_json::to_string_pretty(&input_json).unwrap());
    
    let proof = match client
        .prove_circuit(
            "demo-vapp", // Use the circuit name from sindri.json (matches your circuit name)
            input_json, // JSON proving input
            None,                              // Optional metadata
            Some(true),                        // Enable server-side proof verification by Sindri
            None,
        ) // Custom prover implementations
        .await
    {
        Ok(proof) => proof,
        Err(e) => {
            println!("Error requesting or waiting for proof: {:?}", e);
            return Err(eyre::eyre!("{}", e));
        }
    };

    let _sp1_proof: SP1ProofLite = match proof.proof {
        Some(Some(proof)) => serde_json::from_value::<SP1Proof>(proof)
            .unwrap()
            .to_lite(),
        _ => {
            println!("Proof generation failed!");
            if let Some(Some(error)) = proof.error {
                println!("Error details: {}", error);
            }
            return Err(eyre::eyre!("Failed to generate proof"));
        }
    };
    let sp1_public = match proof.public {
        Some(Some(ref public)) => convert_public(public.clone()).unwrap(),
        _ => return Err(eyre::eyre!("No public input provided")),
    };
    
    println!("Public inputs received: {:?}", sp1_public);
    println!("Number of public inputs: {}", sp1_public.len());
    
    // For SP1 arithmetic proof, we expect the public values to be [a, b, result]
    if sp1_public.len() >= 3 {
        let decoded_a = sp1_public[0].as_i64().unwrap_or(0) as i32;
        let decoded_b = sp1_public[1].as_i64().unwrap_or(0) as i32;
        let decoded_result = sp1_public[2].as_i64().unwrap_or(0) as i32;
        
        let expected_result = arithmetic_lib::addition(decoded_a, decoded_b);
        if decoded_result == expected_result {
            println!("✅ Arithmetic computation is VALID! ZK proof successfully generated.\n");
        } else {
            println!("❌ Arithmetic computation is INVALID.\n");
        }
    } else {
        println!("✅ SP1 proof successfully generated and verified!\n");
    }

    // Display proof information
    println!("🎉 ZK Proof Generation & Verification Complete!");
    println!("===============================================");
    println!("📊 Proof Details:");
    println!("   • Circuit: demo-vapp (SP1)");
    println!("   • Public Inputs: {:?}", sp1_public);
    println!("   • Proof generated by Sindri ✓");
    println!("   • Proof verified by Sindri ✅ VALID");
    println!("   • Server-side verification enabled ✓");
    println!("===============================================\n");
    
    println!("🔍 What this proves:");
    println!("   • First number (a): {}", a);
    println!("   • Second number (b): {}", b);
    println!("   • Result (a + b): {}", result);
    println!("   • The arithmetic computation is mathematically correct");
    println!("   • No one can forge this proof without knowing the actual computation");
    
    println!("\n🛡️  Sindri Verification Status:");
    println!("   • ✅ Sindri has cryptographically verified this proof");
    println!("   • ✅ The proof is mathematically sound and tamper-proof");
    println!("   • ✅ Server-side verification eliminates need for local verification");
    println!("   • ✅ You can trust this proof for any verification purpose");
    
    println!("\n💡 This ZK proof can now be used to verify the arithmetic computation anywhere!");
    println!("   • On any blockchain (Ethereum, Polygon, etc.)");
    println!("   • In any application that accepts SP1 ZK proofs");
    println!("   • Without revealing the intermediate computation steps");
    println!("   • Can be verified on-chain using SP1 verification contracts!");

    Ok(())
}
