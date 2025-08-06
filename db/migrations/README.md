# Database Migrations

This directory contains SQLx database migrations for the arithmetic-db package.

## Migration Files

Migrations are versioned SQL files that are applied in order. Each migration file should:

1. Be named with a sequential number prefix (e.g., `001_`, `002_`, etc.)
2. Have a descriptive name after the number
3. Use the `.sql` extension

## Current Migrations

- `001_create_arithmetic_transactions.sql` - Creates the initial arithmetic_transactions table with indexes

## Running Migrations

Migrations are automatically run when the database is initialized via the `init_db()` function. The `sqlx::migrate!` macro handles:

- Tracking which migrations have been applied
- Running migrations in the correct order
- Preventing duplicate migrations
- Error handling and rollback support

## Adding New Migrations

To add a new migration:

1. Create a new SQL file with the next sequential number
2. Write your SQL statements (CREATE, ALTER, etc.)
3. The migration will be automatically applied on the next database initialization

## Migration Best Practices

- Always use `IF NOT EXISTS` for CREATE statements when possible
- Use `IF EXISTS` for DROP statements when possible
- Include comments explaining the purpose of each migration
- Test migrations on a copy of production data before applying to production
- Consider the impact on existing data when modifying table structures