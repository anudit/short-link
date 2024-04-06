use axum::{
    error_handling::HandleErrorLayer,
    extract::{Path, State},
    http::{Method, StatusCode},
    response::IntoResponse,
    response::Redirect,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{from_str, json};
use std::fs::read_to_string;
use std::{borrow::Cow, collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tower::{BoxError, ServiceBuilder};
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing_subscriber::prelude::*;

#[derive(Deserialize)]
struct LinkMap {
    links: HashMap<String, String>,
}

#[derive(Clone)]
struct AppState {
    link_map: Arc<RwLock<HashMap<String, String>>>,
}

type SharedState = Arc<RwLock<AppState>>;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_key_value_store=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let file_contents = read_to_string("links.json").expect("Failed to read links.json file");
    let link_map: LinkMap = from_str(&file_contents).expect("Failed to parse JSON");

    let app_state = AppState {
        link_map: Arc::new(RwLock::new(link_map.links)),
    };
    let shared_state = Arc::new(RwLock::new(app_state));

    let app = Router::new()
        .route("/:shortcode", get(redirect_to_link))
        .layer(
            ServiceBuilder::new()
                // Handle errors from middleware
                .layer(HandleErrorLayer::new(handle_error))
                .load_shed()
                .concurrency_limit(1024)
                .timeout(Duration::from_secs(60))
                .layer(TraceLayer::new_for_http()),
        )
        .layer(
            CorsLayer::new()
                .allow_methods([Method::GET, Method::POST])
                .allow_headers(Any)
                .allow_origin(Any),
        )
        .layer(CompressionLayer::new())
        .with_state(Arc::clone(&shared_state));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:5008").await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn handle_error(error: BoxError) -> impl IntoResponse {
    if error.is::<tower::timeout::error::Elapsed>() {
        return (StatusCode::REQUEST_TIMEOUT, Cow::from("request timed out"));
    }

    if error.is::<tower::load_shed::error::Overloaded>() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Cow::from("service is overloaded, try again later"),
        );
    }
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Cow::from(format!("Unhandled internal error: {error}")),
    )
}

async fn redirect_to_link(
    Path(shortcode): Path<String>,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    let app_state = state.read().await; // This gives you access to AppState
    let link_map = app_state.link_map.read().await; // Now access the link_map within AppState

    match link_map.get(&shortcode) {
        Some(url) => Ok(Redirect::permanent(url)),
        None => Err((StatusCode::NOT_FOUND, Json(json!({"error": "Not Found"})))),
    }
}
