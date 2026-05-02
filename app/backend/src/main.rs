use axum::{routing::get, Json, Router};
use serde_json::json;
use std::{env, path::PathBuf};
use tower_http::cors::CorsLayer;

mod yuanling;
mod spiritkind;

async fn health() -> Json<serde_json::Value> {
  Json(json!({
    "status": "ok",
    "service": "taichu-backend",
  }))
}

#[tokio::main]
async fn main() {
  let host = env::var("BACKEND_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
  let port = env::var("BACKEND_PORT").unwrap_or_else(|_| "4000".to_string());
  let bind_addr = format!("{host}:{port}");
  let data_dir = env::var("BACKEND_DATA_DIR").unwrap_or_else(|_| "./data".to_string());

  let data_path = PathBuf::from(data_dir);
  if let Err(err) = std::fs::create_dir_all(&data_path) {
    eprintln!("failed to create data directory: {err}");
    return;
  }

  let app = Router::new()
    .route("/health", get(health))
    .merge(yuanling::router())
    .layer(CorsLayer::permissive());
  let listener = match tokio::net::TcpListener::bind(&bind_addr).await {
    Ok(listener) => listener,
    Err(err) => {
      eprintln!("failed to bind {bind_addr}: {err}");
      return;
    }
  };

  println!("Taichu backend listening on {bind_addr}");
  if let Err(err) = axum::serve(listener, app.into_make_service()).await {
    eprintln!("server error: {err}");
  }
}
