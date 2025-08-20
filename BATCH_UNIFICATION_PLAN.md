# Batch Flow Unification Plan

## Problem Statement
Currently we have TWO separate batch creation workflows:
- Legacy SQL-based workflow (no ADS integration)
- ADS-integrated workflow (BackgroundBatchProcessor)

This creates inconsistency and confusion. All batch creation should go through the same ADS-integrated flow.

## Current Architecture (BROKEN)

```
┌─────────────────────────────────────────────────────────────────┐
│                     DUAL WORKFLOWS (BAD)                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Legacy Path (No ADS):                                        │
│  POST /api/v2/batches                                          │
│      ↓                                                         │
│  create_batch_endpoint()                                       │
│      ↓                                                         │
│  create_batch() [db.rs]                                        │
│      ↓                                                         │
│  SQL create_batch() function                                   │
│      ↓                                                         │
│  ❌ NO ADS, NO nullifiers, NO merkle roots                     │
│                                                                 │
│  ─────────────────────────────────────────────────────         │
│                                                                 │
│  ADS Path (Full Integration):                                  │
│  POST /api/v2/batches/trigger                                  │
│  Background Timer                                              │
│  Count Threshold                                               │
│      ↓                                                         │
│  BackgroundBatchProcessor::process_batch()                     │
│      ↓                                                         │
│  ✅ ADS Service → IMT → Nullifiers → Merkle Roots             │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Target Architecture (UNIFIED)

```
┌─────────────────────────────────────────────────────────────────┐
│                   UNIFIED ADS WORKFLOW                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  All Triggers:                                                 │
│  • POST /api/v2/batches                                        │
│  • POST /api/v2/batches/trigger                                │
│  • Background Timer                                            │
│  • Count Threshold                                             │
│      ↓                                                         │
│  UnifiedBatchService::create_batch_with_ads()                  │
│      ↓                                                         │
│  ✅ ADS Service → IMT → Nullifiers → Merkle Roots             │
│      ↓                                                         │
│  Consistent batch creation with full ADS integration           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Implementation Steps

### Step 1: Create Unified Batch Service
Create a new service that consolidates batch creation logic:

```rust
pub struct UnifiedBatchService {
    pool: PgPool,
    ads_service: Arc<RwLock<IndexedMerkleTreeADS>>,
}

impl UnifiedBatchService {
    pub async fn create_batch_with_ads(
        &self, 
        batch_size: Option<i32>,
        trigger_source: &str
    ) -> Result<Option<i32>, String> {
        // Unified ADS-integrated batch creation logic
        // This replaces both the old SQL create_batch() and process_batch()
    }
}
```

### Step 2: Update REST Endpoint
Modify `POST /api/v2/batches` to use the unified service:

```rust
async fn create_batch_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<CreateBatchRequest>,
) -> Result<Json<CreateBatchResponse>, (StatusCode, String)> {
    // Use unified batch service instead of legacy create_batch()
    let unified_service = UnifiedBatchService::new(state.pool, state.ads_service);
    match unified_service.create_batch_with_ads(request.batch_size, "api").await {
        // Handle response...
    }
}
```

### Step 3: Update Background Processor
Modify BackgroundBatchProcessor to use the same unified service:

```rust
impl BackgroundBatchProcessor {
    async fn process_batch(&self, trigger_type: &str) -> Result<Option<i32>, String> {
        let unified_service = UnifiedBatchService::new(self.pool.clone(), self.ads_service.clone());
        unified_service.create_batch_with_ads(None, trigger_type).await
    }
}
```

### Step 4: Deprecate Legacy Components
- Mark SQL `create_batch()` function as deprecated
- Remove direct calls to `create_batch()` from db.rs
- Ensure all batch creation goes through unified service

## Benefits

1. **Consistency**: All batches are created with ADS integration
2. **Maintainability**: Single codebase for batch logic
3. **Reliability**: No more confusion about which endpoint to use
4. **Completeness**: All batches have nullifiers and merkle roots

## Migration Strategy

1. Implement unified service alongside existing code
2. Update endpoints one by one to use unified service
3. Test thoroughly to ensure same behavior
4. Remove legacy code once all paths are unified
5. Update API documentation to reflect unified behavior

## Testing Plan

1. Test all trigger methods produce identical batch structure
2. Verify all batches have nullifiers and merkle roots
3. Confirm backward compatibility for API clients
4. Performance testing to ensure no regression