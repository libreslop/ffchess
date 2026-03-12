use axum::{Router, routing::get};
use server::state::ServerState;
use std::sync::Arc;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};

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
        .nest(
            "/api",
            Router::new().route("/ws", get(server::handlers::ws_handler)),
        )
        .fallback_service(ServeDir::new("client/dist"))
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
