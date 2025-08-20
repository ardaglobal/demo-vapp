#!/bin/bash

# Script to regenerate the sqlx cache for offline development
# This should be run whenever the database schema changes

set -e

echo "🔧 Regenerating sqlx cache for offline development..."

# Check if DATABASE_URL is set
if [ -z "$DATABASE_URL" ]; then
    echo "ℹ️  DATABASE_URL not set, using default..."
    export DATABASE_URL="postgresql://postgres:password@localhost:5432/arithmetic_db"
fi

echo "📍 Using database: $DATABASE_URL"

# Start PostgreSQL if not running
echo "🚀 Starting PostgreSQL database..."
docker-compose up postgres -d

# Wait for PostgreSQL to be ready
echo "⏳ Waiting for PostgreSQL to be ready..."
sleep 5

# Check if database is accessible
echo "🏥 Checking database connectivity..."
if ! pg_isready -h localhost -p 5432 -U postgres; then
    echo "❌ PostgreSQL is not ready. Please check if it's running and accessible."
    exit 1
fi

# Run database migrations
echo "📦 Running database migrations..."
cd db && sqlx migrate run
cd ..

# Generate sqlx cache
echo "💾 Generating sqlx cache..."
cargo sqlx prepare --workspace

echo "✅ sqlx cache regenerated successfully!"
echo ""
echo "💡 You can now use 'SQLX_OFFLINE=true cargo check' without a database connection."
echo "📝 Don't forget to commit the .sqlx/ directory to version control."

# Optional: Stop database if you want
read -p "🛑 Stop PostgreSQL database? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "🛑 Stopping PostgreSQL..."
    docker-compose down postgres
    echo "✅ PostgreSQL stopped"
else
    echo "ℹ️  PostgreSQL left running for development"
fi

echo "🎉 Done!"
