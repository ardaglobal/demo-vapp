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
	docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d
	@echo "✅ Services started with local build"
	@echo "🌐 Server: http://localhost:8080"
	@echo "🗄️  Database: localhost:5432"

## down: Stop all services
.PHONY: down
down:
	docker-compose down

## logs: View server logs
.PHONY: logs
logs:
	docker-compose logs server -f

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

## clean: Clean up Docker resources
.PHONY: clean
clean:
	docker-compose down -v
	docker system prune -f
	@echo "✅ Docker resources cleaned up"

## setup: Complete setup from scratch
.PHONY: setup
setup: install env
	@echo ""
	@echo "🎉 Setup complete! Next steps:"
	@echo "1. Edit .env and add your SINDRI_API_KEY"
	@echo "2. Run: make deploy"
	@echo "3. Run: make up"
