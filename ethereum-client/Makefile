# Ethereum Client Makefile
# Provides convenient commands for testing and development

.PHONY: help test test-unit test-mock test-basic test-cli build clean lint fmt check install-deps setup-env

# Default target
help:
	@echo "🧪 Ethereum Client Development Commands"
	@echo "======================================"
	@echo ""
	@echo "Testing:"
	@echo "  make test       - Run all tests"
	@echo "  make test-unit  - Run unit tests only"
	@echo "  make test-mock  - Run mock integration tests"
	@echo "  make test-basic - Run basic usage test (requires config)"
	@echo "  make test-cli   - Test CLI commands"
	@echo ""
	@echo "Development:"
	@echo "  make build      - Build the project"
	@echo "  make check      - Check code without building"
	@echo "  make lint       - Run clippy lints"
	@echo "  make fmt        - Format code"
	@echo "  make clean      - Clean build artifacts"
	@echo ""
	@echo "Setup:"
	@echo "  make setup-env  - Copy .env.example to .env"
	@echo "  make install-deps - Install development dependencies"
	@echo ""
	@echo "Examples:"
	@echo "  make example-basic    - Run basic usage example"
	@echo "  make example-mock     - Run mock integration example"

# Testing commands
test:
	@echo "🧪 Running all tests..."
	./test_commands.sh all

test-unit:
	@echo "🔬 Running unit tests..."
	cargo test

test-mock:
	@echo "🎭 Running mock integration tests..."
	cargo run --example mock_integration

test-basic:
	@echo "🔧 Running basic usage test..."
	cargo run --example basic_usage

test-cli:
	@echo "🖥️  Testing CLI commands..."
	./test_commands.sh cli

# Build commands
build:
	@echo "🔨 Building project..."
	cargo build

build-release:
	@echo "🚀 Building release version..."
	cargo build --release

check:
	@echo "🔍 Checking code..."
	cargo check

# Code quality
lint:
	@echo "📏 Running clippy lints..."
	cargo clippy -- -D warnings

fmt:
	@echo "🎨 Formatting code..."
	cargo fmt

fmt-check:
	@echo "📋 Checking code formatting..."
	cargo fmt -- --check

# Cleanup
clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean

# Setup commands
setup-env:
	@if [ ! -f .env ]; then \
		echo "📝 Creating .env file from template..."; \
		cp .env.example .env; \
		echo "✅ .env file created. Please edit it with your configuration."; \
	else \
		echo "⚠️  .env file already exists."; \
	fi

install-deps:
	@echo "📦 Installing development dependencies..."
	@# Install cargo tools if not present
	@command -v cargo-watch >/dev/null 2>&1 || { echo "Installing cargo-watch..."; cargo install cargo-watch; }
	@command -v cargo-nextest >/dev/null 2>&1 || { echo "Installing cargo-nextest..."; cargo install cargo-nextest; }
	@echo "✅ Development dependencies installed."

# Example commands
example-basic:
	@echo "📖 Running basic usage example..."
	cargo run --example basic_usage

example-mock:
	@echo "📖 Running mock integration example..."
	cargo run --example mock_integration

# CLI commands with common parameters
cli-help:
	@echo "📚 Showing CLI help..."
	cargo run --bin ethereum_service -- --help

cli-network-stats:
	@echo "📊 Getting network statistics..."
	cargo run --bin ethereum_service network-stats

# Development workflow
dev: fmt lint check test-unit
	@echo "✅ Development checks completed!"

# CI workflow
ci: fmt-check lint build test
	@echo "✅ CI pipeline completed!"

# Watch commands (requires cargo-watch)
watch-test:
	@echo "👀 Watching for changes and running tests..."
	cargo watch -x test

watch-check:
	@echo "👀 Watching for changes and checking code..."
	cargo watch -x check

# Database setup (if using database features)
setup-db:
	@echo "🗄️  Setting up database..."
	@if command -v docker >/dev/null 2>&1; then \
		docker run -d \
			--name ethereum-cache-test \
			-e POSTGRES_PASSWORD=password \
			-e POSTGRES_DB=ethereum_cache \
			-p 5433:5432 \
			postgres:15; \
		echo "✅ Test database started on port 5433"; \
		echo "💡 Use: export DATABASE_URL=postgresql://postgres:password@localhost:5433/ethereum_cache"; \
	else \
		echo "❌ Docker not found. Please install Docker to set up test database."; \
	fi

cleanup-db:
	@echo "🧹 Cleaning up test database..."
	@docker stop ethereum-cache-test >/dev/null 2>&1 || true
	@docker rm ethereum-cache-test >/dev/null 2>&1 || true
	@echo "✅ Test database cleaned up."