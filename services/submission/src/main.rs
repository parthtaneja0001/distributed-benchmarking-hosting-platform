use axum::{
    extract::Multipart,
    http::StatusCode,
    response::Json,
    routing::post,
    Router,
};
use serde_json::json;
use std::net::SocketAddr;

mod storage;
use storage::Storage;

mod events;
use events::publish_submission_created;

async fn handle_upload(mut multipart: Multipart) -> (StatusCode, Json<serde_json::Value>) {
    let data = match multipart.next_field().await {
        Ok(Some(field)) => match field.bytes().await {
            Ok(bytes) => bytes.to_vec(),
            Err(_) => {
                return (StatusCode::BAD_REQUEST, Json(json!({"error": "failed to read field"})));
            }
        },
        _ => {
            return (StatusCode::BAD_REQUEST, Json(json!({"error": "no file provided"})));
        }
    };

    let id = uuid::Uuid::new_v4().to_string();
    let storage = Storage::new("./submissions");

    match storage.store(&id, &data).await {
        Ok(path) => {
            publish_submission_created(&id, &path, "unknown").await;
            (StatusCode::OK, Json(json!({"id": id, "stored": path})))
        }
        Err(e) => {
            eprintln!("Storage error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "storage failed"})))
        }
    }
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/upload", post(handle_upload));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Submission service running on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}