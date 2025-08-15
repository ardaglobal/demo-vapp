#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔧 Sindri Circuit Deployment Script${NC}"
echo "=================================="

# Show usage if requested
if [ "$1" = "--help" ] || [ "$1" = "-h" ]; then
    echo "Usage: $0 [TAG]"
    echo ""
    echo "Arguments:"
    echo "  TAG    Optional circuit tag (defaults to 'latest' if not provided)"
    echo ""
    echo "Examples:"
    echo "  $0                 # Deploy with 'latest' tag"
    echo "  $0 dev-v1.0       # Deploy with 'dev-v1.0' tag"
    echo "  $0 local-\$(date +%s)  # Deploy with timestamp tag"
    echo ""
    echo "Environment variables:"
    echo "  SINDRI_API_KEY     Required - your Sindri API key"
    exit 0
fi

# Parse command line arguments
TAG=""
if [ $# -gt 0 ]; then
    TAG="$1"
fi

# Check if Sindri CLI is installed
if ! command -v sindri &> /dev/null; then
    echo -e "${RED}❌ Sindri CLI not found. Please run ./install-dependencies.sh first${NC}"
    exit 1
fi

# Check if API key is set
if [ -z "$SINDRI_API_KEY" ]; then
    echo -e "${YELLOW}⚠️  SINDRI_API_KEY not set in environment${NC}"
    echo "Please set your Sindri API key:"
    echo "  export SINDRI_API_KEY=your_api_key_here"
    echo ""
    echo "Or add it to your .env file and run:"
    echo "  source .env"
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
    DEPLOY_CMD="sindri deploy --tag \"$TAG\""
else
    echo -e "${BLUE}🚀 Deploying circuit (will use 'latest' tag by default)${NC}"
    DEPLOY_CMD="sindri deploy"
fi

if eval "$DEPLOY_CMD"; then
    echo -e "${GREEN}✅ Circuit deployed successfully!${NC}"
    echo ""
    echo -e "${BLUE}📝 Next steps:${NC}"
    echo "1. Start the server: docker-compose up -d"
    echo "2. Test proof generation:"
    echo "   curl -X POST http://localhost:8080/api/v1/transactions \\"
    echo "     -H 'Content-Type: application/json' \\"
    echo "     -d '{\"a\": 5, \"b\": 10, \"generate_proof\": true}'"
else
    echo -e "${RED}❌ Circuit deployment failed${NC}"
    exit 1
fi
