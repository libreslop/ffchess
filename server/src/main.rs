mod state;
mod handlers;
mod npc;
mod state_extensions;

use axum::{
    routing::get,
    Router,
    response::Html,
};
use std::sync::Arc;
use crate::state::ServerState;
use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
    trace::TraceLayer,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let state = Arc::new(ServerState::new());
    state.spawn_initial_shops().await;

    // Spawn game loop task
    let state_clone = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
        loop {
            interval.tick().await;
            state_clone.handle_tick().await;
        }
    });

    // Serve static files from client/dist at "/"
    // Serve the API routes under "/api"
    let app = Router::new()
        .route("/ping", get(|| async { "pong" }))
        .nest("/api", Router::new().route("/ws", get(handlers::ws_handler)))
        .fallback_service(ServeDir::new("client/dist"))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("[INFO] FFChess listening on :8080");
    axum::serve(listener, app).await.unwrap();
}
