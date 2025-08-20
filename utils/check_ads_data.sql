-- Quick verification that ADS integration is working

\echo '=== NULLIFIERS TABLE ==='
SELECT 
    value as nullifier_value,
    tree_index,
    next_index,
    next_value,
    'Positive: ' || CASE WHEN value > 0 THEN 'YES ✅' ELSE 'NO ❌' END as validation
FROM nullifiers 
ORDER BY tree_index DESC 
LIMIT 5;

\echo ''
\echo '=== ADS STATE COMMITS ==='
SELECT 
    batch_id,
    LEFT(encode(merkle_root, 'hex'), 16) as merkle_root_prefix,
    char_length(encode(merkle_root, 'hex')) / 2 as root_length_bytes,
    created_at
FROM ads_state_commits 
ORDER BY batch_id DESC 
LIMIT 5;

\echo ''
\echo '=== BATCH TO ADS RELATIONSHIP ==='
SELECT 
    pb.id as batch_id,
    pb.final_counter_value - pb.previous_counter_value as counter_delta,
    CASE WHEN asc.batch_id IS NOT NULL THEN 'YES ✅' ELSE 'NO ❌' END as has_ads_commit,
    COUNT(n.value) as nullifier_count
FROM proof_batches pb
LEFT JOIN ads_state_commits asc ON pb.id = asc.batch_id
LEFT JOIN nullifiers n ON true  -- Simple join to count nullifiers
WHERE pb.id >= (SELECT MAX(id) - 4 FROM proof_batches)
GROUP BY pb.id, pb.final_counter_value, pb.previous_counter_value, asc.batch_id
ORDER BY pb.id DESC;