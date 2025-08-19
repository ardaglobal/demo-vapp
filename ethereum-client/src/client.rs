use crate::{
    config::Config,
    contracts::{ContractAddresses, IArithmetic, IArithmeticInstance, ISP1Verifier},
    error::{EthereumError, Result},
    types::{
        BatchStateUpdate, InclusionProof, NetworkStats, ProofVerificationResult, StateHistory,
        StateResponse, StateUpdate,
    },
};
use alloy_network::EthereumWallet;
use alloy_primitives::{Address, Bytes, FixedBytes, U256};
use alloy_provider::{
    fillers::{
        BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller, WalletFiller,
    },
    Identity, Provider, ProviderBuilder, RootProvider,
};
use alloy_rpc_types_eth::{Filter, TransactionReceipt};
use alloy_signer_local::PrivateKeySigner;
use alloy_sol_types::SolEvent;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

// Use a simpler provider type that works with the current Alloy version
type EthProvider = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        WalletFiller<EthereumWallet>,
    >,
    RootProvider,
>;

#[cfg(feature = "database")]
use crate::cache::EthereumCache;
#[cfg(feature = "database")]
use sqlx;

pub struct EthereumClient {
    #[allow(dead_code)]
    config: Config,
    http_provider: EthProvider,
    #[allow(dead_code)]
    contracts: ContractAddresses,
    signer: PrivateKeySigner,

    #[cfg(feature = "database")]
    cache: Option<EthereumCache>,
}

impl EthereumClient {
    pub async fn new(config: Config) -> Result<Self> {
        config.validate()?;

        let signer_config = config
            .signer
            .as_ref()
            .ok_or_else(|| EthereumError::Config("Signer required".to_string()))?;

        let signer = PrivateKeySigner::from_bytes(&FixedBytes::<32>::try_from(
            hex::decode(&signer_config.private_key)?.as_slice(),
        )?)
        .map_err(|e| EthereumError::Signer(e.to_string()))?;

        let wallet = EthereumWallet::from(signer.clone());

        // Create provider with wallet for signing
        let http_provider = ProviderBuilder::new()
            .wallet(wallet)
            .connect_http(config.network.rpc_url.clone());

        let contracts = ContractAddresses::new(
            config.contract.arithmetic_contract,
            config.contract.verifier_contract,
        );

        #[cfg(feature = "database")]
        let cache = if let Ok(database_url) = std::env::var("DATABASE_URL") {
            let pool = sqlx::PgPool::connect(&database_url).await?;
            Some(EthereumCache::new(pool))
        } else {
            None
        };

        Ok(Self {
            config,
            http_provider,
            contracts,
            signer,

            #[cfg(feature = "database")]
            cache,
        })
    }

    pub async fn verify_proof(&self, public_values: Bytes, proof: Bytes) -> Result<i32> {
        let contract = IArithmetic::new(self.contracts.arithmetic, &self.http_provider);
        let call_builder = contract
            .verifyArithmeticProof(public_values.clone(), proof.clone())
            .from(self.signer.address());

        let tx_result = call_builder.call().await.map_err(|e| {
            error!("Failed to send verify proof transaction: {e}");
            EthereumError::Transaction(format!("Transaction failed: {e}"))
        })?;

        Ok(tx_result)
    }

    pub async fn update_state(
        &self,
        state_id: FixedBytes<32>,
        new_state_root: FixedBytes<32>,
        proof: Bytes,
        public_values: Bytes,
    ) -> Result<StateUpdate> {
        // Create contract instance
        let contract = IArithmetic::new(self.contracts.arithmetic, &self.http_provider);

        // Send transaction (wallet handles signing automatically)
        let tx_result = contract
            .updateState(
                state_id,
                new_state_root,
                proof.clone(),
                public_values.clone(),
            )
            .send()
            .await
            .map_err(|e| {
                error!("Failed to send state update transaction: {e}");
                EthereumError::Transaction(format!("Transaction failed: {e}"))
            })?;

        info!("State update transaction sent: {}", tx_result.tx_hash());

        // Wait for confirmation
        let receipt = tx_result.get_receipt().await.map_err(|e| {
            error!("Failed to get transaction receipt: {e}");
            EthereumError::Transaction(format!("Receipt error: {e}"))
        })?;

        let block_number = receipt.block_number.unwrap_or(0);
        let tx_hash = receipt.transaction_hash;

        info!(
            "State update confirmed in block {}: {}",
            block_number,
            hex::encode(tx_hash.as_slice())
        );

        let state_update = StateUpdate {
            state_id,
            new_state_root,
            proof,
            public_values,
            block_number: Some(block_number),
            transaction_hash: Some(tx_hash),
        };

        #[cfg(feature = "database")]
        if let Some(cache) = &self.cache {
            cache.store_state_update(&state_update).await?;
        }

        Ok(state_update)
    }

    pub async fn is_authorized(&self, account: Address) -> Result<bool> {
        let contract = IArithmetic::new(self.contracts.arithmetic, &self.http_provider);
        let call_builder = contract.isAuthorized(account).from(self.signer.address());
        let tx_result = call_builder.call().await.map_err(|e| {
            error!("Failed to send is authorized transaction: {e}");
            EthereumError::Transaction(format!("Transaction failed: {e}"))
        })?;

        Ok(tx_result)
    }

    pub async fn check_proof_exists(&self, proof: Bytes) -> Result<bool> {
        let contract = IArithmetic::new(self.contracts.arithmetic, &self.http_provider);
        let call_builder = contract.proofExists(proof).from(self.signer.address());
        let tx_result = call_builder.call().await.map_err(|e| {
            error!("Failed to send check proof exists transaction: {e}");
            EthereumError::Transaction(format!("Transaction failed: {e}"))
        })?;
        Ok(tx_result)
    }

    pub async fn batch_update_states(
        &self,
        updates: Vec<(FixedBytes<32>, FixedBytes<32>, Bytes, Bytes)>,
    ) -> Result<BatchStateUpdate> {
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

        info!("Batch updating {} states", state_ids.len());

        // Create contract instance
        let contract = IArithmetic::new(self.contracts.arithmetic, &self.http_provider);

        // Send batch transaction (wallet handles signing automatically)
        let tx_result = contract
            .batchUpdateStates(
                state_ids.clone(),
                new_state_roots.clone(),
                proofs.clone(),
                results.clone(),
            )
            .send()
            .await
            .map_err(|e| {
                error!("Failed to send batch update transaction: {e}");
                EthereumError::Transaction(format!("Batch transaction failed: {e}"))
            })?;

        info!("Batch update transaction sent: {}", tx_result.tx_hash());

        // Wait for confirmation
        let receipt = tx_result.get_receipt().await.map_err(|e| {
            error!("Failed to get batch transaction receipt: {e}");
            EthereumError::Transaction(format!("Batch receipt error: {e}"))
        })?;

        let block_number = receipt.block_number.unwrap_or(0);
        let tx_hash = receipt.transaction_hash;
        let gas_used = U256::from(receipt.gas_used);

        info!(
            "Batch update confirmed in block {}: {} (gas used: {})",
            block_number,
            hex::encode(tx_hash.as_slice()),
            gas_used
        );

        // For now, assume all updates succeeded
        // TODO: Parse transaction logs to determine individual success/failure
        let success_flags = vec![true; state_ids.len()];

        let batch_update = BatchStateUpdate {
            state_ids,
            new_state_roots,
            proofs,
            results,
            transaction_hash: tx_hash,
            block_number,
            gas_used,
            success_flags,
        };

        Ok(batch_update)
    }

    pub async fn get_current_state(
        &self,
        state_id: FixedBytes<32>,
    ) -> Result<Option<StateResponse>> {
        let contract = IArithmetic::new(self.contracts.arithmetic, &self.http_provider);

        let state_root = contract
            .getCurrentState(state_id)
            .call()
            .await
            .map_err(|e| {
                warn!("Failed to get current state: {e}");
                EthereumError::Contract(format!("State query failed: {e}"))
            })?;

        let current_block = self.http_provider.get_block_number().await?;

        Ok(Some(StateResponse {
            state_id,
            state_root,
            block_number: current_block,
            timestamp: 0,   // TODO: Get actual timestamp
            proof_id: None, // TODO: Get associated proof ID
        }))
    }

    #[cfg(feature = "database")]
    pub fn with_cache(mut self, cache: EthereumCache) -> Result<Self> {
        self.cache = Some(cache);
        Ok(self)
    }

    pub async fn monitor_events(&self) -> Result<()> {
        info!(
            "Starting event monitoring for contract: {}",
            self.contracts.arithmetic
        );

        let contract = IArithmetic::new(self.contracts.arithmetic, &self.http_provider);

        // Create filter for all contract events
        let _filter = Filter::new()
            .address(self.contracts.arithmetic)
            .from_block(0);

        // Start monitoring loop
        let mut current_block = self.http_provider.get_block_number().await?;

        loop {
            match self.check_for_new_events(current_block, &contract).await {
                Ok(new_block) => {
                    current_block = new_block;
                }
                Err(e) => {
                    error!("Error monitoring events: {}", e);
                }
            }

            // Poll every 12 seconds (Ethereum block time)
            sleep(Duration::from_secs(12)).await;
        }
    }

    async fn check_for_new_events(
        &self,
        last_block: u64,
        _contract: &IArithmeticInstance<&EthProvider>,
    ) -> Result<u64> {
        let current_block = self.http_provider.get_block_number().await?;

        if current_block <= last_block {
            return Ok(last_block);
        }

        debug!(
            "Checking for events from block {} to {}",
            last_block + 1,
            current_block
        );

        // Create filter for new blocks
        let filter = Filter::new()
            .address(self.contracts.arithmetic)
            .from_block(last_block + 1)
            .to_block(current_block);

        let logs = self.http_provider.get_logs(&filter).await.map_err(|e| {
            error!("Failed to get logs: {}", e);
            EthereumError::External(format!("Log retrieval failed: {e}"))
        })?;

        let log_count = logs.len();
        for log in &logs {
            self.process_event_log(log);
        }

        info!(
            "Processed {} events from blocks {} to {}",
            log_count,
            last_block + 1,
            current_block
        );
        Ok(current_block)
    }

    fn process_event_log(&self, log: &alloy_rpc_types_eth::Log) {
        let primitive_log = Self::convert_rpc_log_to_primitive(log);
        self.decode_and_log_event(&primitive_log, log);
    }

    fn convert_rpc_log_to_primitive(log: &alloy_rpc_types_eth::Log) -> alloy_primitives::Log {
        alloy_primitives::Log {
            address: log.address(),
            data: alloy_primitives::LogData::new(log.topics().to_vec(), log.data().data.clone())
                .unwrap_or_else(|| {
                    alloy_primitives::LogData::new_unchecked(
                        log.topics().to_vec(),
                        log.data().data.clone(),
                    )
                }),
        }
    }

    fn decode_and_log_event(
        &self,
        primitive_log: &alloy_primitives::Log,
        original_log: &alloy_rpc_types_eth::Log,
    ) {
        if let Ok(event) = IArithmetic::StateUpdated::decode_log(primitive_log) {
            self.handle_state_updated_event(&event);
        } else if let Ok(event) = IArithmetic::ProofStored::decode_log(primitive_log) {
            Self::handle_proof_stored_event(&event);
        } else if let Ok(event) = IArithmetic::ProofVerified::decode_log(primitive_log) {
            Self::handle_proof_verified_event(&event);
        } else {
            debug!("Unknown event type in log: {:?}", original_log);
        }
    }

    fn handle_state_updated_event(&self, event: &IArithmetic::StateUpdated) {
        info!(
            "StateUpdated event: stateId={}, newState={}, proofId={}, updater={}",
            hex::encode(event.stateId.as_slice()),
            hex::encode(event.newState.as_slice()),
            hex::encode(event.proofId.as_slice()),
            event.updater
        );

        #[cfg(feature = "database")]
        if let Some(_cache) = &self.cache {
            // Store event in cache for later processing
            // TODO: Implement cache.store_event(&event)
        }
    }

    fn handle_proof_stored_event(event: &IArithmetic::ProofStored) {
        info!(
            "ProofStored event: proofId={}, stateId={}, submitter={}",
            hex::encode(event.proofId.as_slice()),
            hex::encode(event.stateId.as_slice()),
            event.submitter
        );
    }

    fn handle_proof_verified_event(event: &IArithmetic::ProofVerified) {
        info!(
            "ProofVerified event: proofId={}, success={}, result={}",
            hex::encode(event.proofId.as_slice()),
            event.success,
            hex::encode(event.result.as_ref())
        );
    }

    pub async fn publish_state_root(
        &self,
        state_id: FixedBytes<32>,
        state_root: FixedBytes<32>,
        proof: Bytes,
        public_values: Bytes,
    ) -> Result<StateUpdate> {
        self.update_state(state_id, state_root, proof, public_values)
            .await
    }

    pub async fn verify_zk_proof(
        &self,
        _proof: Bytes,
        public_values: Bytes,
    ) -> Result<ProofVerificationResult> {
        // TODO: Implement ZK proof verification with public values
        let current_block = self.http_provider.get_block_number().await?;

        Ok(ProofVerificationResult {
            proof_id: FixedBytes::ZERO,
            verified: true,
            result: Some(public_values),
            block_number: current_block,
            gas_used: U256::ZERO,
            error_message: None,
        })
    }

    pub async fn get_verifier_key(&self) -> Result<Bytes> {
        let contract = IArithmetic::new(self.contracts.arithmetic, &self.http_provider);

        let vkey = contract.arithmeticProgramVKey().call().await.map_err(|e| {
            warn!("Failed to get verifier key: {}", e);
            EthereumError::Contract(format!("Verifier key query failed: {e}"))
        })?;

        // Convert FixedBytes<32> to Bytes
        Ok(Bytes::from(vkey.as_slice().to_vec()))
    }

    pub async fn get_proof_result(&self, proof_id: FixedBytes<32>) -> Result<Option<Bytes>> {
        let contract = IArithmetic::new(self.contracts.arithmetic, &self.http_provider);

        let result = contract
            .getStoredResult(proof_id)
            .call()
            .await
            .map_err(|e| {
                warn!("Failed to get proof result: {}", e);
                EthereumError::Contract(format!("Proof result query failed: {e}"))
            })?;

        if result.is_empty() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    pub const fn get_verification_data(&self, _proof_id: FixedBytes<32>) -> Result<Option<Bytes>> {
        // TODO: Implement verification data retrieval from contract
        Ok(Some(Bytes::new()))
    }

    pub async fn verify_proof_independently(
        &self,
        _proof_id: FixedBytes<32>,
    ) -> Result<ProofVerificationResult> {
        // TODO: Implement independent proof verification
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

    pub fn get_historical_states(
        &self,
        state_id: FixedBytes<32>,
        limit: Option<u64>,
    ) -> Result<StateHistory> {
        // TODO: Implement historical states retrieval
        Ok(StateHistory {
            state_id,
            state_roots: vec![FixedBytes::ZERO],
            block_numbers: vec![0],
            timestamps: vec![0],
            proof_ids: vec![Some(FixedBytes::ZERO)],
            limit: limit.unwrap_or(100),
        })
    }

    pub async fn get_network_stats(&self) -> Result<NetworkStats> {
        // TODO: Implement network stats retrieval
        let current_block = self.http_provider.get_block_number().await?;

        Ok(NetworkStats {
            chain_id: 1,
            network_name: "mainnet".to_string(),
            block_number: current_block,
            gas_price: U256::ZERO,
            base_fee: Some(U256::ZERO),
            sync_status: true,
        })
    }

    pub fn check_inclusion_proof(
        &self,
        _leaf_hash: FixedBytes<32>,
        _leaf_index: u64,
        _siblings: Vec<FixedBytes<32>>,
        _root: FixedBytes<32>,
    ) -> Result<InclusionProof> {
        // TODO: Implement inclusion proof checking
        Ok(InclusionProof {
            leaf_hash: FixedBytes::ZERO,
            leaf_index: 0,
            siblings: vec![],
            root: FixedBytes::ZERO,
            verified: false,
        })
    }

    pub async fn get_proof_data(&self, proof_id: FixedBytes<32>) -> Result<Option<Bytes>> {
        let contract = IArithmetic::new(self.contracts.arithmetic, &self.http_provider);

        let proof_data = contract
            .getStoredProof(proof_id)
            .call()
            .await
            .map_err(|e| {
                warn!("Failed to get proof data: {}", e);
                EthereumError::Contract(format!("Proof data query failed: {e}"))
            })?;

        if proof_data.is_empty() {
            Ok(None)
        } else {
            Ok(Some(proof_data))
        }
    }

    pub async fn get_state_root(&self, state_id: FixedBytes<32>) -> Result<FixedBytes<32>> {
        let contract = IArithmetic::new(self.contracts.arithmetic, &self.http_provider);

        let state_root = contract
            .getCurrentState(state_id)
            .call()
            .await
            .map_err(|e| {
                warn!("Failed to get state root: {}", e);
                EthereumError::Contract(format!("State root query failed: {e}"))
            })?;

        Ok(state_root)
    }

    pub fn get_state_proof_history(
        &self,
        _state_id: FixedBytes<32>,
    ) -> Result<Vec<FixedBytes<32>>> {
        // TODO: Implement state proof history retrieval from contract
        Ok(vec![FixedBytes::ZERO])
    }

    pub async fn get_verifier_version(&self) -> Result<String> {
        let verifier_contract = ISP1Verifier::new(self.contracts.verifier, &self.http_provider);

        let version = verifier_contract.VERSION().call().await.map_err(|e| {
            warn!("Failed to get verifier version: {}", e);
            EthereumError::Contract(format!("Verifier version query failed: {e}"))
        })?;

        Ok(version)
    }
}

pub type Receipt = TransactionReceipt;
