use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;
use crate::connection::TelemetryEvent;
use tokio::sync::mpsc;

pub async fn kafka_publisher(mut rx: mpsc::UnboundedReceiver<TelemetryEvent>) {
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", "localhost:9092")
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Failed to create Kafka producer for telemetry");

    while let Some(event) = rx.recv().await {
        let payload = serde_json::to_string(&event).unwrap();
        let record = FutureRecord::to("telemetry.raw").key(&event.order_id).payload(&payload);

        match producer.send(record, std::time::Duration::from_secs(0)).await {
            Ok(_) => {}
            Err((e, _)) => eprintln!("Failed to publish telemetry: {}", e),
        }
    }
}