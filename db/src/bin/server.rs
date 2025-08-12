//! `RESTful` API server for the SP1 arithmetic counter vApp
//!
//! This server provides a REST API for submitting transactions, generating proofs,
//! and verifying proofs externally. It integrates with the existing Merkle tree
//! infrastructure and Sindri proof generation.
//!
//! Run this server using:
//! ```shell
//! cd db && cargo run --bin server
//! ```

use arithmetic_db::{
    ads_service::AdsServiceFactory,
    api::{ApiConfig, ApiServerBuilder},
    db::init_db,
    vapp_integration::{
        MockComplianceService, MockNotificationService, MockProofService, MockSettlementService,
        VAppAdsIntegration, VAppConfig,
    },
};
use clap::Parser;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// Command line arguments for the server
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Host to bind to
    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    /// Port to bind to
    #[arg(long, default_value = "8080")]
    port: u16,

    /// Enable CORS for all origins
    #[arg(long, default_value = "true")]
    cors: bool,

    /// Enable GraphQL endpoint
    #[arg(long, default_value = "true")]
    graphql: bool,

    /// Enable GraphQL playground
    #[arg(long, default_value = "true")]
    playground: bool,

    /// Maximum request size in bytes
    #[arg(long, default_value = "1048576")] // 1MB
    max_request_size: usize,

    /// Request timeout in seconds
    #[arg(long, default_value = "30")]
    timeout: u64,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[allow(clippy::too_many_lines)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenv::dotenv().ok();

    // Parse command line arguments
    let args = Args::parse();

    // Initialize logging
    let filter = match args.log_level.as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "info" => tracing::Level::INFO,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => unreachable!(),
    };

    tracing_subscriber::fmt()
        .with_max_level(filter)
        .with_target(false)
        .compact()
        .init();

    info!("🚀 Starting vApp REST API Server");
    info!("📡 Host: {}", args.host);
    info!("🔌 Port: {}", args.port);
    info!("🌐 CORS: {}", args.cors);
    info!("🔄 GraphQL: {}", args.graphql);
    info!("🎮 Playground: {}", args.playground);

    // Initialize database
    info!("💾 Initializing database connection...");
    let pool = match init_db().await {
        Ok(pool) => {
            info!("✅ Database initialized successfully");
            pool
        }
        Err(e) => {
            error!("❌ Failed to initialize database: {}", e);
            error!("💡 Make sure PostgreSQL is running and configured correctly");
            error!("💡 Check your DATABASE_URL environment variable");
            std::process::exit(1);
        }
    };

    // Create ADS service
    info!("🌳 Initializing Authenticated Data Structure...");
    let factory = AdsServiceFactory::new(pool.clone());
    let ads = match factory.create_indexed_merkle_tree().await {
        Ok(ads) => {
            info!("✅ ADS service initialized successfully");
            Arc::new(RwLock::new(ads))
        }
        Err(e) => {
            error!("❌ Failed to create ADS service: {}", e);
            std::process::exit(1);
        }
    };

    // Create vApp integration with mock services
    info!("🔧 Initializing vApp integration...");
    let vapp_config = VAppConfig::default();
    let vapp_integration = match VAppAdsIntegration::new(
        pool.clone(),
        vapp_config,
        Arc::new(MockSettlementService),
        Arc::new(MockProofService),
        Arc::new(MockComplianceService),
        Arc::new(MockNotificationService),
    )
    .await
    {
        Ok(vapp) => {
            info!("✅ vApp integration initialized successfully");
            Arc::new(RwLock::new(vapp))
        }
        Err(e) => {
            error!("❌ Failed to create vApp integration: {}", e);
            std::process::exit(1);
        }
    };

    // Configure API server
    let api_config = ApiConfig {
        server_name: "vApp Arithmetic Counter API".to_string(),
        version: "1.0.0".to_string(),
        max_batch_size: 1000,
        rate_limit_per_minute: 100,
        enable_metrics: true,
        enable_debug_endpoints: false,
        cors_origins: if args.cors {
            vec!["*".to_string()]
        } else {
            vec![]
        },
    };

    // Build API server
    info!("🔧 Building API server...");
    let server = match ApiServerBuilder::new()
        .host(args.host.clone())
        .port(args.port)
        .api_config(api_config.clone())
        .enable_rest(true)
        .enable_graphql(args.graphql)
        .enable_playground(args.playground)
        .enable_subscriptions(false) // Disable for simplicity
        .cors_origins(if args.cors {
            vec!["*".to_string()]
        } else {
            vec![]
        })
        .request_timeout(args.timeout)
        .max_request_size(args.max_request_size)
        .build(ads.clone(), vapp_integration.clone())
        .await
    {
        Ok(server) => {
            info!("✅ API server built successfully");
            server
        }
        Err(e) => {
            error!("❌ Failed to build API server: {}", e);
            std::process::exit(1);
        }
    };

    // Create API state for the router
    let api_state = arithmetic_db::api::ApiState {
        ads: ads.clone(),
        vapp_integration: vapp_integration.clone(),
        config: api_config.clone(),
    };

    // Create the router using the REST API directly
    let mut app = arithmetic_db::api::create_router(api_state);

    // Apply CORS if enabled
    if args.cors {
        app = app.layer(tower_http::cors::CorsLayer::permissive());
    }

    // Bind and serve
    let bind_address = server.bind_address();
    info!("🎯 Binding to address: {}", bind_address);

    let listener = match tokio::net::TcpListener::bind(&bind_address).await {
        Ok(listener) => {
            info!("✅ Successfully bound to {bind_address}");
            listener
        }
        Err(e) => {
            error!("❌ Failed to bind to {bind_address}: {e}");
            error!("💡 Make sure the port is not already in use");
            std::process::exit(1);
        }
    };

    // Print startup information
    println!();
    println!("🌟 vApp REST API Server Running!");
    println!("📍 Server Address: http://{bind_address}");
    println!();
    println!("📚 Available Endpoints:");
    println!("   • POST   /api/v1/transactions      - Submit new transactions");
    println!("   • GET    /api/v1/results/{{result}}  - Get transaction by result");
    println!("   • GET    /api/v1/proofs/{{proof_id}} - Get proof information");
    println!("   • POST   /api/v1/results/{{result}}/verify - Verify proof for result");
    println!("   • POST   /api/v1/verify            - Verify proof by ID");
    println!("   • GET    /api/v1/health             - Health check");
    println!("   • GET    /api/v1/info               - API information");

    if args.graphql {
        println!("   • POST   /graphql                  - GraphQL endpoint");
        if args.playground {
            println!("   • GET    /playground               - GraphQL playground");
        }
    }

    println!();
    println!("🔗 Example cURL Commands:");
    println!("   # Submit transaction");
    println!("   curl -X POST http://{bind_address}/api/v1/transactions \\");
    println!("        -H 'Content-Type: application/json' \\");
    println!("        -d '{{\"a\": 5, \"b\": 10, \"generate_proof\": true}}'");
    println!();
    println!("   # Get transaction by result");
    println!("   curl http://{bind_address}/api/v1/results/15");
    println!();
    println!("   # Health check");
    println!("   curl http://{bind_address}/api/v1/health");
    println!();

    if args.playground {
        println!("🎮 GraphQL Playground: http://{bind_address}/playground");
        println!();
    }

    println!("🛑 Press Ctrl+C to stop the server");
    println!();

    // Start serving
    info!("🎯 Starting to serve requests...");

    if let Err(e) = axum::serve(listener, app).await {
        error!("❌ Server error: {}", e);
        std::process::exit(1);
    }

    info!("👋 Server shutdown complete");
    Ok(())
}
