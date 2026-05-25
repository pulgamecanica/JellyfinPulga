pub mod routes;

use crate::api::JellyfinApi;
use crate::chat::db::ChatDb;
use crate::config::Config;
use axum::Router;
use std::sync::Arc;
use tower_http::services::ServeDir;

pub struct AppState {
    pub db: ChatDb,
    pub api: JellyfinApi,
    pub config: Config,
}

pub async fn start_server(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let db = ChatDb::new("jellyfin_pulga.db")?;
    let api = JellyfinApi::new(&config.jellyfin);

    let state = Arc::new(AppState { db, api, config: config.clone() });

    let app = Router::new()
        .nest("/api", routes::api_routes())
        .fallback_service(ServeDir::new("static"))
        .with_state(state);

    let addr = format!("{}:{}", config.server.host, config.server.port);
    println!("Starting web server on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
