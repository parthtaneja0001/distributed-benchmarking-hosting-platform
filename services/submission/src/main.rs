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

    // parse_boundary handles parameter order, quoting and extra params
    // (e.g. `multipart/form-data; charset=utf-8; boundary="x"`), which a
    // plain prefix-trim does not.
    let boundary = match multer::parse_boundary(content_type) {
        Ok(b) => b,
        Err(_) => {
            return (StatusCode::BAD_REQUEST, Json(json!({"error": "expected multipart/form-data with a boundary"})));
        }
    };

    // Create a stream from the bytes
    let bytes_stream = stream::once(async move { Ok::<_, std::io::Error>(body.clone()) });
    let mut multipart = Multipart::new(bytes_stream, boundary);

    let mut file_data: Option<Vec<u8>> = None;
    let mut file_name = String::from("engine");

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                return (StatusCode::BAD_REQUEST, Json(json!({"error": format!("malformed upload: {e}")})));
            }
        };
        if field.name() == Some("file") {
            if let Some(n) = field.file_name() {
                file_name = n.to_string();
            }
            match field.bytes().await {
                Ok(data) => file_data = Some(data.to_vec()),
                Err(e) => {
                    return (StatusCode::BAD_REQUEST, Json(json!({"error": format!("upload interrupted: {e}")})));
                }
            }
        }
    }

    match file_data {
        Some(data) => {
            let id = uuid::Uuid::new_v4().to_string();
            let language = detect_language(&data);
            println!("Detected language: {}", language); 
            if language == "unknown" {
                return (StatusCode::BAD_REQUEST, Json(json!({
                    "error": "could not detect language: include go.mod, Cargo.toml, or CMakeLists.txt/.cpp files"
                })));
            }
            let display_name = display_name(&file_name, &id);

            match storage.store(&id, &data).await {
                Ok(object_key) => {
                    record_submission(&id, &display_name, language).await;
                    publish_submission_created(&id, &object_key, language).await;
                    (StatusCode::OK, Json(json!({"id": id, "language": language, "name": display_name})))
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

/// "My Engine v2.tar.gz" + id -> "My-Engine-v2-1a2b3c" (safe, bounded length).
fn display_name(file_name: &str, id: &str) -> String {
    let base = file_name
        .trim_end_matches(".tar.gz")
        .trim_end_matches(".tgz");
    let mut clean: String = base
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '-' })
        .take(40)
        .collect();
    if clean.is_empty() {
        clean = "engine".into();
    }
    format!("{}-{}", clean, &id[..6])
}

/// Write submission metadata for the leaderboard (key submission:<id>).
/// Best effort: a Redis outage must not fail the upload itself.
async fn record_submission(id: &str, name: &str, language: &str) {
    let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let doc = json!({
        "id": id,
        "name": name,
        "language": language,
        "status": "queued",
        "submitted_at": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    });
    let result: redis::RedisResult<()> = async {
        let client = redis::Client::open(url.as_str())?;
        let mut conn = client.get_multiplexed_async_connection().await?;
        redis::AsyncCommands::set_ex(&mut conn, format!("submission:{id}"), doc.to_string(), 7 * 24 * 3600).await
    }
    .await;
    if let Err(e) = result {
        eprintln!("failed to record submission {id} in Redis: {e}");
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
        .route("/healthz", axum::routing::get(|| async { "ok" }))
        .with_state(storage)
        .layer(cors)
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024));

    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Submission service running on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}