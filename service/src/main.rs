//! Runs the pixel-factor HTTP service. Configuration comes from the environment;
//! see `docs/service.md`.
//!
//! `pixel-factor-service healthcheck` instead asks a running copy on
//! `PF_LISTEN` for `/healthz` and exits 0 if it answers 200. The container
//! image uses it, since it has no shell or curl.

use pixel_factor_core::KdfParams;
use pixel_factor_service::config::Config;
use pixel_factor_service::store::Store;
use pixel_factor_service::{router, App, BUILD_COMMIT};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        return match healthcheck().await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("healthcheck: {e}");
                ExitCode::FAILURE
            }
        };
    }
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    // One JSON object per line for log collectors; readable text otherwise.
    if std::env::var("PF_LOG_FORMAT").as_deref() == Ok("json") {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(filter)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }
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
        .with_graceful_shutdown(shutdown())
        .await
        .map_err(|e| e.to_string())?;
    tracing::info!("stopped");
    Ok(())
}

/// Resolves on Ctrl-C, or on SIGTERM, which is how a container is stopped.
async fn shutdown() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut term) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = term.recv() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

async fn healthcheck() -> Result<(), String> {
    let listen: SocketAddr = std::env::var("PF_LISTEN")
        .map_err(|_| "PF_LISTEN must be set".to_owned())?
        .parse()
        .map_err(|e| format!("PF_LISTEN: {e}"))?;
    // A wildcard bind address is reached through loopback.
    let addr = match listen {
        SocketAddr::V4(a) if a.ip().is_unspecified() => {
            SocketAddr::from((Ipv4Addr::LOCALHOST, a.port()))
        }
        SocketAddr::V6(a) if a.ip().is_unspecified() => {
            SocketAddr::from((Ipv6Addr::LOCALHOST, a.port()))
        }
        other => other,
    };
    let check = async {
        let mut stream = tokio::net::TcpStream::connect(addr).await?;
        stream
            .write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await?;
        let mut answer = Vec::new();
        stream.read_to_end(&mut answer).await?;
        Ok::<_, std::io::Error>(answer)
    };
    let answer = tokio::time::timeout(Duration::from_secs(2), check)
        .await
        .map_err(|_| format!("{addr}: timed out"))?
        .map_err(|e| format!("{addr}: {e}"))?;
    if answer.starts_with(b"HTTP/1.1 200") {
        Ok(())
    } else {
        Err(format!("{addr}: unexpected answer"))
    }
}
