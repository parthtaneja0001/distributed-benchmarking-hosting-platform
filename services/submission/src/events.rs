use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;
use serde_json::json;
use std::time::Duration;

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