use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;
use serde_json::json;
use std::time::Duration;
use tar::Archive;
use flate2::read::GzDecoder;

/// Detect the programming language from the uploaded tarball.
/// Returns "go", "rust", "cpp", or "unknown".
pub fn detect_language(tarball_bytes: &[u8]) -> &'static str {
    let cursor = std::io::Cursor::new(tarball_bytes);
    let decoder = GzDecoder::new(cursor);
    let mut archive = Archive::new(decoder);

    let entries = match archive.entries() {
        Ok(entries) => entries,
        Err(_) => return "unknown",
    };

    let mut found_go = false;
    let mut found_rust = false;
    let mut found_cpp = false;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path().unwrap_or_default();
        let filename = path.to_string_lossy();
        if filename.contains("go.mod") {
            found_go = true;
        } else if filename.contains("Cargo.toml") {
            found_rust = true;
        } else if filename.contains("CMakeLists.txt") || filename.ends_with(".cpp") || filename.ends_with(".hpp") {
            found_cpp = true;
        }
    }

    if found_go { return "go"; }
    if found_rust { return "rust"; }
    if found_cpp { return "cpp"; }
    "unknown"
}
/// Publish a `submission.created` event to Redpanda.
pub async fn publish_submission_created(id: &str, object_key: &str, language: &str) {
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", "localhost:9092")
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Failed to create Kafka producer");

    let payload = json!({
        "id": id,
        "bucket": "submissions",
        "object_key": object_key,
        "language": language,
    });

    let payload_string = payload.to_string();
    let record = FutureRecord::to("submission.created")
        .key(id)
        .payload(&payload_string);

    match producer.send(record, Duration::from_secs(0)).await {
        Ok(delivery) => println!("Event published: {:?}", delivery),
        Err((e, _)) => eprintln!("Failed to publish event: {}", e),
    }
}