use crate::{
    config::Config,
    contracts::ContractAddresses,
    error::{EthereumError, Result},
    types::*,
};
use alloy_primitives::{Bytes, FixedBytes, TxHash, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types_eth::TransactionReceipt;
use alloy_signer_local::PrivateKeySigner;
use std::sync::Arc;

#[cfg(feature = "database")]
use crate::cache::EthereumCache;

pub struct EthereumClient {
    config: Config,
    http_provider: Arc<dyn Provider>,
    contracts: ContractAddresses,
    signer: Option<PrivateKeySigner>,

    #[cfg(feature = "database")]
    cache: Option<EthereumCache>,
}

impl EthereumClient {
    pub async fn new(config: Config) -> Result<Self> {
        config.validate()?;

        let http_provider = ProviderBuilder::new().on_http(config.network.rpc_url.clone());

        let contracts = ContractAddresses::new(
            config.contract.arithmetic_contract,
            config.contract.verifier_contract,
        );

        let signer = if let Some(signer_config) = &config.signer {
            Some(
                PrivateKeySigner::from_bytes(&FixedBytes::<32>::try_from(hex::decode(&signer_config.private_key)?.as_slice())?)
                    .map_err(|e| EthereumError::Signer(e.to_string()))?,
            )
        } else {
            None
        };

        #[cfg(feature = "database")]
        let cache = if let Ok(database_url) = std::env::var("DATABASE_URL") {
            Some(EthereumCache::new(&database_url).await?)
        } else {
            None
        };

        Ok(Self {
            config,
            http_provider: Arc::new(http_provider),
            contracts,
            signer,

            #[cfg(feature = "database")]
            cache,
        })
    }

    pub async fn update_state(
        &self,
        state_id: FixedBytes<32>,
        new_state_root: FixedBytes<32>,
        proof: Bytes,
        public_values: Bytes,
    ) -> Result<StateUpdate> {
        let _signer = self.signer.as_ref().ok_or_else(|| {
            EthereumError::Config("Signer required for state updates".to_string())
        })?;

        // TODO: Implement proper contract call encoding and transaction sending
        let tx_hash = TxHash::default();

        let state_update = StateUpdate {
            state_id,
            new_state_root,
            proof,
            public_values,
            block_number: Some(0),
            transaction_hash: Some(tx_hash),
        };

        #[cfg(feature = "database")]
        if let Some(cache) = &self.cache {
            cache.store_state_update(&state_update).await?;
        }

        Ok(state_update)
    }

    pub async fn batch_update_states(
        &self,
        updates: Vec<(FixedBytes<32>, FixedBytes<32>, Bytes, Bytes)>,
    ) -> Result<BatchStateUpdate> {
        let _signer = self.signer.as_ref().ok_or_else(|| {
            EthereumError::Config("Signer required for state updates".to_string())
        })?;

        if updates.is_empty() {
            return Err(EthereumError::Config("No updates provided".to_string()));
        }

        let mut state_ids = Vec::new();
        let mut new_state_roots = Vec::new();
        let mut proofs = Vec::new();
        let mut results = Vec::new();
        
        for (state_id, new_state_root, proof, result) in updates {
            state_ids.push(state_id);
            new_state_roots.push(new_state_root);
            proofs.push(proof);
            results.push(result);
        }

        // TODO: Implement proper contract call encoding and transaction sending
        let tx_hash = TxHash::default();
        let success_flags = vec![true; state_ids.len()];

        let batch_update = BatchStateUpdate {
            state_ids,
            new_state_roots,
            proofs,
            results,
            transaction_hash: tx_hash,
            block_number: 0,
            gas_used: U256::ZERO,
            success_flags,
        };

        Ok(batch_update)
    }

    pub async fn verify_proof(&self, _proof: Bytes) -> Result<ProofVerificationResult> {
        // TODO: Implement proper proof verification
        let current_block = self.http_provider.get_block_number().await?;

        Ok(ProofVerificationResult {
            proof_id: FixedBytes::ZERO,
            verified: true,
            result: Some(Bytes::new()),
            block_number: current_block,
            gas_used: U256::ZERO,
            error_message: None,
        })
    }

    pub async fn get_current_state(&self, state_id: FixedBytes<32>) -> Result<Option<StateResponse>> {
        // TODO: Implement state retrieval from contract
        let current_block = self.http_provider.get_block_number().await?;
        
        Ok(Some(StateResponse {
            state_id,
            state_root: FixedBytes::ZERO,
            block_number: current_block,
            timestamp: 0,
            proof_id: Some(FixedBytes::ZERO),
        }))
    }
}

pub type Receipt = TransactionReceipt;