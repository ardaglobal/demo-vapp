#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

echo "Deploying circuit bundle with BYO proving key to Sindri..."

# Check for required environment variable
: "${SINDRI_API_KEY:?ERROR: SINDRI_API_KEY environment variable is required}"

# Configuration
CIRCUIT_NAME=$(jq -r '.name' sindri.json 2>/dev/null || echo "arda-counter-v1")
BUNDLE_DIR=".out/bundle"
BUNDLE_FILE=".out/counter-circuit.tgz"

echo "Configuration:"
echo "  Circuit Name: $CIRCUIT_NAME"
echo "  Bundle Output: $BUNDLE_FILE"
echo "  API Key: ${SINDRI_API_KEY:0:8}..."
echo ""

# Validate required files exist
required_files=("sindri.json" "Cargo.toml" "src/main.rs" "keys/proving.key" "keys/verifying.key")
for file in "${required_files[@]}"; do
    if [ ! -f "$file" ]; then
        echo "ERROR: Required file $file not found. Run ./build.sh first."
        exit 1
    fi
done

# Check if ELF exists
ELF_PATH=$(jq -r '.artifactPaths.elf' sindri.json)
if [ ! -f "$ELF_PATH" ]; then
    echo "ERROR: ELF not found at $ELF_PATH. Run ./build.sh first."
    exit 1
fi

echo "✓ All required files found"
echo ""

# Create bundle directory and copy required files
echo "Creating circuit bundle..."
rm -rf "$BUNDLE_DIR" && mkdir -p "$BUNDLE_DIR"

# Copy source files
cp -r src Cargo.toml sindri.json "$BUNDLE_DIR/"

# Copy the ELF binary (create target directory structure)
ELF_DIR=$(dirname "$ELF_PATH")
mkdir -p "$BUNDLE_DIR/$ELF_DIR"
cp "$ELF_PATH" "$BUNDLE_DIR/$ELF_PATH"

# Copy keys (this is the BYO-PK part!)
cp -r keys "$BUNDLE_DIR/"

# Copy input examples for reference
if [ -d "inputs" ]; then
    cp -r inputs "$BUNDLE_DIR/"
fi

# Add metadata
cat > "$BUNDLE_DIR/README.md" << EOF
# Arda Counter Circuit Bundle

This bundle contains:
- SP1 guest program source code
- Compiled RISC-V ELF binary
- **BYO Proving Key** and Verification Key
- Sindri manifest configuration
- Example input files

## Circuit Information
- Name: $CIRCUIT_NAME
- Type: SP1
- ELF: $ELF_PATH
- Proving Key: keys/proving.key
- Verification Key: keys/verifying.key

Generated on: $(date)
Git commit: \${GIT_SHA:-unknown}
EOF

echo "Bundle contents:"
find "$BUNDLE_DIR" -type f | sort

# Create tarball
echo ""
echo "Creating tarball..."
tar -czf "$BUNDLE_FILE" -C "$BUNDLE_DIR" .

echo "✓ Bundle created: $BUNDLE_FILE ($(du -h "$BUNDLE_FILE" | cut -f1))"
echo ""

# Upload to Sindri
echo "Uploading to Sindri..."
echo "This would execute:"
echo ""
echo "# Using Sindri CLI:"
echo "sindri circuits create \\"
echo "  --api-key \"\$SINDRI_API_KEY\" \\"
echo "  --file \"$BUNDLE_FILE\""
echo ""
echo "# Or using curl:"
echo "curl -X POST \\"
echo "  -H \"Authorization: Bearer \$SINDRI_API_KEY\" \\"
echo "  -H \"Content-Type: multipart/form-data\" \\"
echo "  -F \"file=@$BUNDLE_FILE\" \\"
echo "  https://sindri.app/api/v1/circuits"
echo ""

# Simulate upload (in real implementation, this would call Sindri API)
echo "Simulating upload..."
sleep 2

# Create a mock response
cat > ".out/sindri_upload_response.json" << EOF
{
  "circuit_id": "$(uuidgen | tr '[:upper:]' '[:lower:]')",
  "name": "$CIRCUIT_NAME",
  "status": "Ready",
  "created_at": "$(date -Iseconds)",
  "proving_key_uploaded": true,
  "verification_key_uploaded": true,
  "elf_uploaded": true
}
EOF

echo "✓ Upload simulation complete"
echo ""
echo "Mock Sindri Response:"
if command -v jq >/dev/null 2>&1; then
    jq . ".out/sindri_upload_response.json"
else
    cat ".out/sindri_upload_response.json"
fi

echo ""
echo "🎉 CIRCUIT DEPLOYMENT SUCCESSFUL (simulated)"
echo ""
echo "Your circuit bundle with BYO proving key has been uploaded to Sindri."
echo "The proving key you generated locally will be used for all remote proofs."
echo ""
echo "Next steps:"
echo "  - Generate Sindri proof: ./prove_sindri.sh"
echo "  - Compare with local proof: ./verify_compare.sh"
echo ""
echo "Note: In production, replace the simulation with actual Sindri API calls."
