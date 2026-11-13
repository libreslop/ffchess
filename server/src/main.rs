//! Server entrypoint wiring routes, state, and the game loop.

use axum::{Router, routing::get};
use server::state::ServerState;
use std::sync::Arc;
use std::time::Duration;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};

/// Interval between simulation ticks.
const TICK_INTERVAL: Duration = Duration::from_millis(100);

/// Port used when `PORT` is not set.
const DEFAULT_PORT: u16 = 8080;

/// Boots the HTTP/WebSocket server and the periodic game tick loop.
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let state = Arc::new(ServerState::new());

    let ticking_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(TICK_INTERVAL);
        loop {
            interval.tick().await;
            ticking_state.tick();
        }
    });

    let app = Router::new()
        .route("/ping", get(|| async { "pong" }))
        .nest_service("/assets", ServeDir::new(server::paths::assets_dir()))
        .nest(
            "/api",
            Router::new()
                .route("/ws/:mode_id", get(server::handlers::ws_handler))
                .route("/modes", get(server::handlers::list_modes)),
        )
        .route("/", get(server::handlers::index_html))
        .fallback_service(ServeDir::new(server::paths::client_dist_dir()))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .unwrap_or_else(|e| panic!("Failed to bind port {port}: {e}"));
    tracing::info!("FFchess listening on :{}", port);
    axum::serve(listener, app).await.expect("server error");
}
