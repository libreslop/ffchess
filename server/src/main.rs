//! Server entrypoint wiring routes, state, and game loop.

use axum::{Router, routing::get};
use server::state::ServerState;
use std::sync::Arc;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};

#[tokio::main]
/// Boots the HTTP/WebSocket server and the periodic game tick loop.
async fn main() {
    tracing_subscriber::fmt::init();

    let state = Arc::new(ServerState::new());

    // Initialize shops for all games
    state.spawn_initial_shops().await;

    // Spawn game loop task
    let state_clone = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
        loop {
            interval.tick().await;
            state_clone.tick_all_games().await;
            state_clone.tick_previews().await;
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
        .unwrap_or(8080);

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("FFchess listening on :{}", port);
    axum::serve(listener, app).await.unwrap();
}
