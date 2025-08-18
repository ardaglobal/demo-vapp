# Release Management Strategy

This document outlines the comprehensive release management strategy for the demo-vapp repository, which contains three distinct but interconnected components that require coordinated versioning.

## Component Overview

### 1. Smart Contracts (`contracts/`)
- **Location**: `contracts/src/Arithmetic.sol` and related contracts
- **Dependencies**: SP1 contracts, OpenZeppelin libraries
- **Deployment Target**: Ethereum networks (mainnet, testnets)
- **Versioning Strategy**: Independent semantic versioning
- **Release Artifacts**: Compiled contracts, deployment addresses, ABIs

### 2. ZKVM Program (`program/`)
- **Location**: `program/src/main.rs`
- **Dependencies**: SP1 ZKVM runtime (`sp1-zkvm = "5.2.1"`)
- **Build Target**: SP1 ZKVM ELF binary
- **Versioning Strategy**: Independent semantic versioning
- **Release Artifacts**: Program verification key (vkey), ELF binary

### 3. Web Server Application (Main Repository)
- **Location**: Root workspace with `api/`, `db/`, `cli/`, etc.
- **Dependencies**: Both smart contracts and ZKVM program
- **Deployment Target**: Docker containers via `docker-compose.yml`
- **Versioning Strategy**: Main repository version that references specific versions of contracts and ZKVM program
- **Release Artifacts**: Docker images, deployment configurations

## Semantic Versioning Strategy

### Version Format: `MAJOR.MINOR.PATCH`

#### Smart Contracts (`contracts-vX.Y.Z`)
- **MAJOR**: Breaking changes to contract interface or state structure
- **MINOR**: New features, additional functions, backward-compatible changes
- **PATCH**: Bug fixes, gas optimizations, security patches

#### ZKVM Program (`program-vX.Y.Z`)
- **MAJOR**: Changes to program logic that affect verification key or public inputs/outputs
- **MINOR**: Performance improvements, additional features that don't change verification
- **PATCH**: Bug fixes, optimizations that don't affect verification

#### Main Application (`vX.Y.Z`)
- **MAJOR**: Breaking changes to API, database schema, or deployment requirements
- **MINOR**: New features, API enhancements, backward-compatible changes
- **PATCH**: Bug fixes, dependency updates, minor improvements

## Release Process

### 1. Smart Contract Releases

#### Pre-Release Checklist
- [ ] Update contract version comments
- [ ] Run full test suite: `cd contracts && forge test`
- [ ] Gas optimization analysis
- [ ] Security audit (for major releases)
- [ ] Update deployment documentation

#### Release Steps
1. **Tag the release**: `git tag contracts-v1.2.3`
2. **Deploy to testnet**: Use GitHub Actions or manual deployment
3. **Verify contracts**: Ensure Etherscan verification
4. **Update deployment records**: Record addresses in `contracts/deployments/`
5. **Generate ABI artifacts**: Store in release assets
6. **Create GitHub release**: Include deployment addresses and ABI files

#### Post-Release
- Update dependent services with new contract addresses
- Notify integration partners of new contract versions

### 2. ZKVM Program Releases

#### Pre-Release Checklist
- [ ] Verify program compiles: `cd program && cargo build --release`
- [ ] Generate verification key: `cd script && cargo run --release -- --vkey`
- [ ] Test proof generation and verification
- [ ] Benchmark performance changes
- [ ] Update program documentation

#### Release Steps
1. **Tag the release**: `git tag program-v2.1.0`
2. **Build ELF binary**: Automated via CI/CD
3. **Generate verification key**: Store in release assets
4. **Test with current contracts**: Ensure compatibility
5. **Create GitHub release**: Include vkey and performance metrics

#### Post-Release
- Update contract deployments if vkey changed (major version)
- Update web server configuration to use new program version

### 3. Main Application Releases

#### Pre-Release Checklist
- [ ] Specify contract and program versions in release notes
- [ ] Update Docker images: `docker build -t demo-vapp:vX.Y.Z .`
- [ ] Database migration testing
- [ ] Integration testing with specified contract/program versions
- [ ] Load testing
- [ ] Update documentation and API specs

#### Release Steps
1. **Update version references**:
   ```toml
   # In api/Cargo.toml or environment configuration
   CONTRACT_VERSION = "contracts-v1.2.3"
   PROGRAM_VERSION = "program-v2.1.0"
   ```

2. **Tag the release**: `git tag v3.4.5`

3. **Build and push Docker images**:
   ```bash
   docker build -t ghcr.io/ardaglobal/demo-vapp:v3.4.5 .
   docker push ghcr.io/ardaglobal/demo-vapp:v3.4.5
   ```

4. **Update docker-compose.yml**:
   ```yaml
   services:
     server:
       image: ghcr.io/ardaglobal/demo-vapp:v3.4.5
   ```

5. **Create GitHub release** with comprehensive release notes

## Version Compatibility Matrix

| Main App Version | Contract Version | Program Version | SP1 SDK Version |
|------------------|------------------|-----------------|-----------------|
| v1.0.0           | contracts-v1.0.0 | program-v1.0.0  | 5.2.1          |
| v1.1.0           | contracts-v1.0.0 | program-v1.1.0  | 5.2.1          |
| v1.2.0           | contracts-v1.1.0 | program-v1.1.0  | 5.2.1          |
| v2.0.0           | contracts-v2.0.0 | program-v2.0.0  | 5.3.0          |

## Configuration Management

### Environment Variables
```bash
# Contract Configuration
CONTRACT_ADDRESS=0x1234567890123456789012345678901234567890
CONTRACT_VERSION=contracts-v1.2.3

# Program Configuration  
PROGRAM_VKEY=0x00620892344c310c32a74bf0807a5c043964264e4f37c96a10ad12b5c9214e0e
PROGRAM_VERSION=program-v2.1.0

# Application Configuration
APP_VERSION=v3.4.5
```

### Version Tracking in Code

#### Rust Configuration (`api/src/config.rs`)
```rust
#[derive(Debug, Clone)]
pub struct VersionConfig {
    pub app_version: &'static str,
    pub contract_version: &'static str,
    pub contract_address: String,
    pub program_version: &'static str,
    pub program_vkey: String,
}

impl Default for VersionConfig {
    fn default() -> Self {
        Self {
            app_version: env!("CARGO_PKG_VERSION"),
            contract_version: env!("CONTRACT_VERSION"),
            contract_address: std::env::var("CONTRACT_ADDRESS")
                .expect("CONTRACT_ADDRESS must be set"),
            program_version: env!("PROGRAM_VERSION"),
            program_vkey: std::env::var("PROGRAM_VKEY")
                .expect("PROGRAM_VKEY must be set"),
        }
    }
}
```

## Automated Release Pipeline

### GitHub Actions Workflow Structure

```yaml
name: Multi-Component Release

on:
  push:
    tags:
      - 'contracts-v*'
      - 'program-v*' 
      - 'v*'

jobs:
  detect-component:
    runs-on: ubuntu-latest
    outputs:
      component: ${{ steps.detect.outputs.component }}
      version: ${{ steps.detect.outputs.version }}
    steps:
      - id: detect
        run: |
          if [[ $GITHUB_REF == refs/tags/contracts-v* ]]; then
            echo "component=contracts" >> $GITHUB_OUTPUT
            echo "version=${GITHUB_REF#refs/tags/contracts-v}" >> $GITHUB_OUTPUT
          elif [[ $GITHUB_REF == refs/tags/program-v* ]]; then
            echo "component=program" >> $GITHUB_OUTPUT
            echo "version=${GITHUB_REF#refs/tags/program-v}" >> $GITHUB_OUTPUT
          else
            echo "component=app" >> $GITHUB_OUTPUT
            echo "version=${GITHUB_REF#refs/tags/v}" >> $GITHUB_OUTPUT
          fi

  release-contracts:
    if: needs.detect-component.outputs.component == 'contracts'
    # ... contract deployment steps

  release-program:
    if: needs.detect-component.outputs.component == 'program'
    # ... program build and vkey generation steps

  release-app:
    if: needs.detect-component.outputs.component == 'app'
    # ... Docker build and deployment steps
```

## Development Workflow

### Feature Development
1. **Create feature branch**: `git checkout -b feature/new-feature`
2. **Develop with current versions**: Use latest stable versions of all components
3. **Test integration**: Ensure compatibility across all components
4. **Update version references**: If component versions changed during development

### Release Preparation
1. **Version bump decision**: Determine which components need version bumps
2. **Dependency updates**: Update SP1 SDK or other dependencies if needed
3. **Integration testing**: Test all components together
4. **Documentation updates**: Update README, API docs, deployment guides

### Hotfix Process
1. **Identify scope**: Determine which component(s) need fixes
2. **Create hotfix branch**: `git checkout -b hotfix/critical-fix`
3. **Apply minimal fix**: Keep changes as small as possible
4. **Fast-track testing**: Focused testing on affected component
5. **Expedited release**: Follow abbreviated release process

## Monitoring and Rollback

### Version Monitoring
- **Health checks**: Each component exposes version information
- **Compatibility alerts**: Monitor for version mismatches
- **Performance tracking**: Track performance across version changes

### Rollback Strategy
1. **Application rollback**: Revert Docker image in docker-compose.yml
2. **Program rollback**: Revert to previous vkey (requires contract support)
3. **Contract rollback**: Deploy new contract with previous logic (complex)

### Emergency Procedures
- **Circuit breaker**: Ability to pause system during critical issues
- **Rollback automation**: Scripts to quickly revert to known-good versions
- **Communication plan**: Notify stakeholders of version changes and issues

## Release Notes Template

### Main Application Release (v3.4.5)
**Component Versions:**
- Smart Contracts: contracts-v1.2.3 (0x1234...7890)
- ZKVM Program: program-v2.1.0 (vkey: 0x0062...4e0e)
- SP1 SDK: 5.2.1

**Changes:**
- **Added**: New API endpoints for batch operations
- **Changed**: Improved database query performance
- **Fixed**: Memory leak in proof verification
- **Security**: Updated dependencies for CVE fixes

**Migration Notes:**
- Database migration required: `sqlx migrate run`
- Environment variable changes: Added `NEW_CONFIG_VAR`
- API breaking changes: `/v1/old-endpoint` deprecated

**Deployment:**
```bash
docker-compose down
docker-compose pull
docker-compose up -d
```

## Best Practices

### Version Management
- **Pin exact versions**: Never use floating versions in production
- **Test compatibility**: Always test component version combinations
- **Document dependencies**: Maintain clear dependency graphs
- **Automate verification**: Use CI/CD to verify version compatibility

### Release Coordination
- **Staged rollouts**: Deploy to staging before production
- **Feature flags**: Use flags to enable/disable features across versions
- **Monitoring**: Monitor metrics across version deployments
- **Communication**: Keep stakeholders informed of release schedules

### Security Considerations
- **Audit trail**: Maintain complete history of version deployments
- **Access control**: Restrict who can create releases
- **Verification**: Verify signatures and checksums for all artifacts
- **Incident response**: Have procedures for security-related rollbacks

This release strategy ensures coordinated, traceable, and reliable deployments across all components of the demo-vapp ecosystem while maintaining the flexibility to release components independently when appropriate.
