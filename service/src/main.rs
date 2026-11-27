//! Runs the pixel-factor HTTP service. Configuration comes from the environment;
//! see `docs/service.md`.

use pixel_factor_core::KdfParams;
use pixel_factor_service::config::Config;
use pixel_factor_service::store::Store;
use pixel_factor_service::{router, App, BUILD_COMMIT};
use std::process::ExitCode;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "failed to start");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let config = Config::from_env()?;
    tracing::warn!("using the development pepper from PF_DEV_PEPPER_HEX; not for production");
    let store = Store::open(&config.db_path).map_err(|e| e.to_string())?;
    let app = Arc::new(App::new(&config, store, KdfParams::default())?);
    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .map_err(|e| format!("bind {}: {e}", config.listen))?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        commit = BUILD_COMMIT,
        images = config.images.len(),
        "started"
    );
    // A plain line on stdout, so scripts and agents can read the address.
    println!("listening on http://{addr}");
    axum::serve(listener, router(app))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|e| e.to_string())
}
