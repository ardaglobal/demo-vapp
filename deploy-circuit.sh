#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔧 Sindri Circuit Build & Deployment Script${NC}"
echo "==========================================="

# Show usage if requested
if [ "$1" = "--help" ] || [ "$1" = "-h" ]; then
    echo "Usage: $0 [TAG]"
    echo ""
    echo "Arguments:"
    echo "  TAG    Optional circuit tag"
    echo ""
    echo "Tag Resolution (in priority order):"
    echo "  1. Command line argument (if provided)"
    echo "  2. SINDRI_CIRCUIT_TAG environment variable (if set and not 'latest')"
    echo "  3. Default to 'latest' tag"
    echo ""
    echo "Examples:"
    echo "  $0                 # Uses SINDRI_CIRCUIT_TAG env var, or 'latest'"
    echo "  $0 dev-v1.0       # Deploy with 'dev-v1.0' tag (overrides env var)"
    echo "  $0 local-\$(date +%s)  # Deploy with timestamp tag"
    echo ""
    echo "Environment variables:"
    echo "  SINDRI_API_KEY     Required - your Sindri API key"
    echo "  SINDRI_CIRCUIT_TAG Optional - circuit tag to use if no CLI arg provided"
    exit 0
fi

# Source .env file first if it exists
if [ -f ".env" ]; then
    echo -e "${BLUE}📄 Loading environment from .env file...${NC}"
    set -a  # Automatically export all variables
    source .env
    set +a  # Stop auto-exporting
fi

# Parse command line arguments and check environment variable
TAG=""
if [ $# -gt 0 ]; then
    TAG="$1"
elif [ -n "$SINDRI_CIRCUIT_TAG" ] && [ "$SINDRI_CIRCUIT_TAG" != "latest" ]; then
    TAG="$SINDRI_CIRCUIT_TAG"
    echo -e "${BLUE}📋 Using circuit tag from environment: ${TAG}${NC}"
fi

# Check if Sindri CLI is installed
if ! command -v sindri &> /dev/null; then
    echo -e "${RED}❌ Sindri CLI not found. Please run ./install-dependencies.sh first${NC}"
    exit 1
fi

# Check if API key is set
if [ -z "$SINDRI_API_KEY" ]; then
    echo -e "${YELLOW}⚠️  SINDRI_API_KEY not set in environment${NC}"
    echo "Please set your Sindri API key using one of these methods:"
    echo ""
    echo "Method 1 - Export directly:"
    echo "  export SINDRI_API_KEY=your_api_key_here"
    echo "  $0 $*"
    echo ""
    echo "Method 2 - Add to .env file:"
    echo "  echo 'SINDRI_API_KEY=your_api_key_here' >> .env"
    echo "  $0 $*"
    echo ""
    echo "Method 3 - Source .env in current shell:"
    echo "  source .env && $0 $*"
    echo ""
    if [ -f ".env" ]; then
        echo "Note: Found .env file but SINDRI_API_KEY is not set or empty in it."
    else
        echo "Note: No .env file found in current directory."
    fi
    exit 1
fi

echo -e "${BLUE}🔨 Building SP1 program (ELF)...${NC}"
if [ ! -d "program" ]; then
    echo -e "${RED}❌ Program directory not found. Make sure you're in the project root.${NC}"
    exit 1
fi

# Build the SP1 program to create the ELF file
cd program
if cargo prove build --output-directory ../build; then
    echo -e "${GREEN}✅ SP1 program built successfully${NC}"
    cd ..
else
    echo -e "${RED}❌ Failed to build SP1 program${NC}"
    cd ..
    exit 1
fi

# Verify ELF file exists
if [ ! -f "build/arithmetic-program" ]; then
    echo -e "${RED}❌ ELF file not found at ./build/arithmetic-program${NC}"
    echo "Expected location: $(pwd)/build/arithmetic-program"
    exit 1
fi

echo -e "${BLUE}📋 Linting circuit...${NC}"
if sindri lint; then
    echo -e "${GREEN}✅ Circuit lint passed${NC}"
else
    echo -e "${RED}❌ Circuit lint failed${NC}"
    exit 1
fi

# Deploy with or without tag
if [ -n "$TAG" ]; then
    echo -e "${BLUE}🚀 Deploying circuit with tag: ${TAG}${NC}"
    DEPLOY_ARGS=(deploy --tag "$TAG")
else
    echo -e "${BLUE}🚀 Deploying circuit (will use 'latest' tag by default)${NC}"
    DEPLOY_ARGS=(deploy)
fi

if sindri "${DEPLOY_ARGS[@]}"; then
    echo -e "${GREEN}✅ Circuit deployed successfully!${NC}"
    echo -e "${GREEN}✅ Circuit built and deployed successfully!${NC}"
    echo ""
    echo -e "${BLUE}📝 Deployment Summary:${NC}"
    echo "• SP1 program compiled to ELF: ./build/arithmetic-program"
    echo "• Circuit deployed to Sindri with tag: ${TAG:-latest}"
    echo "• Circuit name: demo-vapp:${TAG:-latest}"
    echo ""
    echo -e "${BLUE}📝 Next steps:${NC}"
    echo "1. Start the server: docker-compose up -d"
    echo "2. Test proof generation:"
    echo "   curl -X POST http://localhost:8080/api/v1/transactions \\"
    echo "     -H 'Content-Type: application/json' \\"
    echo "     -d '{\"a\": 5, \"b\": 10, \"generate_proof\": true}'"
    echo ""
    echo "3. Or use the CLI:"
    echo "   cd script && cargo run --release -- --prove --a 5 --b 10"
else
    echo -e "${RED}❌ Circuit deployment failed${NC}"
    exit 1
fi
