# Verify ADS Database Integration

## Quick Database Check Commands

### 1. Check if nullifiers are being created:
```bash
make cli ARGS="submit-transaction --amount 42"
curl -X POST http://localhost:8080/api/v2/batches/trigger
```

### 2. Enable debug logging to see ADS activity:
```bash
RUST_LOG=debug cargo run --bin server
```

**Look for these debug logs:**
- `🔄 Processing batch with ADS integration`
- `📦 Processing transactions through ADS batch workflow`
- `🔐 ADS Service: Batch inserting N nullifiers`
- `🌳 IndexedMerkleTree: insert_nullifier`

### 3. Check database tables manually:
```sql
-- Count active nullifiers
SELECT COUNT(*) FROM nullifiers WHERE is_active = true;

-- Recent nullifiers
SELECT value, tree_index, created_at 
FROM nullifiers 
WHERE is_active = true 
ORDER BY created_at DESC 
LIMIT 5;

-- ADS state commits
SELECT batch_id, created_at 
FROM ads_state_commits 
ORDER BY created_at DESC 
LIMIT 5;

-- Tree state
SELECT total_nullifiers, next_available_index, updated_at 
FROM tree_state 
WHERE tree_id = 'default';
```

### 4. Verify batch ↔ merkle root mapping:
```sql
SELECT pb.id as batch_id, pb.transaction_count, 
       ads.merkle_root, ads.created_at
FROM proof_batches pb
JOIN ads_state_commits ads ON pb.id = ads.batch_id
ORDER BY pb.id DESC
LIMIT 5;
```

## What Should Happen with ADS Integration:

1. **Transaction submitted** → stored in `incoming_transactions`
2. **Batch trigger** → calls `BackgroundBatchProcessor::process_batch()`
3. **ADS processing** → converts transaction to nullifier value
4. **IMT insertion** → stores in `nullifiers` table with tree structure
5. **Merkle root** → computed and stored in `ads_state_commits`
6. **Batch completion** → proof generation triggered

## Architecture Summary:

```
🔄 Background Processor → 🔐 ADS Service → 🌳 IndexedMerkleTree
                                              ↓
📦 Batch Creation → 💾 Database Tables → ⚡ ZK Proof Generation
```

**Key Tables:**
- `nullifiers` - IMT nullifier values with linked-list structure
- `ads_state_commits` - Merkle roots linked to batches  
- `proof_batches` - Batch metadata and ZK proof status
- `tree_state` - Global IMT state (root, counter, etc.)