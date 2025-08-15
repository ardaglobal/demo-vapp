# Demo vApp Makefile
# Provides convenient shortcuts for common development tasks

.PHONY: help install deploy up up-dev down logs test clean run cli server

# Default target
help:
	@echo "Demo vApp Development Commands:"
	@echo ""
	@echo "Setup:"
	@echo "  make install     Install all dependencies"
	@echo "  make deploy      Deploy circuit to Sindri"
	@echo ""
	@echo "Docker Operations:"
	@echo "  make up          Start services (uses pre-built image)"
	@echo "  make up-dev      Start services (builds locally)"
	@echo "  make down        Stop all services"
	@echo "  make logs        View server logs"
	@echo ""
	@echo "Development:"
	@echo "  make run         Local SP1 unit testing (fast ~3.5s Core proofs)"
	@echo "  make cli         CLI client (requires API server running)"  
	@echo "  make server      Start API server locally"
	@echo "  make test        Run all tests"
	@echo "  make clean       Clean up Docker resources"
	@echo ""
	@echo "Environment:"
	@echo "  make env         Copy .env.example to .env"

# Setup commands
install:
	./install-dependencies.sh

env:
	cp .env.example .env
	@echo "✅ Environment file created. Edit .env to add your SINDRI_API_KEY"

deploy:
	./deploy-circuit.sh

# Docker commands
up:
	docker-compose up -d
	@echo "✅ Services started using pre-built image"
	@echo "🌐 Server: http://localhost:8080"
	@echo "🗄️  Database: localhost:5432"

up-dev:
	docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d
	@echo "✅ Services started with local build"
	@echo "🌐 Server: http://localhost:8080"
	@echo "🗄️  Database: localhost:5432"

down:
	docker-compose down

logs:
	docker-compose logs server -f

# Development commands
test:
	cargo test

# Development commands
run:
	cargo run --release

cli:
	@echo "CLI Usage Examples:"
	@echo "  make cli ARGS='health-check'"
	@echo "  make cli ARGS='store-transaction --a 5 --b 10'"  
	@echo "  make cli ARGS='get-transaction --result 15'"
	@echo ""
	@cargo run --bin cli -- $(ARGS)

server:
	@echo "Starting API server locally (requires database)..."
	@echo "💡 Tip: Run 'make up postgres' in another terminal first"
	cargo run --bin server --release

clean:
	docker-compose down -v
	docker system prune -f
	@echo "✅ Docker resources cleaned up"

# Complete setup from scratch
setup: install env
	@echo ""
	@echo "🎉 Setup complete! Next steps:"
	@echo "1. Edit .env and add your SINDRI_API_KEY"
	@echo "2. Run: make deploy"
	@echo "3. Run: make up"
