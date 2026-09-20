use sidefeed::{api, config::Config, ingest, AppState};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter(
        EnvFilter::try_from_default_env().unwrap_or_else(|_| "sidefeed=info,tower_http=info".into())
    ).init();
    let config = Config::from_env()?;
    let listen = config.listen;
    let state = AppState::new(config).await?;
    let worker_state = state.clone();
    tokio::spawn(async move { ingest::poll_loop(worker_state).await });
    let listener = TcpListener::bind(listen).await?;
    tracing::info!(%listen, "sidefeed listening");
    axum::serve(listener, api::router(state)).with_graceful_shutdown(shutdown_signal()).await?;
    Ok(())
}
async fn shutdown_signal() {
    let ctrl_c = async { tokio::signal::ctrl_c().await.expect("install ctrl-c handler") };
    #[cfg(unix)]
    let terminate = async { tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("install signal handler").recv().await; };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
}
