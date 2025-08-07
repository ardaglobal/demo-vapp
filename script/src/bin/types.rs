use eyre::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct SP1Proof {
    pub pi_a: Vec<String>,
    pub pi_b: Vec<Vec<String>>,
    pub pi_c: Vec<String>,
    pub protocol: String,
    pub curve: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SP1ProofLite {
    pub pi_a: Vec<String>,
    pub pi_b: Vec<Vec<String>>,
    pub pi_c: Vec<String>,
}

impl SP1Proof {
    pub fn to_lite(self) -> SP1ProofLite {
        SP1ProofLite {
            pi_a: self.pi_a,
            pi_b: self.pi_b,
            pi_c: self.pi_c,
        }
    }
}

pub fn convert_public(public: Value) -> Result<Vec<Value>> {
    match public {
        Value::Array(arr) => Ok(arr),
        _ => Err(eyre::eyre!("Expected array for public inputs")),
    }
}