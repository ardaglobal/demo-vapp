#!/usr/bin/env python3
"""Quick script to check ADS table contents."""

import os
import psycopg2
from psycopg2 import sql

# Get database URL from environment
database_url = os.environ.get('DATABASE_URL', 'postgres://postgres@localhost:5432/postgres')

try:
    # Connect to database
    conn = psycopg2.connect(database_url)
    cur = conn.cursor()
    
    print("🔍 Checking ADS table contents...")
    
    # Check nullifiers table
    cur.execute("SELECT COUNT(*) FROM nullifiers WHERE is_active = true")
    nullifier_count = cur.fetchone()[0]
    print(f"📊 Active nullifiers: {nullifier_count}")
    
    if nullifier_count > 0:
        cur.execute("SELECT value, tree_index, created_at FROM nullifiers WHERE is_active = true ORDER BY created_at DESC LIMIT 3")
        recent_nullifiers = cur.fetchall()
        print("📝 Recent nullifiers:")
        for value, tree_index, created_at in recent_nullifiers:
            print(f"   Value: {value}, Tree Index: {tree_index}, Created: {created_at}")
    
    # Check ads_state_commits table
    cur.execute("SELECT COUNT(*) FROM ads_state_commits")
    commit_count = cur.fetchone()[0]
    print(f"📊 ADS state commits: {commit_count}")
    
    if commit_count > 0:
        cur.execute("SELECT batch_id, created_at FROM ads_state_commits ORDER BY created_at DESC LIMIT 3")
        recent_commits = cur.fetchall()
        print("📝 Recent ADS commits:")
        for batch_id, created_at in recent_commits:
            print(f"   Batch ID: {batch_id}, Created: {created_at}")
    
    # Check tree_state table
    cur.execute("SELECT total_nullifiers, next_available_index, updated_at FROM tree_state WHERE tree_id = 'default'")
    tree_state = cur.fetchone()
    if tree_state:
        total_nullifiers, next_index, updated_at = tree_state
        print(f"📊 Tree state: {total_nullifiers} nullifiers, next index: {next_index}, updated: {updated_at}")
    else:
        print("❌ No default tree state found")
    
    # Check recent batches
    cur.execute("SELECT id, transaction_count, proof_status, created_at FROM proof_batches ORDER BY created_at DESC LIMIT 3")
    recent_batches = cur.fetchall()
    print("📝 Recent batches:")
    for batch_id, tx_count, status, created_at in recent_batches:
        print(f"   Batch {batch_id}: {tx_count} txns, status: {status}, created: {created_at}")
    
    cur.close()
    conn.close()
    
except Exception as e:
    print(f"❌ Error connecting to database: {e}")
    print(f"💡 Make sure DATABASE_URL is set correctly: {database_url}")