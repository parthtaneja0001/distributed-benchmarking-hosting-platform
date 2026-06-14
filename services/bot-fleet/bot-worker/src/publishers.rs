use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;
use serde::Serialize;
use tokio::sync::mpsc;

#[derive(Serialize, Clone)]
pub struct OrderSent {
    pub test_id: String,
    pub order_id: String,
    pub order_type: String,
    pub side: String,
    pub price: f64,
    pub quantity: u32,
}

#[derive(Serialize, Clone)]
pub struct FillActual {
    pub test_id: String,
    pub order_id: String,
    pub fill_price: f64,
    pub fill_quantity: u32,
}

fn create_producer() -> FutureProducer {
    let broker = std::env::var("KAFKA_BROKER").unwrap_or_else(|_| "localhost:9092".to_string());
    ClientConfig::new()
        .set("bootstrap.servers", &broker)
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Failed to create Kafka producer")
}

pub async fn order_publisher(mut rx: mpsc::UnboundedReceiver<OrderSent>) {
    let producer = create_producer();
    while let Some(event) = rx.recv().await {
        let payload = serde_json::to_string(&event).unwrap();
        let record = FutureRecord::to("orders.sent")
            .key(&event.order_id)
            .payload(&payload);
        let _ = producer.send(record, std::time::Duration::from_secs(0)).await;
    }
}

pub async fn fill_publisher(mut rx: mpsc::UnboundedReceiver<FillActual>) {
    let producer = create_producer();
    while let Some(event) = rx.recv().await {
        let payload = serde_json::to_string(&event).unwrap();
        let record = FutureRecord::to("fills.actual")
            .key(&event.order_id)
            .payload(&payload);
        let _ = producer.send(record, std::time::Duration::from_secs(0)).await;
    }
}