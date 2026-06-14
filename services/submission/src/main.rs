use axum::{
    extract::{State, DefaultBodyLimit},
    http::{StatusCode, header},
    response::Json,
    routing::post,
    Router,
    body::Bytes,
};
use serde_json::json;
use std::net::SocketAddr;
use tower_http::cors::{CorsLayer, Any};
use multer::Multipart;
use futures::stream;

mod storage;
use storage::Storage;

mod events;
use events::{publish_submission_created, detect_language};

async fn handle_upload(
    State(storage): State<Storage>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> (StatusCode, Json<serde_json::Value>) {
    // Get the content-type header to extract boundary
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|val| val.to_str().ok())
        .unwrap_or("");

    if !content_type.starts_with("multipart/form-data") {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "expected multipart/form-data"})));
    }

    let boundary = content_type
        .trim_start_matches("multipart/form-data; boundary=")
        .to_string();

    // Create a stream from the bytes
    let bytes_stream = stream::once(async move { Ok::<_, std::io::Error>(body.clone()) });
    let mut multipart = Multipart::new(bytes_stream, boundary);

    let mut file_data: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            let data = field.bytes().await.unwrap_or_default();
            file_data = Some(data.to_vec());
        }
    }

    match file_data {
        Some(data) => {
            let id = uuid::Uuid::new_v4().to_string();
            let language = detect_language(&data);
            println!("Detected language: {}", language); 

            match storage.store(&id, &data).await {
                Ok(object_key) => {
                    publish_submission_created(&id, &object_key, language).await;
                    (StatusCode::OK, Json(json!({"id": id, "language": language})))
                }
                Err(e) => {
                    eprintln!("Storage error: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "storage failed"})))
                }
            }
        }
        None => {
            (StatusCode::BAD_REQUEST, Json(json!({"error": "no file field found"})))
        }
    }
}

#[tokio::main]
async fn main() {
    let storage = Storage::new("submissions").await;

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/upload", post(handle_upload))
        .with_state(storage)
        .layer(cors)
        .layer(DefaultBodyLimit::max(200 * 1024 * 1024)); // <-- semicolon added here

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Submission service running on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}