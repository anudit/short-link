extern crate dotenv;

use axum::{
    error_handling::HandleErrorLayer,
    extract::{Path, State},
    http::{Method, StatusCode},
    response::IntoResponse,
    response::Redirect,
    routing::get,
    Json, Router,
};
use dotenv::dotenv;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{from_str, json};
use std::fs::read_to_string;
use std::{borrow::Cow, collections::HashMap, env, sync::Arc, time::Duration};
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

async fn send_umami_track_event(url: &str, shortcode: &str) -> Result<(), reqwest::Error> {
    let client = Client::new();
    let umami_host = "https://umami.omnid.io/api/send";

    let website_id = match env::var("UMAMI_WEBSITE_ID") {
        Ok(v) => v,
        Err(_) => "dev".to_string(),
    };

    eprint!("website_id:{:?}", website_id);

    let payload = json!({
        "type": "event",
        "payload": {
            "website": website_id,
            "name": shortcode,
            "url": url
        }
    });

    let response = client
        .post(umami_host)
        .header(
            "User-Agent",
            "Mozilla/5.0 (U; Linux x86_64; en-US) Gecko/20130401 Firefox/69.6",
        )
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await;

    match response {
        Ok(resp) => match resp.text().await {
            Ok(text) => {
                eprint!("{:?}", text);
            }
            Err(e) => eprintln!("Failed to read response text: {}", e),
        },
        Err(e) => eprintln!("Failed to send request: {}", e),
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_key_value_store=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let file_contents = read_to_string("./links.json").expect("Failed to read links.json file");
    let link_map: LinkMap = from_str(&file_contents).expect("Failed to parse JSON");

    let app_state = AppState {
        link_map: Arc::new(RwLock::new(link_map.links)),
    };
    let shared_state = Arc::new(RwLock::new(app_state));

    let app = Router::new()
        .route("/", get(get_links_count))
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
        Some(url) => {
            let _ = send_umami_track_event(url, &shortcode).await;
            Ok(Redirect::permanent(url))
        }
        None => Err((StatusCode::NOT_FOUND, Json(json!({"error": "Not Found"})))),
    }
}

async fn get_links_count(State(state): State<SharedState>) -> impl IntoResponse {
    let app_state = state.read().await; // Access the AppState
    let link_map = app_state.link_map.read().await; // Access the link_map within AppState

    let count = link_map.len();
    (StatusCode::OK, Json(json!({"total_links": count})))
}
