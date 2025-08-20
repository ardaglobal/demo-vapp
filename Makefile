# Demo vApp Makefile
# Provides convenient shortcuts for common development tasks

REGISTRY ?= ghcr.io
OWNER ?= ardaglobal
IMAGE_NAME ?= demo-vapp
DOCKER_TAG ?= $(shell whoami)-dev
PLATFORM ?= linux/amd64
APPNAME ?= demo-vapp


## help: Get more info on make commands.
.PHONY: help
help: Makefile
	@echo " Choose a command to run in '$(APPNAME)':"
	@sed -n 's/^##//p' $< | column -t -s ':' | sed -e 's/^/ /'

## install: Install all dependencies
.PHONY: install
install:
	./install-dependencies.sh

## env: Copy .env.example to .env
.PHONY: env
env:
	cp .env.example .env
	@echo "✅ Environment file created. Edit .env to add your SINDRI_API_KEY"

## deploy: Deploy circuit to Sindri
.PHONY: deploy
deploy:
	./deploy-circuit.sh

## up: Start services (uses pre-built image)
.PHONY: up
up:
	docker-compose up -d
	@echo "✅ Services started using pre-built image"
	@echo "🌐 Server: http://localhost:8080"
	@echo "🗄️  Database: localhost:5432"

## up-dev: Start services (builds locally)
.PHONY: up-dev
up-dev:
	docker-compose -f docker-compose.yml -f docker-compose.dev.yml up --build -d
	@echo "✅ Services started with local build"
	@echo "🌐 Server: http://localhost:8080"
	@echo "🗄️  Database: localhost:5432"

## down: Stop all services
.PHONY: down
down:
	docker-compose down -v

## docker-build: Build image locally
.PHONY: docker-build
docker-build:
	@echo "Building Docker image locally..."
	@echo "Image: $(REGISTRY)/$(OWNER)/$(IMAGE_NAME):$(DOCKER_TAG)"
	@echo "Platform: $(PLATFORM)"
	@echo ""
	docker build --platform $(PLATFORM) -t $(REGISTRY)/$(OWNER)/$(IMAGE_NAME):$(DOCKER_TAG) .
	@echo "✅ Image built successfully!"
	@echo "🐳 Image: $(REGISTRY)/$(OWNER)/$(IMAGE_NAME):$(DOCKER_TAG)"
	@echo "🏗️  Platform: $(PLATFORM)"

## docker-push: Build and push image to GitHub registry
# Docker Configuration:
#   PLATFORM=linux/amd64  Build for x86_64 (default)
#   PLATFORM=linux/arm64  Build for ARM64
#   Example: make docker-push PLATFORM=linux/amd64
.PHONY: docker-push
docker-push: docker-build
	@echo "Pushing Docker image..."
	@echo "Registry: $(REGISTRY)"
	docker push $(REGISTRY)/$(OWNER)/$(IMAGE_NAME):$(DOCKER_TAG)
	@echo "✅ Image pushed successfully!"
	@echo "🚀 Published: $(REGISTRY)/$(OWNER)/$(IMAGE_NAME):$(DOCKER_TAG)"

## test: Run all tests
.PHONY: test
test:
	cargo test

## run: Local SP1 unit testing (fast ~3.5s Core proofs)
.PHONY: run
run:
	cargo run --release

## cli: CLI client (requires API server running)
.PHONY: cli
cli:
	@cargo run --bin cli -- $(ARGS)

## server: Start API server locally
.PHONY: server
server:
	@echo "Starting API server locally (requires database)..."
	@echo "💡 Tip: Run 'make up postgres' in another terminal first"
	cargo run --bin server --release

## clean-docker: Clean up Docker resources
.PHONY: clean-docker
clean-docker:
	docker-compose down -v
	docker system prune -f -a
	@echo "✅ Docker resources cleaned up"

## clean-sqlx: Clean up SQLx resources
.PHONY: clean-sqlx
clean-sqlx:
	rm -rf .sqlx
	@echo "✅ SQLx resources cleaned up"

## clean-builds: Clean up build artifacts
.PHONY: clean-builds
clean-builds:
	rm -rf target
	rm -rf build
	rm -rf ADS
	@echo "✅ Build artifacts cleaned up"

## clean: Clean up all resources
.PHONY: clean
clean: clean-docker clean-sqlx clean-builds
	@echo "✅ All resources cleaned up"

## initDB: Initialize database (start, migrate, generate cache, stop)
.PHONY: initDB
initDB:
	@echo "🔧 Initializing database with SQLx offline mode support..."
	@echo ""
	@# Check if DATABASE_URL is set, use default if not
	@if [ -z "$$DATABASE_URL" ]; then \
		echo "ℹ️  DATABASE_URL not set, using default..."; \
		export DATABASE_URL="postgresql://postgres:password@localhost:5432/arithmetic_db"; \
	fi
	@echo "📍 Using database: $$DATABASE_URL"
	@echo ""
	@# Start PostgreSQL database
	@echo "🚀 Starting PostgreSQL database..."
	@docker-compose up postgres -d
	@echo ""
	@# Wait for PostgreSQL to be ready
	@echo "⏳ Waiting for PostgreSQL to be ready..."
	@sleep 8
	@# Check database connectivity  
	@echo "🏥 Checking database connectivity..."
	@if ! pg_isready -h localhost -p 5432 -U postgres >/dev/null 2>&1; then \
		echo "❌ PostgreSQL is not ready. Please check if it's running and accessible."; \
		exit 1; \
	fi
	@echo ""
	@# Run database migrations
	@echo "📦 Running database migrations..."
	@cd db && DATABASE_URL="postgresql://postgres:password@localhost:5432/arithmetic_db" sqlx migrate run
	@echo ""
	@# Generate SQLx cache
	@echo "💾 Generating SQLx cache for offline mode..."
	@DATABASE_URL="postgresql://postgres:password@localhost:5432/arithmetic_db" cargo sqlx prepare --workspace
	@echo ""
	@# Stop PostgreSQL database
	@echo "🛑 Stopping PostgreSQL database..."
	@docker-compose down postgres
	@echo ""
	@echo "✅ Database initialization complete!"
	@echo ""
	@echo "💡 You can now use 'SQLX_OFFLINE=true cargo check' without a database connection."
	@echo "📝 The .sqlx/ directory has been updated and should be committed to version control."

## setup: Complete setup from scratch
.PHONY: setup
setup: install env initDB
	@echo ""
	@echo "🎉 Setup complete! Next steps:"
	@echo "1. Edit .env and add your SINDRI_API_KEY"
	@echo "2. Run: make deploy"
	@echo "3. Run: make up"
