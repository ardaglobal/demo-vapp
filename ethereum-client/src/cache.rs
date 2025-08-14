#[cfg(feature = "database")]
use crate::{
    error::Result,
    types::*,
};
use sqlx::{PgPool, Row};
use tracing::{debug, info};
use uuid::Uuid;

#[cfg(feature = "database")]
#[derive(Clone)]
pub struct EthereumCache {
    pool: PgPool,
}

#[cfg(feature = "database")]
impl EthereumCache {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn initialize(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS ethereum_state_updates (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                state_id BYTEA NOT NULL,
                new_state_root BYTEA NOT NULL,
                proof BYTEA NOT NULL,
                public_values BYTEA NOT NULL,
                block_number BIGINT,
                transaction_hash BYTEA,
                created_at TIMESTAMPTZ DEFAULT NOW()
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS ethereum_proof_submissions (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                proof_id BYTEA NOT NULL UNIQUE,
                state_id BYTEA NOT NULL,
                proof BYTEA NOT NULL,
                result BYTEA NOT NULL,
                submitter BYTEA NOT NULL,
                block_number BIGINT NOT NULL,
                transaction_hash BYTEA NOT NULL,
                gas_used NUMERIC NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW()
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS ethereum_contract_events (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                event_type TEXT NOT NULL,
                state_id BYTEA,
                proof_id BYTEA,
                block_number BIGINT NOT NULL,
                transaction_hash BYTEA NOT NULL,
                log_index BIGINT NOT NULL,
                timestamp BIGINT NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW()
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS ethereum_network_stats (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                chain_id BIGINT NOT NULL,
                block_number BIGINT NOT NULL,
                gas_price NUMERIC NOT NULL,
                base_fee NUMERIC,
                network_name TEXT NOT NULL,
                sync_status BOOLEAN NOT NULL,
                recorded_at TIMESTAMPTZ DEFAULT NOW()
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        // Create indexes for better query performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_state_updates_state_id ON ethereum_state_updates(state_id)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_state_updates_block_number ON ethereum_state_updates(block_number)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_proof_submissions_proof_id ON ethereum_proof_submissions(proof_id)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_proof_submissions_state_id ON ethereum_proof_submissions(state_id)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_events_block_number ON ethereum_contract_events(block_number)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_events_event_type ON ethereum_contract_events(event_type)")
            .execute(&self.pool)
            .await?;

        info!("Ethereum cache database initialized successfully");
        Ok(())
    }

    pub async fn store_state_update(&self, update: &StateUpdate) -> Result<Uuid> {
        let id = Uuid::new_v4();
        
        sqlx::query(
            r#"
            INSERT INTO ethereum_state_updates (
                id, state_id, new_state_root, proof, public_values, 
                block_number, transaction_hash
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#
        )
        .bind(id)
        .bind(update.state_id.as_slice())
        .bind(update.new_state_root.as_slice())
        .bind(update.proof.as_ref())
        .bind(update.public_values.as_ref())
        .bind(update.block_number.map(|n| n as i64))
        .bind(update.transaction_hash.as_ref().map(|h| h.0.to_vec()))
        .execute(&self.pool)
        .await?;

        debug!("Stored state update with ID: {}", id);
        Ok(id)
    }

    pub async fn store_proof_submission(&self, submission: &ProofSubmission) -> Result<Uuid> {
        let id = Uuid::new_v4();
        
        sqlx::query(
            r#"
            INSERT INTO ethereum_proof_submissions (
                id, proof_id, state_id, proof, result, submitter,
                block_number, transaction_hash, gas_used
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#
        )
        .bind(id)
        .bind(submission.proof_id.as_slice())
        .bind(submission.state_id.as_slice())
        .bind(submission.proof.as_ref())
        .bind(submission.result.as_ref())
        .bind(submission.submitter.as_slice())
        .bind(submission.block_number as i64)
        .bind(submission.transaction_hash.as_slice())
        .bind(submission.gas_used.to_string())
        .execute(&self.pool)
        .await?;

        debug!("Stored proof submission with ID: {}", id);
        Ok(id)
    }

    pub async fn store_event(&self, event: &ContractEvent) -> Result<Uuid> {
        let id = Uuid::new_v4();
        
        sqlx::query(
            r#"
            INSERT INTO ethereum_contract_events (
                id, event_type, state_id, proof_id, block_number,
                transaction_hash, log_index, timestamp
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#
        )
        .bind(id)
        .bind(&event.event_type)
        .bind(event.state_id.as_ref().map(|s| s.0.to_vec()))
        .bind(event.proof_id.as_ref().map(|p| p.0.to_vec()))
        .bind(event.block_number as i64)
        .bind(event.transaction_hash.as_slice())
        .bind(event.log_index as i64)
        .bind(event.timestamp as i64)
        .execute(&self.pool)
        .await?;

        debug!("Stored contract event with ID: {}", id);
        Ok(id)
    }

    pub async fn store_network_stats(&self, stats: &NetworkStats) -> Result<Uuid> {
        let id = Uuid::new_v4();
        
        sqlx::query(
            r#"
            INSERT INTO ethereum_network_stats (
                id, chain_id, block_number, gas_price, base_fee,
                network_name, sync_status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#
        )
        .bind(id)
        .bind(stats.chain_id as i64)
        .bind(stats.block_number as i64)
        .bind(stats.gas_price.to_string())
        .bind(stats.base_fee.map(|f| f.to_string()))
        .bind(&stats.network_name)
        .bind(stats.sync_status)
        .execute(&self.pool)
        .await?;

        debug!("Stored network stats with ID: {}", id);
        Ok(id)
    }

    pub async fn get_state_updates_by_state_id(&self, state_id: StateId, limit: Option<i64>) -> Result<Vec<StateUpdate>> {
        let query = if let Some(limit) = limit {
            sqlx::query(
                r#"
                SELECT state_id, new_state_root, proof, public_values, 
                       block_number, transaction_hash
                FROM ethereum_state_updates 
                WHERE state_id = $1 
                ORDER BY created_at DESC 
                LIMIT $2
                "#
            )
            .bind(state_id.as_slice())
            .bind(limit)
        } else {
            sqlx::query(
                r#"
                SELECT state_id, new_state_root, proof, public_values, 
                       block_number, transaction_hash
                FROM ethereum_state_updates 
                WHERE state_id = $1 
                ORDER BY created_at DESC
                "#
            )
            .bind(state_id.as_slice())
        };

        let rows = query.fetch_all(&self.pool).await?;
        
        let mut updates = Vec::new();
        for row in rows {
            updates.push(StateUpdate {
                state_id: StateId::from_slice(row.get::<&[u8], _>("state_id")),
                new_state_root: StateRoot::from_slice(row.get::<&[u8], _>("new_state_root")),
                proof: alloy_primitives::Bytes::from(row.get::<Vec<u8>, _>("proof")),
                public_values: alloy_primitives::Bytes::from(row.get::<Vec<u8>, _>("public_values")),
                block_number: row.get::<Option<i64>, _>("block_number").map(|n| n as u64),
                transaction_hash: row.get::<Option<&[u8]>, _>("transaction_hash")
                    .map(|h| alloy_primitives::FixedBytes::from_slice(h)),
            });
        }

        Ok(updates)
    }

    pub async fn get_proof_by_id(&self, proof_id: ProofId) -> Result<Option<ProofSubmission>> {
        let row = sqlx::query(
            r#"
            SELECT proof_id, state_id, proof, result, submitter,
                   block_number, transaction_hash, gas_used
            FROM ethereum_proof_submissions 
            WHERE proof_id = $1
            "#
        )
        .bind(proof_id.as_slice())
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(ProofSubmission {
                proof_id: ProofId::from_slice(row.get::<&[u8], _>("proof_id")),
                state_id: StateId::from_slice(row.get::<&[u8], _>("state_id")),
                proof: alloy_primitives::Bytes::from(row.get::<Vec<u8>, _>("proof")),
                result: alloy_primitives::Bytes::from(row.get::<Vec<u8>, _>("result")),
                submitter: alloy_primitives::Address::from_slice(row.get::<&[u8], _>("submitter")),
                block_number: row.get::<i64, _>("block_number") as u64,
                transaction_hash: alloy_primitives::FixedBytes::from_slice(row.get::<&[u8], _>("transaction_hash")),
                gas_used: row.get::<String, _>("gas_used").parse().unwrap_or_default(),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_events_by_block_range(&self, from_block: u64, to_block: u64) -> Result<Vec<ContractEvent>> {
        let rows = sqlx::query(
            r#"
            SELECT event_type, state_id, proof_id, block_number,
                   transaction_hash, log_index, timestamp
            FROM ethereum_contract_events 
            WHERE block_number >= $1 AND block_number <= $2
            ORDER BY block_number, log_index
            "#
        )
        .bind(from_block as i64)
        .bind(to_block as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut events = Vec::new();
        for row in rows {
            events.push(ContractEvent {
                event_type: row.get("event_type"),
                state_id: row.get::<Option<&[u8]>, _>("state_id")
                    .map(|s| StateId::from_slice(s)),
                proof_id: row.get::<Option<&[u8]>, _>("proof_id")
                    .map(|p| ProofId::from_slice(p)),
                block_number: row.get::<i64, _>("block_number") as u64,
                transaction_hash: alloy_primitives::FixedBytes::from_slice(row.get::<&[u8], _>("transaction_hash")),
                log_index: row.get::<i64, _>("log_index") as u64,
                timestamp: row.get::<i64, _>("timestamp") as u64,
            });
        }

        Ok(events)
    }

    pub async fn get_latest_network_stats(&self) -> Result<Option<NetworkStats>> {
        let row = sqlx::query(
            r#"
            SELECT chain_id, block_number, gas_price, base_fee,
                   network_name, sync_status
            FROM ethereum_network_stats 
            ORDER BY recorded_at DESC 
            LIMIT 1
            "#
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(NetworkStats {
                chain_id: row.get::<i64, _>("chain_id") as u64,
                block_number: row.get::<i64, _>("block_number") as u64,
                gas_price: row.get::<String, _>("gas_price").parse().unwrap_or_default(),
                base_fee: row.get::<Option<String>, _>("base_fee")
                    .and_then(|s| s.parse().ok()),
                network_name: row.get("network_name"),
                sync_status: row.get("sync_status"),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn cleanup_old_data(&self, days_to_keep: i32) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM ethereum_contract_events 
            WHERE created_at < NOW() - INTERVAL '$1 days'
            "#
        )
        .bind(days_to_keep)
        .execute(&self.pool)
        .await?;

        let deleted_events = result.rows_affected();

        let result = sqlx::query(
            r#"
            DELETE FROM ethereum_network_stats 
            WHERE recorded_at < NOW() - INTERVAL '$1 days'
            "#
        )
        .bind(days_to_keep)
        .execute(&self.pool)
        .await?;

        let deleted_stats = result.rows_affected();

        info!("Cleaned up {} old events and {} old network stats", deleted_events, deleted_stats);
        Ok(deleted_events + deleted_stats)
    }
}