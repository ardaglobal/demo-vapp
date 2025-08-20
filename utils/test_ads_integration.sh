#!/bin/bash
# Test script for ADS integration verification

echo "🧪 Testing ADS Integration - Step by Step"
echo "=========================================="

echo ""
echo "📝 Step 1: Submit a test transaction"
RESPONSE1=$(curl -s -X POST http://localhost:8080/api/v2/transactions \
     -H "Content-Type: application/json" \
     -d '{"amount": 1337}')
echo "Transaction Response: $RESPONSE1"

echo ""
echo "⏳ Step 2: Wait 2 seconds..."
sleep 2

echo ""
echo "🔄 Step 3: Trigger ADS-integrated batch processing (CORRECT ENDPOINT)"
RESPONSE2=$(curl -s -X POST http://localhost:8080/api/v2/batches/trigger)
echo "Batch Trigger Response: $RESPONSE2"

echo ""
echo "⏳ Step 4: Wait 3 seconds for processing..."
sleep 3

echo ""
echo "📋 Step 5: Check recent batches"
echo "Recent batches:"
make cli ARGS="list-batches" 2>/dev/null | head -10

echo ""
echo "🔍 Step 6: Expected ADS Integration Evidence"
echo "If ADS integration is working, you should see:"
echo "  ✅ New batch created with counter progression"
echo "  ✅ Debug logs showing 'Processing batch with ADS integration'"
echo "  ✅ Debug logs showing 'batch inserting nullifiers'"
echo "  ✅ Data in nullifiers, ads_state_commits tables"
echo ""
echo "💡 To see debug logs: Check server output with RUST_LOG=debug"
echo "💡 To check database: Use the SQL queries in test_ads_data.sql"