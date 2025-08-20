-- Quick test to verify ADS integration is working
-- Check if nullifiers, tree state, and ADS state commits are being populated

SELECT 'Nullifiers in tree:' as table_name, COUNT(*) as count FROM nullifiers WHERE is_active = true
UNION ALL
SELECT 'Tree state records:', COUNT(*) FROM tree_state
UNION ALL  
SELECT 'ADS state commits:', COUNT(*) FROM ads_state_commits
UNION ALL
SELECT 'Recent batches:', COUNT(*) FROM proof_batches WHERE created_at > NOW() - INTERVAL '1 hour';

-- Show some sample data
SELECT 'Recent nullifier values:' as info, '' as data
UNION ALL
SELECT 'Value: ' || value::text, 'Tree Index: ' || tree_index::text 
FROM nullifiers 
WHERE is_active = true 
ORDER BY created_at DESC 
LIMIT 3;