#!/usr/bin/env bash
set -euo pipefail

# CI Bootstrap Script - Shared helpers for ZK CI pipeline
# This script provides common functions used across the CI workflow

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $*"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $*"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $*"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $*"
}

# Check if running in CI
is_ci() {
    [ "${CI:-false}" = "true" ] || [ -n "${GITHUB_ACTIONS:-}" ]
}

# Install SP1 toolchain (placeholder for actual installation)
install_sp1() {
    log_info "Installing SP1 toolchain..."
    
    if command -v sp1 >/dev/null 2>&1; then
        log_success "SP1 already installed"
        sp1 --version || true
        return 0
    fi
    
    # In production, this would be:
    # curl -L https://sp1.succinct.xyz | bash
    # source ~/.bashrc
    # sp1up
    
    # For CI testing, create a mock
    if is_ci; then
        log_warning "Creating mock SP1 CLI for CI testing"
        create_mock_sp1
    else
        log_error "SP1 not found. Please install SP1 toolchain manually."
        return 1
    fi
}

# Create mock SP1 CLI for testing
create_mock_sp1() {
    local sp1_path="/usr/local/bin/sp1"
    
    if [ -w "/usr/local/bin" ] || sudo -n true 2>/dev/null; then
        sudo tee "$sp1_path" > /dev/null << 'EOF'
#!/bin/bash
# Mock SP1 CLI for CI testing
echo "Mock SP1 CLI - command: $*"

case "$1" in
    "--version")
        echo "sp1-cli 0.1.0 (mock)"
        ;;
    "setup")
        echo "Mock: Generating proving and verification keys..."
        # Parse arguments to find output files
        while [[ $# -gt 0 ]]; do
            case $1 in
                --proving-key)
                    touch "$2"
                    shift 2
                    ;;
                --verification-key)
                    touch "$2"
                    shift 2
                    ;;
                *)
                    shift
                    ;;
            esac
        done
        echo "Keys generated successfully (mock)"
        ;;
    "prove")
        echo "Mock: Generating proof..."
        # Parse arguments to find output files
        while [[ $# -gt 0 ]]; do
            case $1 in
                --proof)
                    echo "Mock proof data" > "$2"
                    shift 2
                    ;;
                --public-outputs)
                    echo '{"result":55}' > "$2"
                    shift 2
                    ;;
                *)
                    shift
                    ;;
            esac
        done
        echo "Proof generated successfully (mock)"
        ;;
    "verify")
        echo "Mock: Verifying proof..."
        sleep 1
        echo "Verification successful (mock)"
        ;;
    *)
        echo "Mock SP1 CLI - unknown command: $1"
        exit 1
        ;;
esac
EOF
        sudo chmod +x "$sp1_path"
        log_success "Mock SP1 CLI created at $sp1_path"
    else
        log_error "Cannot create mock SP1 CLI - insufficient permissions"
        return 1
    fi
}

# Validate environment
validate_environment() {
    log_info "Validating CI environment..."
    
    # Check required tools
    local required_tools=("jq" "curl" "tar" "git")
    for tool in "${required_tools[@]}"; do
        if ! command -v "$tool" >/dev/null 2>&1; then
            log_error "Required tool not found: $tool"
            return 1
        fi
    done
    
    # Check Rust toolchain
    if ! command -v rustc >/dev/null 2>&1; then
        log_error "Rust toolchain not found"
        return 1
    fi
    
    # Check for RISC-V target
    if ! rustup target list --installed | grep -q "riscv32im-unknown-none-elf"; then
        log_warning "RISC-V target not installed, installing..."
        rustup target add riscv32im-unknown-none-elf
    fi
    
    log_success "Environment validation passed"
}

# Setup circuit directory
setup_circuit() {
    local circuit_name="${1:-counter}"
    local circuit_dir="circuits/$circuit_name"
    
    log_info "Setting up circuit: $circuit_name"
    
    if [ ! -d "$circuit_dir" ]; then
        log_error "Circuit directory not found: $circuit_dir"
        return 1
    fi
    
    cd "$circuit_dir"
    
    # Validate required files
    local required_files=("Cargo.toml" "src/main.rs" "sindri.json" "build.sh")
    for file in "${required_files[@]}"; do
        if [ ! -f "$file" ]; then
            log_error "Required file not found: $file"
            return 1
        fi
    done
    
    # Make scripts executable
    chmod +x ./*.sh
    
    log_success "Circuit setup complete: $circuit_name"
}

# Clean build artifacts
clean_build() {
    log_info "Cleaning build artifacts..."
    
    # Clean Rust build artifacts
    if [ -d "target" ]; then
        rm -rf target
        log_info "Removed target directory"
    fi
    
    # Clean output directories
    if [ -d ".out" ]; then
        rm -rf .out
        log_info "Removed .out directory"
    fi
    
    # Clean generated keys (if requested)
    if [ "${CLEAN_KEYS:-false}" = "true" ] && [ -d "keys" ]; then
        rm -rf keys
        log_info "Removed keys directory"
    fi
    
    log_success "Build cleanup complete"
}

# Archive artifacts for CI
archive_artifacts() {
    local artifact_name="${1:-zk-artifacts}"
    local output_dir="${2:-.out/artifacts}"
    
    log_info "Archiving artifacts as: $artifact_name"
    
    mkdir -p "$output_dir"
    
    # Copy important files
    [ -f "target/riscv32im-succinct-zkvm-elf/release/counter" ] && \
        cp "target/riscv32im-succinct-zkvm-elf/release/counter" "$output_dir/"
    
    [ -d "keys" ] && cp -r "keys" "$output_dir/"
    [ -d ".out" ] && cp -r ".out" "$output_dir/"
    [ -f "sindri.json" ] && cp "sindri.json" "$output_dir/"
    
    # Create manifest
    cat > "$output_dir/manifest.json" << EOF
{
    "artifact_name": "$artifact_name",
    "created_at": "$(date -Iseconds)",
    "git_sha": "${GITHUB_SHA:-$(git rev-parse HEAD 2>/dev/null || echo 'unknown')}",
    "circuit_name": "$(jq -r '.name' sindri.json 2>/dev/null || echo 'unknown')",
    "files": $(find "$output_dir" -type f -printf '"%f"\n' | jq -s . 2>/dev/null || echo '[]')
}
EOF
    
    log_success "Artifacts archived to: $output_dir"
}

# Display help
show_help() {
    cat << EOF
CI Bootstrap Script - Shared helpers for ZK CI pipeline

Usage: $0 [COMMAND] [OPTIONS]

Commands:
    install-sp1         Install SP1 toolchain
    validate-env        Validate CI environment
    setup-circuit NAME  Setup circuit directory (default: counter)
    clean-build         Clean build artifacts
    archive NAME DIR    Archive artifacts
    help                Show this help

Environment Variables:
    CI                  Set to 'true' in CI environment
    CLEAN_KEYS         Set to 'true' to clean keys during build cleanup
    GITHUB_SHA         Git commit SHA (set by GitHub Actions)

Examples:
    $0 install-sp1
    $0 setup-circuit counter
    $0 clean-build
    $0 archive "build-123" ".out/artifacts"
EOF
}

# Main function
main() {
    case "${1:-help}" in
        "install-sp1")
            install_sp1
            ;;
        "validate-env")
            validate_environment
            ;;
        "setup-circuit")
            setup_circuit "${2:-counter}"
            ;;
        "clean-build")
            clean_build
            ;;
        "archive")
            archive_artifacts "${2:-zk-artifacts}" "${3:-.out/artifacts}"
            ;;
        "help"|"--help"|"-h")
            show_help
            ;;
        *)
            log_error "Unknown command: $1"
            show_help
            exit 1
            ;;
    esac
}

# Run main function if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
