use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use openrouter_free_gateway::proxy::{GatewayState, create_router};
use tokio::sync::RwLock;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(
    name = "openrouter-free-gateway",
    about = "High-performance proxy exposing strictly 100% free OpenRouter models"
)]
struct Args {
    #[arg(long, default_value = "127.0.0.1", env = "GATEWAY_HOST")]
    host: String,

    #[arg(short, long, default_value_t = 4005, env = "GATEWAY_PORT")]
    port: u16,

    #[arg(
        long,
        default_value = "https://openrouter.ai/api/v1",
        env = "OPENROUTER_BASE_URL"
    )]
    openrouter_url: String,

    #[arg(long, default_value_t = 300, env = "GATEWAY_CACHE_TTL_SECS")]
    cache_ttl_secs: u64,

    #[arg(long, env = "OPENROUTER_API_KEY")]
    api_key: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "openrouter_free_gateway=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;

    let state = GatewayState {
        client,
        cache: Arc::new(RwLock::new(None)),
        cache_ttl: Duration::from_secs(args.cache_ttl_secs),
        default_api_key: args.api_key,
        openrouter_base_url: args.openrouter_url,
    };

    let app = create_router(state).layer(tower_http::trace::TraceLayer::new_for_http());

    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(
        target: "gateway",
        "OpenRouter Free Gateway listening on http://{}", addr
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!(target: "gateway", "Server stopped gracefully.");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
