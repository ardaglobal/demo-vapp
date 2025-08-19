#!/bin/bash

# SQL Test Runner
# Runs all SQL test files in the current directory

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check if DATABASE_URL is set
if [ -z "$DATABASE_URL" ]; then
    echo -e "${RED}ERROR: DATABASE_URL environment variable is not set${NC}"
    echo "Please set it to your PostgreSQL connection string:"
    echo 'export DATABASE_URL="postgresql://postgres:password@localhost:5432/arithmetic_db"'
    exit 1
fi

# Check if psql is available
if ! command -v psql &> /dev/null; then
    echo -e "${RED}ERROR: psql command not found${NC}"
    echo "Please install PostgreSQL client tools"
    exit 1
fi

# Test database connectivity
echo -e "${BLUE}🔍 Testing database connectivity...${NC}"
if ! psql "$DATABASE_URL" -c "SELECT 1;" > /dev/null 2>&1; then
    echo -e "${RED}ERROR: Cannot connect to database${NC}"
    echo "Please ensure PostgreSQL is running and DATABASE_URL is correct"
    exit 1
fi
echo -e "${GREEN}✅ Database connection successful${NC}"

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Find all test files
TEST_FILES=("$SCRIPT_DIR"/test_*.sql)

if [ ${#TEST_FILES[@]} -eq 0 ] || [ ! -f "${TEST_FILES[0]}" ]; then
    echo -e "${YELLOW}⚠️  No test files found (test_*.sql)${NC}"
    exit 0
fi

echo -e "${BLUE}🧪 Found ${#TEST_FILES[@]} SQL test file(s)${NC}"
echo

# Run each test file
PASSED=0
FAILED=0

for test_file in "${TEST_FILES[@]}"; do
    if [ ! -f "$test_file" ]; then
        continue
    fi
    
    test_name=$(basename "$test_file" .sql)
    echo -e "${BLUE}🔄 Running: $test_name${NC}"
    echo "----------------------------------------"
    
    # Run the test and capture output
    if psql "$DATABASE_URL" -f "$test_file"; then
        echo -e "${GREEN}✅ PASSED: $test_name${NC}"
        ((PASSED++))
    else
        echo -e "${RED}❌ FAILED: $test_name${NC}"
        ((FAILED++))
    fi
    
    echo
done

# Summary
echo "========================================"
echo -e "${BLUE}📊 Test Results Summary${NC}"
echo -e "${GREEN}✅ Passed: $PASSED${NC}"
if [ $FAILED -gt 0 ]; then
    echo -e "${RED}❌ Failed: $FAILED${NC}"
else
    echo -e "${GREEN}❌ Failed: $FAILED${NC}"
fi
echo "========================================"

# Exit with error code if any tests failed
if [ $FAILED -gt 0 ]; then
    exit 1
else
    echo -e "${GREEN}🎉 All tests passed!${NC}"
    exit 0
fi
