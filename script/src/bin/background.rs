#!/usr/bin/env cargo

//! Background processor binary that asynchronously processes arithmetic transactions
//! and builds the indexed Merkle tree without affecting CLI performance.
//!
//! Run with:
//! ```shell
//! cargo run --bin background
//! ```
//! or
//! ```shell
//! cargo run --bin background -- --batch-size 50 --interval 10
//! ```

use arithmetic_db::{db::init_db, ProcessorBuilder};
use clap::Parser;
use std::time::Duration;
use tracing::info;

/// Command line arguments for the background processor
#[derive(Parser, Debug)]
#[command(
    name = "background",
    about = "Background processor for indexed Merkle tree construction"
)]
struct Args {
    /// Polling interval in seconds
    #[arg(long, default_value = "30")]
    interval: u64,

    /// Batch size for processing transactions
    #[arg(long, default_value = "100")]
    batch_size: usize,

    /// Run once and exit (default: continuous mode)
    #[arg(long)]
    one_shot: bool,

    /// Set logging level
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Setup logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&args.log_level)),
        )
        .init();

    // Load environment variables
    dotenv::dotenv().ok();

    info!("Starting background processor...");
    info!("Polling interval: {} seconds", args.interval);
    info!("Batch size: {}", args.batch_size);
    info!(
        "Mode: {}",
        if args.one_shot {
            "one-shot"
        } else {
            "continuous"
        }
    );

    // Initialize database
    let pool = init_db().await?;
    info!("Database initialized successfully");

    // Create and configure processor
    let mut processor = ProcessorBuilder::new()
        .polling_interval(Duration::from_secs(args.interval))
        .batch_size(args.batch_size)
        .continuous(!args.one_shot)
        .build(pool);

    // Start processing
    info!("Starting background processing...");
    processor.start().await?;

    Ok(())
}
